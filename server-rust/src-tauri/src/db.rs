// Couche base de données SQLite (rusqlite, SQLite embarqué).
// Port de server/db.ts : mêmes tables, mêmes types de stockage (TEXT ISO pour les dates,
// INTEGER 0/1 pour les booléens) pour rester compatible avec la base existante.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use serde_json::{Map, Value, json};
use rand::RngCore;

use crate::error::{ApiError, ApiResult};

// ===== IDENTIFIANTS DISTRIBUÉS (anti-collision multi-appareils) =====
//
// AVANT : `id INTEGER PRIMARY KEY` laissait SQLite choisir max(id)+1. Deux
// appareils hors ligne créaient donc tous les deux l'id 5 pour DEUX lignes
// différentes. Au push, l'un écrasait l'autre côté Supabase ; au pull suivant,
// `apply_remote` faisait un INSERT OR REPLACE sur l'id 5 et le compte créé
// localement disparaissait silencieusement.
//
// MAINTENANT : id façon Snowflake généré localement, sans réseau :
//   32 bits de secondes | 11 bits de shard appareil | 10 bits de séquence
// Deux appareils distincts ne peuvent pas produire le même id (shard différent),
// et un même appareil supporte 1024 insertions par seconde.
//
// CONTRAINTE CLÉ : 53 bits au maximum. Les ids traversent JSON jusqu'à Vue, où
// Number.MAX_SAFE_INTEGER = 2^53-1 ; un id plus grand serait ARRONDI côté
// JavaScript et l'app appellerait les API avec un mauvais id. Ici la valeur
// actuelle vaut ~1,8e14, soit 50x sous la limite, et le format tient jusqu'en 2160.
//
// Les ids restent croissants dans le temps (ORDER BY id = ordre de création) et
// les anciennes lignes à petits ids restent parfaitement valides.

/// Epoch applicative : 2024-01-01T00:00:00Z (garde les ids courts).
const ID_EPOCH_SECS: i64 = 1_704_067_200;

/// Séquence intra-seconde, partagée par tout le processus.
static ID_SEQUENCE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// 11 bits stables dérivés du device_id (FNV-1a) : l'empreinte de l'appareil.
fn device_shard(device_id: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in device_id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h % 2048) as i64
}

/// Vrai si la table possède une colonne `id` (donc synchronisée). `app_settings`
/// et les tables techniques n'en ont pas et gardent leur comportement d'origine.
/// Résultat mis en cache : un seul PRAGMA par table et par processus.
fn table_has_id_column(conn: &Connection, table: &str) -> bool {
    static CACHE: std::sync::OnceLock<Mutex<std::collections::HashMap<String, bool>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some(v) = g.get(table) {
            return *v;
        }
    }
    let found = conn
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))
        .and_then(|mut stmt| {
            let names = stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(names.iter().any(|n| n == "id"))
        })
        .unwrap_or(false);
    if let Ok(mut g) = cache.lock() {
        g.insert(table.to_string(), found);
    }
    found
}

/// Identifiant unique au monde, sans coordination réseau.
fn generate_distributed_id(device_id: &str) -> i64 {
    let secs = (chrono::Utc::now().timestamp() - ID_EPOCH_SECS).max(0) & 0xFFFF_FFFF;
    let seq = ID_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0x3FF;
    (secs << 21) | (device_shard(device_id) << 10) | seq
}

#[allow(dead_code)]
pub const TABLES: [&str; 11] = [
    "users",
    "consoles",
    "jeux",
    "joueurs",
    "sessions_jeu",
    "tarifs",
    "factures",
    "jetons_transactions",
    "messages",
    "parametres_fidelite",
    "lignes_facture",
];

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, email TEXT UNIQUE, password_hash TEXT, nom TEXT, role TEXT DEFAULT 'employe', created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS joueurs (id INTEGER PRIMARY KEY, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TEXT, derniere_visite TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS consoles (id INTEGER PRIMARY KEY, nom TEXT, type TEXT, etat TEXT DEFAULT 'disponible', poste_numero INTEGER, date_ajout TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS sessions_jeu (id INTEGER PRIMARY KEY, console_id INTEGER, joueur_id INTEGER, jeu_id INTEGER, employe_id INTEGER, tarif_id INTEGER, debut TEXT, fin TEXT, duree_minutes INTEGER, duree_secondes INTEGER DEFAULT 0, duree_allouee INTEGER DEFAULT 60, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS tarifs (id INTEGER PRIMARY KEY, nom TEXT, type TEXT, prix INTEGER, duree_minutes INTEGER, description TEXT, actif INTEGER DEFAULT 1, console_type TEXT, jeu TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS jeux (id INTEGER PRIMARY KEY, titre TEXT, genre TEXT, console_id INTEGER, actif INTEGER DEFAULT 1, jaquette_url TEXT, image_url TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS factures (id INTEGER PRIMARY KEY, numero_facture TEXT UNIQUE, session_id INTEGER, joueur_id INTEGER, montant_ht REAL, taux_tva REAL DEFAULT 20, montant_tva REAL, montant_ttc REAL, mode_paiement TEXT, statut TEXT, date_paiement TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS jetons_transactions (id INTEGER PRIMARY KEY, joueur_id INTEGER, quantite INTEGER, type TEXT, raison TEXT, session_id INTEGER, facture_id INTEGER, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, titre TEXT, contenu TEXT, auteur TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS parametres_fidelite (id INTEGER PRIMARY KEY, regle_type TEXT, seuil INTEGER, jetons_attribues INTEGER, valeur_jeton INTEGER DEFAULT 100, actif INTEGER DEFAULT 1, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS lignes_facture (id INTEGER PRIMARY KEY, facture_id INTEGER, description TEXT, quantite INTEGER, prix_unitaire INTEGER, total_ligne INTEGER, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS hangouts (id INTEGER PRIMARY KEY, titre TEXT, activite TEXT, prix INTEGER, actif INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT);

-- Moteur de sync offline-first (spec mod.md) — tables LOCALES, jamais synchronisées.
-- sync_outbox : journal des changements locaux en attente d'envoi vers Supabase.
CREATE TABLE IF NOT EXISTS sync_outbox (change_id TEXT PRIMARY KEY, device_id TEXT, device_sequence INTEGER, operation TEXT, table_name TEXT, record_id INTEGER, payload TEXT, status TEXT DEFAULT 'PENDING', created_at TEXT);
CREATE INDEX IF NOT EXISTS idx_outbox_status ON sync_outbox(status);
-- sync_state : position de sync de CET appareil (curseurs last_uploaded / last_received)
-- + identité appareil (mission §3) : nom lisible, date d'installation, statut d'activation.
CREATE TABLE IF NOT EXISTS sync_state (
    device_id TEXT PRIMARY KEY,
    device_sequence INTEGER DEFAULT 0,
    last_uploaded TEXT,
    last_received TEXT,
    last_sync_at TEXT,
    device_name TEXT DEFAULT '',
    installation_id TEXT DEFAULT '',
    created_at TEXT DEFAULT '',
    activated INTEGER DEFAULT 0
);
-- sync_conflicts : journal des conflits détectés pendant le pull delta (audit, spec §9).
CREATE TABLE IF NOT EXISTS sync_conflicts (id INTEGER PRIMARY KEY, change_id TEXT, entity TEXT, record_id INTEGER, reason TEXT, remote_payload TEXT, resolved INTEGER DEFAULT 0, created_at TEXT);
"#;

/// Migration soft-delete : ajoute la colonne `deleted` aux bases existantes.
/// (Supabase ne supprime JAMAIS physiquement : une ligne supprimée passe deleted=1,
/// est cachée des listes, et la suppression se propage par la sync.)
const SOFT_DELETE_MIGRATION: &str = "\
ALTER TABLE users ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE consoles ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE jeux ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE joueurs ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE sessions_jeu ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE tarifs ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE factures ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE jetons_transactions ADD COLUMN deleted INTEGER DEFAULT 0; \nALTER TABLE jetons_transactions ADD COLUMN facture_id INTEGER; \
ALTER TABLE messages ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE parametres_fidelite ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE lignes_facture ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE hangouts ADD COLUMN deleted INTEGER DEFAULT 0;";

/// Visuels des entités (item 6) + fix utilisateurs fantômes : colonnes
/// sticker/image_url et index unique partiel sur les utilisateurs vivants.
const IMAGE_COLUMNS_MIGRATION: &str = "\
ALTER TABLE joueurs ADD COLUMN sticker TEXT; \
ALTER TABLE consoles ADD COLUMN image_url TEXT; \
ALTER TABLE jeux ADD COLUMN image_url TEXT; \
DROP INDEX IF EXISTS idx_users_email_alive; \
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_alive ON users(email) WHERE COALESCE(deleted, 0) = 0;";

const TOKEN_VALUE_MIGRATION: &str = "ALTER TABLE parametres_fidelite ADD COLUMN valeur_jeton INTEGER DEFAULT 100;";

/// Base SQLite partagée + dirty set des tables modifiées localement.
/// Une table ABSENTE du dirty set est considérée dirty (première sync = push complet).
/// Le 3e champ est le canal de réveil : chaque écriture locale prévient le worker de
/// sync pour pousser le changement INSTANTANÉMENT (si sync_enabled est actif).
pub struct Db(
    pub Mutex<Connection>,
    pub Mutex<std::collections::HashMap<String, bool>>,
    pub Mutex<Option<tokio::sync::mpsc::UnboundedSender<()>>>,
    pub Mutex<Option<crate::db_notify::ChangeCallback>>,
    /// device_id mis en cache au boot (évite un aller-retour SQLite par notification).
    pub Mutex<Option<String>>,
);

/// Horodatage ISO 8601 avec millisecondes, comme `new Date().toISOString()` en JS.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Date du jour au format YYYYMMDD (pour les numéros de facture).
pub fn today_str() -> String {
    chrono::Utc::now().format("%Y%m%d").to_string()
}

/// Session actuellement EN COURS (pour le watcher d'expiration) :
/// (id, console_id, duree_allouee_en_minutes, secondes_deja_accumulees).
/// None si aucune session active.
pub fn find_active_session(
    conn: &rusqlite::Connection,
) -> Option<(i64, Option<i64>, i64, i64)> {
    conn.query_row(
        "SELECT id, console_id, COALESCE(duree_allouee, duree_minutes, 60), COALESCE(duree_secondes, duree_minutes * 60, 0) \
         FROM sessions_jeu WHERE statut = 'en_cours' AND COALESCE(deleted, 0) = 0 \
         ORDER BY debut ASC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .ok()
}

// Alphabet Crockford base32 (ULID) : pas de I, L, O, U pour éviter les confusions.
const CROCKFORD: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Tables dont les données sont protégées contre l'écrasement silencieux :
/// en cas de conflit local/distant, l'appareil LOCAL gagne toujours (spec §9).
const PROTECTED_TABLES: [&str; 3] = ["sessions_jeu", "factures", "jetons_transactions"];

fn is_protected(table: &str) -> bool {
    PROTECTED_TABLES.contains(&table)
}

/// True si la valeur `deleted` d'une ligne indique une suppression.
/// Gère les DEUX types : boolean (true/false) et integer (1/0).
fn is_deleted_value(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) == 1,
        _ => false,
    }
}

/// ULID maison : 48 bits de timestamp (ms) + 80 bits aléatoires, encodés en
/// base32 Crockford (26 caractères). Triable chronologiquement et unique par
/// appareil — aucun identifiant centralisé, aucune dépendance ajoutée
/// (rand est déjà utilisé par auth.rs pour les sels).
pub fn ulid() -> String {
    let ts = chrono::Utc::now().timestamp_millis() as u64;
    let mut rnd = [0u8; 10];
    rand::thread_rng().fill_bytes(&mut rnd);
    let mut b = [0u8; 16];
    b[0] = (ts >> 40) as u8;
    b[1] = (ts >> 32) as u8;
    b[2] = (ts >> 24) as u8;
    b[3] = (ts >> 16) as u8;
    b[4] = (ts >> 8) as u8;
    b[5] = ts as u8;
    for i in 0..10 {
        b[6 + i] = rnd[i];
    }
    let mut out = String::with_capacity(26);
    let mut val: u64 = 0;
    let mut nbits = 0;
    let mut idx = 0;
    for _ in 0..26 {
        while nbits < 5 {
            // 128 bits utiles + 2 bits de padding (comportement ULID standard)
            let byte = if idx < 16 { b[idx] as u64 } else { 0u64 };
            val = (val << 8) | byte;
            nbits += 8;
            idx += 1;
        }
        let shift = nbits - 5;
        out.push(CROCKFORD.as_bytes()[(val >> shift) as usize & 31] as char);
        nbits -= 5;
        val = val & ((1 << nbits) - 1);
    }
    out
}

fn row_to_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let stmt: &rusqlite::Statement = row.as_ref();
    let count = stmt.column_count();
    let mut map = Map::new();
    for i in 0..count {
        let name = stmt.column_name(i)?.to_string();
        let v = match row.get_ref(i)? {
            rusqlite::types::ValueRef::Null => Value::Null,
            rusqlite::types::ValueRef::Integer(n) => json!(n),
            rusqlite::types::ValueRef::Real(f) => json!(f),
            rusqlite::types::ValueRef::Text(t) => {
                Value::String(String::from_utf8_lossy(t).to_string())
            }
            rusqlite::types::ValueRef::Blob(_) => Value::Null,
        };
        map.insert(name, v);
    }
    Ok(Value::Object(map))
}

fn bind_value(params: &mut Vec<Box<dyn rusqlite::types::ToSql>>, v: &Value) {
    match v {
        Value::Null => params.push(Box::new(rusqlite::types::Null)),
        Value::Bool(b) => params.push(Box::new(if *b { 1i64 } else { 0i64 })),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                params.push(Box::new(i));
            } else if let Some(u) = n.as_u64() {
                params.push(Box::new(u as i64));
            } else if let Some(f) = n.as_f64() {
                params.push(Box::new(f));
            } else {
                params.push(Box::new(""));
            }
        }
        Value::String(s) => params.push(Box::new(s.clone())),
        _ => params.push(Box::new("")),
    }
}

fn apply_pragmas(conn: &Connection) {
    // WAL peut échouer sur certains FS Android (exFAT, SD) -> fallback silencieux
    // On essaye WAL, sinon on reste en DELETE (sûr partout)
    // busy_timeout évite "database is locked" sur flash lente low-end
    let pragmas = [
        "PRAGMA busy_timeout=5000;",
        "PRAGMA synchronous=NORMAL;",
        "PRAGMA journal_mode=WAL;",
        "PRAGMA cache_size=-8192;", // 8MB cache, évite OOM sur device 1GB
        "PRAGMA temp_store=MEMORY;",
        "PRAGMA foreign_keys=ON;",
    ];
    for sql in pragmas {
        let _ = conn.execute_batch(sql);
    }
    // Vérifie que WAL est bien actif, sinon force DELETE+NORMAL (pas de crash)
    if let Ok(mut stmt) = conn.prepare("PRAGMA journal_mode;") {
        if let Ok(jmode) = stmt.query_row([], |r| r.get::<_, String>(0)) {
            if jmode.to_uppercase() != "WAL" {
                let _ = conn.execute_batch("PRAGMA journal_mode=DELETE;");
            }
        }
    }
    // Index légers pour accélérer les filtres fréquents (évite full scan sur device lent)
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sessions_statut ON sessions_jeu(statut);
         CREATE INDEX IF NOT EXISTS idx_sessions_joueur ON sessions_jeu(joueur_id);
         CREATE INDEX IF NOT EXISTS idx_factures_joueur ON factures(joueur_id);
         CREATE INDEX IF NOT EXISTS idx_jetons_joueur ON jetons_transactions(joueur_id);
         CREATE INDEX IF NOT EXISTS idx_lignes_facture ON lignes_facture(facture_id);
         CREATE INDEX IF NOT EXISTS idx_jeux_console ON jeux(console_id);",
    );
}

impl Db {
    pub fn open(path: &Path) -> ApiResult<Db> {
        let conn = Connection::open(path)
            .map_err(|e| ApiError::internal(format!("Ouverture SQLite: {e}")))?;
        // Détecte corruption au premier open (Android coupe brutalement l'alim)
        // Si la DB est corrompue, on tente un recovery via VACUUM ou on laisse l'appelant fallback en mémoire
        if let Err(e) = conn.execute_batch("PRAGMA quick_check;") {
            eprintln!("quick_check warning: {e}");
        }
        conn.execute_batch(SCHEMA)
            .map_err(|e| ApiError::internal(format!("Init schéma: {e}")))?;
        // Migration pour bases existantes créées avant ajout colonnes (évite crash "no such column" après update APK)
        let _ = conn.execute_batch(
            "ALTER TABLE consoles ADD COLUMN created_at TEXT; \
             ALTER TABLE consoles ADD COLUMN date_ajout TEXT; \
             ALTER TABLE joueurs ADD COLUMN derniere_visite TEXT; \
             ALTER TABLE sessions_jeu ADD COLUMN tarif_id INTEGER;",
        );
        // Soft-delete : colonne deleted sur les bases existantes (idempotent, erreurs ignorées)
        let _ = conn.execute_batch(
            "ALTER TABLE sync_state ADD COLUMN device_name TEXT DEFAULT '';              ALTER TABLE sync_state ADD COLUMN installation_id TEXT DEFAULT '';              ALTER TABLE sync_state ADD COLUMN created_at TEXT DEFAULT '';              ALTER TABLE sync_state ADD COLUMN activated INTEGER DEFAULT 0;"
        );
        for stmt in SOFT_DELETE_MIGRATION.split(';') {
            let s = stmt.trim();
            if !s.is_empty() {
                let _ = conn.execute(s, []);
            }
        }
        let _ = conn.execute("ALTER TABLE jetons_transactions ADD COLUMN facture_id INTEGER;", []);
        let _ = conn.execute("ALTER TABLE jetons_transactions ADD COLUMN deleted INTEGER DEFAULT 0;", []);
        // Colonnes visuels (sticker joueur, image console/jeu) + index unique
        // partiel sur users (fix utilisateurs fantômes). Idempotent.
        let _ = conn.execute_batch(IMAGE_COLUMNS_MIGRATION);
        let _ = conn.execute_batch(TOKEN_VALUE_MIGRATION);
        // Durée allouée par le tarif (sessions créées avant cette version) :
        // c'est elle qui déclenche l'expiration automatique. Idempotent.
        let _ = conn.execute_batch("ALTER TABLE sessions_jeu ADD COLUMN duree_allouee INTEGER DEFAULT 60;");
        // Précision SECONDE du temps joué (duree_minutes reste pour compat sync) :
        // les arrondis à la minute faussaient progressivement le chrono.
        let _ = conn.execute_batch("ALTER TABLE sessions_jeu ADD COLUMN duree_secondes INTEGER DEFAULT 0;");
        // NORMALISATION UNIQUE : les sessions actives créées par l'ANCIEN code
        // avaient la durée du tarif pré-remplie dans duree_minutes (accumulator).
        // Au nouveau modèle, duree_minutes = temps déjà joué -> on remet à zéro
        // pour que le watcher n'expire pas instantanément ces sessions.
        let _ = conn.execute_batch(
            "UPDATE sessions_jeu SET duree_minutes = 0 WHERE statut IN ('en_cours','pause') AND (SELECT COUNT(*) FROM app_settings WHERE key = 'sessions_accum_reset_v1') = 0; \
             INSERT OR REPLACE INTO app_settings (key, value) VALUES ('sessions_accum_reset_v1', '1');",
        );
        // Seed des secondes pour les sessions actives créées avant cette colonne.
        let _ = conn.execute_batch(
            "UPDATE sessions_jeu SET duree_secondes = duree_minutes * 60 WHERE statut IN ('en_cours','pause') AND (SELECT COUNT(*) FROM app_settings WHERE key = 'sessions_seconds_seed_v1') = 0; \
             INSERT OR REPLACE INTO app_settings (key, value) VALUES ('sessions_seconds_seed_v1', '1');",
        );
        apply_pragmas(&conn);
        // Test écriture immédiate pour détecter disque plein / permission early
        let _ = conn.execute_batch("CREATE TABLE IF NOT EXISTS __healthcheck (id INTEGER PRIMARY KEY); DROP TABLE IF EXISTS __healthcheck;");
        Ok(Db(Mutex::new(conn), Mutex::new(std::collections::HashMap::new()), Mutex::new(None), Mutex::new(None), Mutex::new(None)))
    }

    /// Fallback en mémoire si le fichier est inaccessible (permissions Android, disque plein).
    /// Permet à l'app de démarrer et d'afficher un message au lieu de crasher.
    pub fn open_in_memory() -> ApiResult<Db> {
        let conn = Connection::open_in_memory()
            .map_err(|e| ApiError::internal(format!("Ouverture SQLite mémoire: {e}")))?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| ApiError::internal(format!("Init schéma mémoire: {e}")))?;
        let _ = conn.execute_batch(
            "ALTER TABLE consoles ADD COLUMN created_at TEXT; \
             ALTER TABLE consoles ADD COLUMN date_ajout TEXT; \
             ALTER TABLE joueurs ADD COLUMN derniere_visite TEXT; \
             ALTER TABLE sessions_jeu ADD COLUMN tarif_id INTEGER;",            );            let _ = conn.execute_batch(
            "ALTER TABLE sync_state ADD COLUMN device_name TEXT DEFAULT '';              ALTER TABLE sync_state ADD COLUMN installation_id TEXT DEFAULT '';              ALTER TABLE sync_state ADD COLUMN created_at TEXT DEFAULT '';              ALTER TABLE sync_state ADD COLUMN activated INTEGER DEFAULT 0;"
        );
        for stmt in SOFT_DELETE_MIGRATION.split(';') {
            let s = stmt.trim();
            if !s.is_empty() {
                let _ = conn.execute(s, []);
            }
        }
        let _ = conn.execute("ALTER TABLE jetons_transactions ADD COLUMN facture_id INTEGER;", []);
        let _ = conn.execute("ALTER TABLE jetons_transactions ADD COLUMN deleted INTEGER DEFAULT 0;", []);
        let _ = conn.execute_batch(IMAGE_COLUMNS_MIGRATION);
        let _ = conn.execute_batch(TOKEN_VALUE_MIGRATION);
        let _ = conn.execute_batch("ALTER TABLE sessions_jeu ADD COLUMN duree_allouee INTEGER DEFAULT 60;");
        let _ = conn.execute_batch("ALTER TABLE sessions_jeu ADD COLUMN duree_secondes INTEGER DEFAULT 0;");
        let _ = conn.execute_batch(
            "UPDATE sessions_jeu SET duree_minutes = 0 WHERE statut IN ('en_cours','pause') AND (SELECT COUNT(*) FROM app_settings WHERE key = 'sessions_accum_reset_v1') = 0; \
             INSERT OR REPLACE INTO app_settings (key, value) VALUES ('sessions_accum_reset_v1', '1');",
        );
        let _ = conn.execute_batch(
            "UPDATE sessions_jeu SET duree_secondes = duree_minutes * 60 WHERE statut IN ('en_cours','pause') AND (SELECT COUNT(*) FROM app_settings WHERE key = 'sessions_seconds_seed_v1') = 0; \
             INSERT OR REPLACE INTO app_settings (key, value) VALUES ('sessions_seconds_seed_v1', '1');",
        );
        apply_pragmas(&conn);
        Ok(Db(Mutex::new(conn), Mutex::new(std::collections::HashMap::new()), Mutex::new(None), Mutex::new(None), Mutex::new(None)))
    }

    /// `SELECT * FROM {table}` avec filtre WHERE optionnel (comme le backend JS).
    /// Gère le poison du Mutex sans panic (Android peut tuer le thread)
    fn query_all_impl(&self, table: &str, filter: &str) -> ApiResult<Vec<Value>> {
        // Sur Android, le Mutex peut être poisoned si un panic a eu lieu lors d'une transaction
        // On récupère quand même l'intérieur pour éviter crash définitif
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                eprintln!("DB Mutex poisoned for {table}, recovering");
                poisoned.into_inner()
            }
        };
        // Validation nom de table pour éviter injection (bien que table vienne du code, pas de l'utilisateur)
        if !TABLES.contains(&table) {
            return Err(ApiError::internal(format!("Table inconnue: {table}")));
        }
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM \"{table}\"{filter}"))
            .map_err(|e| {
                // Sur Android low-end, "database disk image is malformed" arrive après coupure batterie
                if e.to_string().contains("malformed") || e.to_string().contains("corrupt") {
                    eprintln!("DB corruption detected on {table}: {e}");
                }
                ApiError::internal(format!("Requête {table}: {e}"))
            })?;
        let rows = stmt
            .query_map([], |r| row_to_value(r))
            .map_err(|e| ApiError::internal(format!("Requête {table}: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApiError::internal(format!("Requête {table}: {e}")))?;
        Ok(rows)
    }

    /// Liste PAR DÉFAUT : exclut les lignes soft-deleted (deleted=1) pour que
    /// l'utilisateur ne voie jamais une donnée supprimée (sur l'app comme après sync).
    pub fn query_all(&self, table: &str) -> ApiResult<Vec<Value>> {
        self.query_all_impl(table, " WHERE deleted = 0")
    }

    /// Liste TOUTES les lignes, y compris soft-deleted — utilisé par la sync pour
    /// propager les suppressions vers Supabase (tombstones).
    pub fn query_all_all(&self, table: &str) -> ApiResult<Vec<Value>> {
        self.query_all_impl(table, "")
    }

    /// Colonnes de la table locale (PRAGMA table_info) pour filtrer les données venues
    /// de Supabase : si Supabase a des colonnes en plus (ou des noms différents), l'insert
    /// échouait silencieusement avec "no such column" -> sync "OK" mais rien d'écrit.
    pub fn columns(&self, table: &str) -> ApiResult<Vec<String>> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if !TABLES.contains(&table) {
            return Err(ApiError::internal(format!("Table inconnue: {table}")));
        }
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .map_err(|e| ApiError::internal(format!("Colonnes {table}: {e}")))?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| ApiError::internal(format!("Colonnes {table}: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApiError::internal(format!("Colonnes {table}: {e}")))?;
        Ok(cols)
    }

    /// Lit un paramètre applicatif (ex: sync_enabled) — table app_settings.
    pub fn get_setting(&self, key: &str) -> ApiResult<Option<String>> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut stmt = conn
            .prepare("SELECT value FROM app_settings WHERE key = ?")
            .map_err(|e| ApiError::internal(format!("get_setting {key}: {e}")))?;
        let rows = stmt
            .query_map([key], |r| r.get::<_, String>(0))
            .map_err(|e| ApiError::internal(format!("get_setting {key}: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApiError::internal(format!("get_setting {key}: {e}")))?;
        Ok(rows.iter().next().map(|s| s.to_string()))
    }

    /// Écrit un paramètre applicatif (upsert).
    pub fn set_setting(&self, key: &str, value: &str) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        conn.execute("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)", [key, value])
            .map_err(|e| ApiError::internal(format!("set_setting {key}: {e}")))?;
        Ok(())
    }

    /// Marque une table comme modifiée localement -> à re-pousser vers Supabase.
    /// Appelé par insert/update/remove : aucune écriture locale ne doit être oubliée.
    /// RÉVEILLE aussi le worker de sync : si la sync est activée, le changement part
    /// INSTANTANÉMENT vers Supabase (pas d'attente du cycle de 60s).
    pub fn mark_dirty(&self, table: &str) {
        if let Ok(mut guard) = self.1.lock() {
            guard.insert(table.to_string(), true);
        }
        self.wake_sync();
    }

    /// Branche le canal de réveil (appelé une fois au boot après la création de l'AppState).
    pub fn set_sync_waker(&self, tx: tokio::sync::mpsc::UnboundedSender<()>) {
        if let Ok(mut guard) = self.2.lock() {
            *guard = Some(tx);
        }
    }

    /// Signal non-bloquant vers le worker de sync. Jamais d'erreur si le canal est
    /// absent (tests) ou déjà plein (réveil en attente = le worker va tourner).
    pub fn wake_sync(&self) {
        if let Ok(guard) = self.2.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(());
            }
        }
    }

    /// Nombre de changements locaux en attente d'envoi (statut PENDING).
    /// Sert de badge de sync dans l'UI et de garde-fou pour le worker.
    pub fn outbox_pending_count(&self) -> ApiResult<usize> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM sync_outbox WHERE status = 'PENDING'")
            .map_err(|e| ApiError::internal(format!("outbox count: {e}")))?;
        let n = stmt
            .query_row([], |r| r.get::<_, i64>(0))
            .map_err(|e| ApiError::internal(format!("outbox count: {e}")))?;
        Ok(n as usize)
    }

    /// Événements en attente, triés par device_sequence croissant (ordre FIFO).
    /// Consommé par le worker d'envoi (étape 2) et par le test.
    pub fn outbox_pending(&self, limit: usize) -> ApiResult<Vec<Value>> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut stmt = conn
            .prepare(
                "SELECT change_id, device_id, device_sequence, operation, table_name, record_id, payload, status, created_at FROM sync_outbox WHERE status = 'PENDING' ORDER BY device_sequence ASC LIMIT ?",
            )
            .map_err(|e| ApiError::internal(format!("outbox: {e}")))?;
        let rows = stmt
            .query_map([limit as i64], |r| row_to_value(r))
            .map_err(|e| ApiError::internal(format!("outbox: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApiError::internal(format!("outbox: {e}")))?;
        Ok(rows)
    }

    // ===== MOTEUR DELTA SYNC (étapes 2-4 de ~/mod.md) =====

    /// device_id stable de l'appareil (généré une fois, persisté).
    pub fn device_id(&self) -> ApiResult<String> {
        // Cache mémoire : les notifications l'appellent souvent, pas de
        // aller-retour SQLite à chaque fois.
        if let Ok(g) = self.4.lock() {
            if let Some(id) = g.clone() {
                return Ok(id);
            }
        }
        let id = {
            let conn = match self.0.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            self.device_id_conn(&*conn)?
        };
        if let Ok(mut g) = self.4.lock() {
            *g = Some(id.clone());
        }
        Ok(id)
    }

    /// Identité complète de l'appareil (mission §3) : device_id stable, nom,
    /// installation_id, created_at, statut d'activation. Tout est persisté dans
    /// sync_state (même ligne que les curseurs de sync) : stable entre
    /// redémarrages, régénéré seulement si la base disparaît (désinstallation).
    pub fn device_identity(&self) -> ApiResult<Value> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let did = self.device_id_conn(&*conn)?;
        let mut stmt = conn
            .prepare(
                "SELECT device_name, installation_id, created_at, activated FROM sync_state WHERE device_id = ?",
            )
            .map_err(|e| ApiError::internal(format!("device_identity: {e}")))?;
        let row = stmt
            .query_row([&did], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(|e| ApiError::internal(format!("device_identity read: {e}")));
        match row {
            Ok((name, inst, created, activated)) => Ok(json!({
                "device_id": did,
                "device_name": name.unwrap_or_default(),
                "installation_id": inst.unwrap_or_default(),
                "created_at": created.unwrap_or_default(),
                "activated": activated.unwrap_or(0) == 1,
            })),
            Err(_) => {
                // Ligne absente (DB in-memory de test) : recrée le device_id puis renvoie l'identité minimale.
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO sync_state (device_id, device_sequence, last_uploaded, last_received, last_sync_at, created_at, activated) VALUES (?1, 0, '', '', '', ?2, 1)",
                    rusqlite::params![did, now_iso()],
                );
                Ok(json!({
                    "device_id": did,
                    "device_name": String::new(),
                    "installation_id": String::new(),
                    "created_at": now_iso(),
                    "activated": true,
                }))
            }
        }
    }

    /// Nom lisible de l'appareil (défaut : "Game Lounge <suffixe device_id>").
    /// Défini une seule fois (premier appel) puis conservé.
    pub fn device_set_name(&self, name: &str) -> ApiResult<String> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let did = self.device_id_conn(&*conn)?;
        let current: Option<String> = conn
            .query_row(
                "SELECT device_name FROM sync_state WHERE device_id = ?",
                [&did],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);
        let final_name = match current {
            Some(n) if !n.is_empty() => n, // déjà nommé : on ne change pas
            _ => {
                let n = if name.trim().is_empty() {
                    let suffix = did.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>();
                    format!("Game Lounge {suffix}")
                } else {
                    name.trim().to_string()
                };
                conn.execute(
                    "UPDATE sync_state SET device_name = ? WHERE device_id = ?",
                    rusqlite::params![n, did],
                )
                .map_err(|e| ApiError::internal(format!("device_set_name: {e}")))?;
                n
            }
        };
        Ok(final_name)
    }

    /// Reprise après coupure/crash : les événements SENDING sans ACK repassent
    /// PENDING (rejoués au cycle suivant — l'upsert cloud est idempotent, §6-7).
    pub fn outbox_reset_stale(&self) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        conn.execute_batch("UPDATE sync_outbox SET status='PENDING' WHERE status='SENDING'")
            .map_err(|e| ApiError::internal(format!("outbox reset: {e}")))?;
        Ok(())
    }

    /// Marque des événements comme ACKED (confirmés par Supabase) ou FAILED.
    pub fn outbox_mark(&self, ids: &Vec<String>, status: &str) -> ApiResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let placeholders: Vec<&str> = vec!["?"; ids.len()];
        let q = format!(
            "UPDATE sync_outbox SET status=? WHERE change_id IN ({})",
            placeholders.join(",")
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(ids.len() + 1);
        params.push(Box::new(status));
        for id in ids {
            params.push(Box::new(id.clone()));
        }
        conn.execute(&q, rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("outbox mark: {e}")))?;
        Ok(())
    }

    /// Purge : ne garde que les 500 derniers ACKED — l'historique complet reste
    /// dans sync_changes côté Supabase, l'outbox locale n'en a plus besoin (§12).
    pub fn outbox_cleanup(&self) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        conn.execute_batch(
            "DELETE FROM sync_outbox WHERE status='ACKED' AND change_id NOT IN (SELECT change_id FROM sync_outbox WHERE status='ACKED' ORDER BY device_sequence DESC LIMIT 500)",
        )
        .map_err(|e| ApiError::internal(format!("outbox cleanup: {e}")))?;
        Ok(())
    }

    /// Seed initial : journalise TOUTES les lignes locales existantes dans
    /// l'outbox (première sync d'une base pré-existante — ces données n'ont
    /// jamais été journalisées). Idempotent côté Supabase (upsert + ON CONFLICT).
    pub fn outbox_seed(&self) -> ApiResult<usize> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let tx = rusqlite::Transaction::new_unchecked(
            &*conn,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(|e| ApiError::internal(format!("BEGIN seed: {e}")))?;
        let mut total = 0;
        for table in TABLES {
            let mut stmt = tx
                .prepare(&format!("SELECT * FROM \"{table}\""))
                .map_err(|e| ApiError::internal(format!("seed {table}: {e}")))?;
            let rows = stmt
                .query_map([], |r| row_to_value(r))
                .map_err(|e| ApiError::internal(format!("seed {table}: {e}")))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| ApiError::internal(format!("seed {table}: {e}")))?;
            drop(stmt);
            for row in rows {
                let Some(obj) = row.as_object() else { continue };
                let Some(id_v) = obj.get("id") else { continue };
                let id = id_v.as_i64().unwrap_or(0);
                if id == 0 {
                    continue;
                }
                self.enqueue_outbox(&*tx, "INSERT", table, id, &row)?;
                total += 1;
            }
        }
        tx.commit()
            .map_err(|e| ApiError::internal(format!("Commit seed: {e}")))?;
        Ok(total)
    }

    /// Curseur de réception : dernière séquence reçue du cloud (0 = jamais syncé).
    /// IMPORTANT : `last_received` est une colonne TEXT (héritage ISO strings) ; la
    /// lire comme i64 faisait échouer le décodage rusqlite -> l'erreur était avalée
    /// par unwrap_or(0) partout -> le curseur restait à 0 -> PULL COMPLET à CHAQUE
    /// sync (téléchargement de toute la base = le vrai responsable des timeouts
    /// réseau/IPC après login). On CAST explicitement en INTEGER et on prend MAX()
    /// sur la valeur numérique (MAX textuel comparait "9" > "10").
    pub fn sync_cursor_get(&self) -> ApiResult<i64> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut stmt = conn
            .prepare("SELECT COALESCE(MAX(CAST(last_received AS INTEGER)), 0) FROM sync_state")
            .map_err(|e| ApiError::internal(format!("cursor get: {e}")))?;
        let n = stmt
            .query_row([], |r| r.get::<_, i64>(0))
            .map_err(|e| ApiError::internal(format!("cursor get: {e}")))?;
        Ok(n)
    }

    pub fn sync_cursor_set(&self, n: i64) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let device = self.device_id_conn(&*conn)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(n.to_string()), Box::new(device)];
        conn.execute("UPDATE sync_state SET last_received = ? WHERE device_id = ?", rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("cursor set: {e}")))?;
        Ok(())
    }

    pub fn sync_uploaded_set(&self, n: i64) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let device = self.device_id_conn(&*conn)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(n), Box::new(device)];
        conn.execute("UPDATE sync_state SET last_uploaded = ? WHERE device_id = ?", rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("uploaded set: {e}")))?;
        Ok(())
    }

    /// Un changement local PENDING existe-t-il pour cette entité ? (détection conflit)
    fn outbox_pending_for_conn(&self, conn: &Connection, table: &str, id: i64) -> ApiResult<bool> {
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM sync_outbox WHERE status='PENDING' AND table_name=? AND record_id=?")
            .map_err(|e| ApiError::internal(format!("pending for: {e}")))?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(table), Box::new(id)];
        let n = stmt
            .query_row(rusqlite::params_from_iter(params), |r| r.get::<_, i64>(0))
            .map_err(|e| ApiError::internal(format!("pending for: {e}")))?;
        Ok(n > 0)
    }

    /// Variante sans re-lock : à utiliser quand la connexion est DÉJÀ verrouillée
    /// (à l'intérieur de apply_remote_change, sinon deadlock).
    fn conflict_log_conn(
        &self,
        conn: &Connection,
        change_id: &str,
        entity: &str,
        record_id: i64,
        reason: &str,
        remote_payload: &str,
    ) -> ApiResult<()> {
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(change_id),
            Box::new(entity),
            Box::new(record_id),
            Box::new(reason),
            Box::new(remote_payload),
            Box::new(now_iso()),
        ];
        conn.execute(
            "INSERT INTO sync_conflicts (change_id, entity, record_id, reason, remote_payload, resolved, created_at) VALUES (?, ?, ?, ?, ?, 0, ?)",
            rusqlite::params_from_iter(params),
        )
        .map_err(|e| ApiError::internal(format!("conflict log: {e}")))?;
        Ok(())
    }

    /// Colonnes locales (PRAGMA) — variante sans re-lock (appelable en transaction).
    fn columns_conn(&self, conn: &Connection, table: &str) -> ApiResult<Vec<String>> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .map_err(|e| ApiError::internal(format!("Colonnes {table}: {e}")))?;
        let cols = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| ApiError::internal(format!("Colonnes {table}: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApiError::internal(format!("Colonnes {table}: {e}")))?;
        Ok(cols)
    }

    /// Applique un changement VENU DU CLOUD (pull delta) SANS journaliser dans
    /// l'outbox (sinon boucle infinie) ni marquer la table dirty.
    /// Retourne : "applied" | "skipped" (identique) | "tombstone" | "conflict" | "ignored".
    pub fn apply_remote_change(
        &self,
        entity: &str,
        record_id: i64,
        payload: &Map<String, Value>,
        remote_change_id: &str,
        remote_created_at: &str,
    ) -> ApiResult<&str> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        // 1) CONFLIT : un changement local non envoyé existe pour cette entité ?
        if self.outbox_pending_for_conn(&*conn, entity, record_id).unwrap_or(false) {
            // Règle par type de donnée (spec §9) : les données protégées gardent
            // TOUJOURS le local ; les autres comparent l'horodatage (plus récent gagne).
            let local_wins = is_protected(entity)
                || payload
                    .get("created_at")
                    .and_then(Value::as_str)
                    .map(|c| remote_created_at.to_string() < c.to_string())
                    .unwrap_or(false);
            let reason = if local_wins { "local_wins" } else { "remote_wins" };
            self.conflict_log_conn(
                &*conn,
                remote_change_id,
                entity,
                record_id,
                reason,
                &serde_json::to_string(payload).unwrap_or_default(),
            )
            .map_err(|e| ApiError::internal(format!("conflict log: {e}")))?;
            if local_wins {
                return Ok("conflict");
            }
            // sinon : le distant est plus récent -> on applique
        }
        // 2) TOMBSTONE : suppression distante (spec §8)
        if payload.get("deleted").map(is_deleted_value).unwrap_or(false) {
            let touched = conn
                .execute(&format!("UPDATE \"{entity}\" SET deleted=1 WHERE id=?"), [record_id])
                .map_err(|e| ApiError::internal(format!("tombstone {entity}: {e}")))?;
            if touched == 0 {
                // La ligne n'existe pas encore ici (appareil neuf, ou archivage
                // effectué avant la première sync) : on la crée DIRECTEMENT
                // archivée. Sinon l'archive resterait invisible sur cet appareil
                // et ne pourrait jamais être restaurée.
                let local_cols = self.columns_conn(&*conn, entity).unwrap_or_default();
                let mut map = serde_json::Map::new();
                for (k, v) in payload {
                    if *k == "id" || !local_cols.contains(k) {
                        continue;
                    }
                    if entity == "users" && *k == "password_hash" {
                        continue;
                    }
                    map.insert(k.clone(), v.clone());
                }
                if !map.is_empty() {
                    map.insert("id".into(), json!(record_id));
                    map.insert("deleted".into(), json!(1));
                    let keys: Vec<&String> = map.keys().collect();
                    let cols: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
                    let placeholders: Vec<&str> = vec!["?"; cols.len()];
                    let q = format!(
                        "INSERT OR IGNORE INTO \"{entity}\" ({}) VALUES ({})",
                        cols.join(","),
                        placeholders.join(",")
                    );
                    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
                        Vec::with_capacity(cols.len());
                    for k in keys {
                        bind_value(&mut params, &map[k]);
                    }
                    // INSERT OR IGNORE : une contrainte UNIQUE (email déjà pris
                    // par un autre compte) ne doit pas faire échouer toute la sync.
                    let _ = conn.execute(&q, rusqlite::params_from_iter(params));
                }
            }
            return Ok("tombstone");
        }
        // 3) INSERT/UPDATE avec skip-si-identique (zéro fsync inutile)
        let local_cols = self.columns_conn(&*conn, entity).unwrap_or_default();
        let mut map = serde_json::Map::new();
        let mut upd = serde_json::Map::new();
        for (k, v) in payload {
            if *k == "id" || !local_cols.contains(k) {
                continue;
            }
            // Le hash local reste LA référence pour le login (algos incompatibles)
            if entity == "users" && *k == "password_hash" {
                continue;
            }
            map.insert(k.clone(), v.clone());
            upd.insert(k.clone(), v.clone());
        }
        if map.is_empty() {
            return Ok("ignored");
        }
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM \"{entity}\" WHERE id=?"))
            .map_err(|e| ApiError::internal(format!("apply get {entity}: {e}")))?;
        let mut rows = stmt
            .query_map([record_id], |r| row_to_value(r))
            .map_err(|e| ApiError::internal(format!("apply get {entity}: {e}")))?;
        match rows.next() {
            Some(Ok(local)) => {
                // Jamais ressusciter une ligne soft-deleted localement
                if local.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1 {
                    return Ok("ignored");
                }
                let mut same = true;
                for (k, v) in &upd {
                    let lv = local.get(k.as_str());
                    if lv.is_none()
                        || serde_json::to_string(&lv.unwrap()).unwrap_or_default()
                            != serde_json::to_string(v).unwrap_or_default()
                    {
                        same = false;
                        break;
                    }
                }
                if same {
                    return Ok("skipped");
                }
                let sets: Vec<String> = upd.keys().map(|k| format!("{k}=?")).collect();
                let q = format!("UPDATE \"{entity}\" SET {} WHERE id=?", sets.join(","));
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(upd.len() + 1);
                for v in upd.values() {
                    bind_value(&mut params, v);
                }
                params.push(Box::new(record_id));
                conn.execute(&q, rusqlite::params_from_iter(params))
                    .map_err(|e| ApiError::internal(format!("apply update {entity}: {e}")))?;
                Ok("applied")
            }
            Some(Err(e)) => Err(ApiError::internal(format!("apply get {entity}: {e}"))),
            None => {
                map.insert("id".into(), json!(record_id));
                let keys: Vec<&String> = map.keys().collect();
                let cols: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
                let placeholders: Vec<&str> = vec!["?"; cols.len()];
                let q = format!(
                    "INSERT OR REPLACE INTO \"{entity}\" ({}) VALUES ({})",
                    cols.join(","),
                    placeholders.join(",")
                );
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(cols.len());
                for k in keys {
                    bind_value(&mut params, &map[k]);
                }
                conn.execute(&q, rusqlite::params_from_iter(params))
                    .map_err(|e| ApiError::internal(format!("apply insert {entity}: {e}")))?;
                Ok("applied")
            }
        }
    }

    pub fn find_one(&self, table: &str, pred: impl Fn(&Value) -> bool) -> ApiResult<Option<Value>> {
        Ok(self.query_all(table)?.into_iter().find(pred))
    }

    /// Comme find_one mais inclut les lignes soft-deleted (pour ne jamais les ressusciter).
    pub fn find_one_all(&self, table: &str, pred: impl Fn(&Value) -> bool) -> ApiResult<Option<Value>> {
        Ok(self.query_all_all(table)?.into_iter().find(pred))
    }

    /// device_id stable de l'appareil : généré une fois (ULID), persisté dans
    /// sync_state. Variante prenant la connexion déjà verrouillée pour être
    /// appelable DANS une transaction (l'écriture est alors atomique avec elle).
    fn device_id_conn(&self, conn: &Connection) -> ApiResult<String> {
        let mut stmt = conn
            .prepare("SELECT device_id FROM sync_state LIMIT 1")
            .map_err(|e| ApiError::internal(format!("device_id: {e}")))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| ApiError::internal(format!("device_id: {e}")))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| ApiError::internal(format!("device_id: {e}")))?;
        match rows.iter().next() {
            Some(s) => Ok(s.to_string()),
            None => {
                let id = ulid();
                conn.execute(
                    "INSERT INTO sync_state (device_id, device_sequence, last_uploaded, last_received, last_sync_at) VALUES (?, 0, '', '', '')",
                    [id.clone()],
                )
                .map_err(|e| ApiError::internal(format!("device_id insert: {e}")))?;
                Ok(id)
            }
        }
    }

    /// Journalise un changement local dans sync_outbox. À appeler DANS la même
    /// transaction que l'écriture de la donnée : si l'une échoue, l'autre aussi
    /// (atomicité donnée + événement de sync, exigée par la spec ~/mod.md).
    /// Le change_id est "{device_id}:{sequence}" : il identifie de façon unique
    /// et ordonnée chaque changement de cet appareil, même hors ligne.
    fn enqueue_outbox(
        &self,
        conn: &Connection,
        operation: &str,
        table: &str,
        record_id: i64,
        payload: &Value,
    ) -> ApiResult<()> {
        let device = self.device_id_conn(conn)?;
        conn.execute(
            "UPDATE sync_state SET device_sequence = device_sequence + 1 WHERE device_id = ?",
            [device.clone()],
        )
        .map_err(|e| ApiError::internal(format!("seq outbox: {e}")))?;
        let mut stmt = conn
            .prepare("SELECT device_sequence FROM sync_state WHERE device_id = ?")
            .map_err(|e| ApiError::internal(format!("seq outbox: {e}")))?;
        let seq = stmt
            .query_row([device.clone()], |r| r.get::<_, i64>(0))
            .map_err(|e| ApiError::internal(format!("seq outbox: {e}")))?;
        let change_id = format!("{device}:{seq}");
        let payload_str = serde_json::to_string(payload)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(change_id),
            Box::new(device),
            Box::new(seq),
            Box::new(operation),
            Box::new(table),
            Box::new(record_id),
            Box::new(payload_str),
            Box::new(now_iso()),
        ];
        conn.execute(
            "INSERT INTO sync_outbox (change_id, device_id, device_sequence, operation, table_name, record_id, payload, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, 'PENDING', ?)",
            rusqlite::params_from_iter(params),
        )
        .map_err(|e| ApiError::internal(format!("enqueue {table}: {e}")))?;
        Ok(())
    }

    /// Insère un enregistrement. Si un `id` est fourni, INSERT OR REPLACE (comme db.ts).
    /// L'écriture de la donnée ET l'événement sync_outbox sont dans la même
    /// transaction : aucune modification locale ne peut être perdue par la sync.
    /// Snapshot léger d'une ligne juste écrite (pour les notifications UI).
    /// Ne garde que les colonnes utiles à l'affichage — jamais de hash.
    fn snapshot_row(conn: &rusqlite::Connection, table: &str, id: i64) -> Value {
        let mut stmt = match conn.prepare(&format!("SELECT * FROM \"{table}\" WHERE id=?")) {
            Ok(s) => s,
            Err(_) => return Value::Null,
        };
        let mut rows = match stmt.query_map([id], |r| row_to_value(r)) {
            Ok(r) => r,
            Err(_) => return Value::Null,
        };
        let full = rows.next().and_then(|r| r.ok()).unwrap_or(Value::Null);
        let obj = match full.as_object() {
            Some(o) => o,
            None => return Value::Null,
        };
        let mut out = serde_json::Map::new();
        out.insert("id".into(), json!(id));
        for k in ["nom", "titre", "numero_facture", "email", "montant", "montant_ttc", "tarif_prix", "statut", "joueur_nom", "console_nom", "jeu_nom"] {
            if let Some(v) = obj.get(k) {
                out.insert(k.into(), v.clone());
            }
        }
        Value::Object(out)
    }

    /// Notifie les changements de données à l'UI (cloche du header). Appelé
    /// APRÈS commit — n'affecte jamais la transaction de données elle-même.
    fn notify_change(&self, table: &str, operation: &str, row: Value) {
        if let Some(tx) = self.2.lock().ok().and_then(|g| g.clone()) {
            let _ = tx.send(());
        }
        if let Some(cb) = self.3.lock().ok().and_then(|g| g.clone()) {
            cb(table, operation, row);
        }
    }

    /// Notification pour un changement appliqué par la sync delta (venu des
    /// AUTRES appareils) : lit le snapshot APRÈS écriture puis notifie l'UI.
    /// La sync initiale n'appelle PAS ceci (téléchargement massif, pas une
    /// actualité) — seul le delta produit des notifications.
    ///
    /// `change_id` (mission §10) : identifiant cloud unique du changement —
    /// sert à la déduplication (un même change re-téléchargé ne notifie pas 2×).
    /// `origin_device` : device_id de l'appareil AUTEUR du changement — si c'est
    /// NOUS, on ne se notifie pas (mission §10 : ne pas se notifier ses propres
    /// modifications), même si le changement nous revient via le journal cloud.
    pub fn notify_remote_change(&self, table: &str, operation: &str, id: i64, change_id: &str, origin_device: &str) {
        // Mission §10 : ne JAMAIS se notifier ses propres modifications. Le
        // changement peut nous revenir par le journal cloud (multi-appareils) :
        // si l'auteur est cet appareil, l'UI locale a déjà été notifiée à
        // l'écriture (notify_change dans insert/update/remove).
        let my_device = self.device_id().unwrap_or_default();
        if !origin_device.is_empty() && origin_device == my_device {
            return;
        }
        // Déduplication (mission §10) : un change_id déjà notifié est ignoré.
        // Journal en mémoire (ring buffer) : suffisant et sans I/O. Un redémarrage
        // vide le journal — au pire une notification est rejouée après reboot,
        // ce qui est acceptable (pas de doublon DANS une session).
        if let Ok(mut seen) = self.1.lock() {
            let key = format!("notif:{change_id}");
            if seen.contains_key(&key) {
                return;
            }
            seen.insert(key, true);
            // Ring buffer : garde les 500 derniers.
            if seen.len() > 700 {
                let keys: Vec<String> = seen.keys().take(seen.len() - 500).cloned().collect();
                for k in keys {
                    seen.remove(&k);
                }
            }
        }
        let snap = {
            let conn = match self.0.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            Self::snapshot_row(&*conn, table, id)
        };
        self.notify_change(table, operation, snap);
    }

    pub fn insert(&self, table: &str, data: &Map<String, Value>) -> ApiResult<Value> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let has_id = data.contains_key("id");
        // Id attribué ICI (pas par l'autoincrement SQLite) : voir
        // generate_distributed_id — indispensable pour que deux appareils hors
        // ligne ne créent jamais deux lignes différentes avec le même id.
        let mut data = data.clone();
        let mut generated_id: Option<i64> = None;
        if !has_id && table_has_id_column(&conn, table) {
            let device = self
                .4
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .or_else(|| self.device_id_conn(&conn).ok())
                .unwrap_or_else(|| "device-inconnu".to_string());
            let mut candidate = generate_distributed_id(&device);
            // Garde-fou : un id déjà pris (base héritée, horloge reculée) est
            // simplement décalé plutôt que de faire échouer l'insertion.
            for _ in 0..64 {
                let taken: i64 = conn
                    .query_row(
                        &format!("SELECT COUNT(1) FROM \"{table}\" WHERE id = ?1"),
                        rusqlite::params![candidate],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken == 0 {
                    break;
                }
                candidate += 1;
            }
            data.insert("id".into(), json!(candidate));
            generated_id = Some(candidate);
        }
        let data = &data;
        let keys: Vec<&String> = data.keys().collect();
        let cols: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
        let placeholders: Vec<&str> = vec!["?"; cols.len()];
        let q = if has_id {
            format!(
                "INSERT OR REPLACE INTO \"{table}\" ({}) VALUES ({})",
                cols.join(","),
                placeholders.join(",")
            )
        } else {
            format!(
                "INSERT INTO \"{table}\" ({}) VALUES ({})",
                cols.join(","),
                placeholders.join(",")
            )
        };
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(cols.len());
        for k in keys {
            bind_value(&mut params, &data[k]);
        }
        let tx = rusqlite::Transaction::new_unchecked(
            &*conn,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(|e| ApiError::internal(format!("BEGIN insert {table}: {e}")))?;
        tx.execute(&q, rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("Insert {table}: {e}")))?;
        // L'id explicite prévaut : last_insert_rowid() reste juste, mais on ne
        // dépend plus de lui pour les lignes synchronisées.
        let row_id = generated_id.unwrap_or_else(|| tx.last_insert_rowid());
        let mut out = data.clone();
        out.insert("id".into(), json!(row_id));
        self.enqueue_outbox(&*tx, "INSERT", table, row_id, &Value::Object(out.clone()))?;
        let snap = Self::snapshot_row(&*tx, table, row_id);
        tx.commit()
            .map_err(|e| ApiError::internal(format!("Commit insert {table}: {e}")))?;
        self.mark_dirty(table);
        self.notify_change(table, "INSERT", snap);
        Ok(Value::Object(out))
    }

    pub fn update(&self, table: &str, id: i64, updates: &Map<String, Value>) -> ApiResult<Value> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if updates.is_empty() {
            drop(conn);
            return self.get(table, id);
        }
        let sets: Vec<String> = updates.keys().map(|k| format!("{k}=?")).collect();
        let q = format!("UPDATE \"{table}\" SET {} WHERE id=?", sets.join(","));
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(updates.len() + 1);
        for v in updates.values() {
            bind_value(&mut params, v);
        }
        params.push(Box::new(id));
        let tx = rusqlite::Transaction::new_unchecked(
            &*conn,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(|e| ApiError::internal(format!("BEGIN update {table}: {e}")))?;
        tx.execute(&q, rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("Update {table}: {e}")))?;
        // Relit la ligne complète DANS la transaction : le payload journalisé est
        // l'état final exact, pas seulement les colonnes modifiées.
        let mut stmt = tx
            .prepare(&format!("SELECT * FROM \"{table}\" WHERE id=?"))
            .map_err(|e| ApiError::internal(format!("Get {table}: {e}")))?;
        let mut rows = stmt
            .query_map([id], |r| row_to_value(r))
            .map_err(|e| ApiError::internal(format!("Get {table}: {e}")))?;
        let row = match rows.next() {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(ApiError::internal(format!("Get {table}: {e}"))),
            None => Err(ApiError::not_found(format!("Ligne {table} introuvable"))),
        }?;
        self.enqueue_outbox(&*tx, "UPDATE", table, id, &row)?;
        drop(rows);
        drop(stmt);
        tx.commit()
            .map_err(|e| ApiError::internal(format!("Commit update {table}: {e}")))?;
        self.mark_dirty(table);
        self.notify_change(table, "UPDATE", row.clone());
        Ok(row)
    }

    /// SOFT-DELETE : rien n'est supprimé physiquement, la ligne passe deleted=1.
    /// C'est ce qui permet à la sync de propager la suppression vers Supabase (qui ne
    /// supprime jamais non plus) sans que la donnée ne réapparaisse au prochain pull.
    pub fn restore_with_row(&self, table: &str, id: i64, cloud_row: Option<&Value>) -> ApiResult<Value> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let tx = rusqlite::Transaction::new_unchecked(
            &*conn,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(|e| ApiError::internal(format!("BEGIN restore {table}: {e}")))?;

        let exists = {
            let mut stmt = tx
                .prepare(&format!("SELECT id FROM \"{table}\" WHERE id=?"))
                .map_err(|e| ApiError::internal(format!("Check {table}: {e}")))?;
            stmt.exists([id]).unwrap_or(false)
        };

        if exists {
            tx.execute(&format!("UPDATE \"{table}\" SET deleted=0 WHERE id=?"), [id])
                .map_err(|e| ApiError::internal(format!("Restore {table}: {e}")))?;
        } else if let Some(row_val) = cloud_row {
            if let Some(obj) = row_val.as_object() {
                let local_cols = self.columns_conn(&*tx, table).unwrap_or_default();
                let mut map = Map::new();
                for (k, v) in obj {
                    if !local_cols.contains(k) {
                        continue;
                    }
                    if k == "deleted" {
                        map.insert(k.clone(), json!(0));
                    } else {
                        map.insert(k.clone(), v.clone());
                    }
                }
                map.insert("id".into(), json!(id));
                let keys: Vec<&String> = map.keys().collect();
                let cols: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
                let placeholders: Vec<&str> = vec!["?"; cols.len()];
                let q = format!(
                    "INSERT OR REPLACE INTO \"{table}\" ({}) VALUES ({})",
                    cols.join(","),
                    placeholders.join(",")
                );
                let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::with_capacity(cols.len());
                for k in keys {
                    bind_value(&mut params, &map[k]);
                }
                tx.execute(&q, rusqlite::params_from_iter(params))
                    .map_err(|e| ApiError::internal(format!("Insert restored {table}: {e}")))?;
            }
        }

        let mut stmt = tx
            .prepare(&format!("SELECT * FROM \"{table}\" WHERE id=?"))
            .map_err(|e| ApiError::internal(format!("Get {table}: {e}")))?;
        let mut rows = stmt
            .query_map([id], |r| row_to_value(r))
            .map_err(|e| ApiError::internal(format!("Get {table}: {e}")))?;
        let row = match rows.next() {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(ApiError::internal(format!("Get {table}: {e}"))),
            None => Err(ApiError::not_found(format!("Ligne {table} introuvable"))),
        }?;
        self.enqueue_outbox(&*tx, "UPDATE", table, id, &row)?;
        drop(rows);
        drop(stmt);
        tx.commit()
            .map_err(|e| ApiError::internal(format!("Commit restore {table}: {e}")))?;
        self.mark_dirty(table);
        self.notify_change(table, "UPDATE", row.clone());
        Ok(row)
    }

    pub fn restore(&self, table: &str, id: i64) -> ApiResult<Value> {
        self.restore_with_row(table, id, None)
    }

    pub fn permanent_delete(&self, table: &str, id: i64) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let tx = rusqlite::Transaction::new_unchecked(
            &*conn,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(|e| ApiError::internal(format!("BEGIN permanent_delete {table}: {e}")))?;
        tx.execute(&format!("DELETE FROM \"{table}\" WHERE id=?"), [id])
            .map_err(|e| ApiError::internal(format!("Permanent delete {table}: {e}")))?;
        let mut payload = Map::new();
        payload.insert("id".into(), json!(id));
        payload.insert("permanent_delete".into(), json!(true));
        payload.insert("deleted".into(), json!(1));
        self.enqueue_outbox(&*tx, "DELETE", table, id, &Value::Object(payload))?;
        tx.commit()
            .map_err(|e| ApiError::internal(format!("Commit permanent_delete {table}: {e}")))?;
        self.mark_dirty(table);
        self.notify_change(table, "DELETE", json!({ "id": id, "permanent": true }));
        Ok(())
    }

    pub fn remove(&self, table: &str, id: i64) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let tx = rusqlite::Transaction::new_unchecked(
            &*conn,
            rusqlite::TransactionBehavior::Deferred,
        )
        .map_err(|e| ApiError::internal(format!("BEGIN delete {table}: {e}")))?;
        tx.execute(&format!("UPDATE \"{table}\" SET deleted=1 WHERE id=?"), [id])
            .map_err(|e| ApiError::internal(format!("Delete {table}: {e}")))?;
        // Tombstone : la suppression est journalisée avec le payload minimal.
        // C'est ce qui permettra au serveur de propager la suppression sans
        // jamais réapparaître au pull (spec ~/mod.md).
        let mut payload = Map::new();
        payload.insert("id".into(), json!(id));
        payload.insert("deleted".into(), json!(1));
        self.enqueue_outbox(&*tx, "DELETE", table, id, &Value::Object(payload))?;
        tx.commit()
            .map_err(|e| ApiError::internal(format!("Commit delete {table}: {e}")))?;
        self.mark_dirty(table);
        self.notify_change(table, "DELETE", json!({ "id": id }));
        Ok(())
    }

    pub fn get(&self, table: &str, id: i64) -> ApiResult<Value> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM \"{table}\" WHERE id=?"))
            .map_err(|e| ApiError::internal(format!("Get {table}: {e}")))?;
        let mut rows = stmt
            .query_map([id], |r| row_to_value(r))
            .map_err(|e| ApiError::internal(format!("Get {table}: {e}")))?;
        match rows.next() {
            Some(Ok(row)) => Ok(row),
            Some(Err(e)) => Err(ApiError::internal(format!("Get {table}: {e}"))),
            None => Err(ApiError::not_found(format!("Ligne {table} introuvable"))),
        }
    }

    pub fn get_opt(&self, table: &str, id: i64) -> ApiResult<Option<Value>> {
        match self.get(table, id) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.status == 404 => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crud() {
        std::fs::remove_file("/tmp/gl_test.db").ok();
        let db = Db::open(Path::new("/tmp/gl_test.db")).unwrap();
        let mut row = Map::new();
        row.insert("nom".into(), json!("PS5 - Poste 1"));
        row.insert("type".into(), json!("PS5"));
        row.insert("etat".into(), json!("disponible"));
        row.insert("poste_numero".into(), json!(1));
        let created = db.insert("consoles", &row).unwrap();
        assert_eq!(created["nom"], json!("PS5 - Poste 1"));
        let id = created["id"].as_i64().unwrap();
        let mut upd = Map::new();
        upd.insert("etat".into(), json!("occupee"));
        db.update("consoles", id, &upd).unwrap();
        let got = db.get("consoles", id).unwrap();
        assert_eq!(got["etat"], json!("occupee"));
        db.remove("consoles", id).unwrap();
        // Soft-delete : la ligne existe toujours mais est cachée (deleted=1)
        let soft = db.get_opt("consoles", id).unwrap().unwrap();
        assert_eq!(soft["deleted"], json!(1));
        assert!(db.query_all("consoles").unwrap().is_empty());
    }

    #[test]
    fn outbox() {
        std::fs::remove_file("/tmp/gl_test_outbox.db").ok();
        let db = Db::open(Path::new("/tmp/gl_test_outbox.db")).unwrap();
        assert_eq!(db.outbox_pending_count().unwrap(), 0);

        // INSERT -> 1 événement journalisé
        let mut row = Map::new();
        row.insert("nom".into(), json!("PS5 - Poste 1"));
        row.insert("type".into(), json!("PS5"));
        let created = db.insert("consoles", &row).unwrap();
        assert_eq!(db.outbox_pending_count().unwrap(), 1);
        let id = created["id"].as_i64().unwrap();

        // UPDATE -> 2
        let mut upd = Map::new();
        upd.insert("etat".into(), json!("occupee"));
        db.update("consoles", id, &upd).unwrap();
        assert_eq!(db.outbox_pending_count().unwrap(), 2);

        // DELETE -> 3 (tombstone)
        db.remove("consoles", id).unwrap();
        assert_eq!(db.outbox_pending_count().unwrap(), 3);

        let pending = db.outbox_pending(10).unwrap();
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0]["operation"], json!("INSERT"));
        assert_eq!(pending[1]["operation"], json!("UPDATE"));
        assert_eq!(pending[2]["operation"], json!("DELETE"));
        assert_eq!(pending[0]["table_name"], json!("consoles"));
        assert_eq!(pending[0]["record_id"], json!(id));
        assert_eq!(pending[0]["status"], json!("PENDING"));

        // device_id : ULID de 26 chars, stable, séquence strictement croissante
        let did0 = pending[0]["device_id"].as_str().unwrap();
        let did2 = pending[2]["device_id"].as_str().unwrap();
        assert_eq!(did0.len(), 26);
        assert_eq!(did0, did2);
        let seq0 = pending[0]["device_sequence"].as_i64().unwrap();
        let seq2 = pending[2]["device_sequence"].as_i64().unwrap();
        assert!(seq2 > seq0);
        // change_id = device:sequence, unique
        assert_eq!(pending[0]["change_id"].as_str().unwrap(), format!("{did0}:1"));
    }

    #[test]
    fn delta_apply() {
        std::fs::remove_file("/tmp/gl_test_delta.db").ok();
        let db = Db::open(Path::new("/tmp/gl_test_delta.db")).unwrap();

        // Insertion distante (sessions_jeu = protégée) : appliquée SANS journal outbox
        let mut row = Map::new();
        row.insert("id".into(), json!(42));
        row.insert("statut".into(), json!("en_cours"));
        row.insert("debut".into(), json!("2026-09-12T10:00:00"));
        let r = db
            .apply_remote_change("sessions_jeu", 42, &row, "remote:1", "2026-09-12T10:00:00")
            .unwrap();
        assert_eq!(r, "applied");
        assert_eq!(db.outbox_pending_count().unwrap(), 0); // pas de re-push

        // Conflit : modification locale PENDING puis changement distant -> local gagne
        let mut upd = Map::new();
        upd.insert("statut".into(), json!("terminee"));
        db.update("sessions_jeu", 42, &upd).unwrap();
        assert_eq!(db.outbox_pending_count().unwrap(), 1);
        let mut row2 = Map::new();
        row2.insert("id".into(), json!(42));
        row2.insert("statut".into(), json!("annulee"));
        row2.insert("debut".into(), json!("2026-09-12T10:00:00"));
        let r2 = db
            .apply_remote_change("sessions_jeu", 42, &row2, "remote:2", "2026-09-12T11:00:00")
            .unwrap();
        assert_eq!(r2, "conflict"); // protégée -> local gagne
        let got = db.get("sessions_jeu", 42).unwrap();
        assert_eq!(got["statut"], json!("terminee"));

        // Table non protégée + distant plus récent -> distant gagne (last-write-wins horodaté)
        let mut c = Map::new();
        c.insert("id".into(), json!(7));
        c.insert("nom".into(), json!("PS5"));
        db.insert("consoles", &c).unwrap();
        let mut c2 = Map::new();
        c2.insert("id".into(), json!(7));
        c2.insert("nom".into(), json!("PS5 Pro"));
        let r3 = db
            .apply_remote_change("consoles", 7, &c2, "remote:3", "2026-09-12T12:00:00")
            .unwrap();
        assert_eq!(r3, "applied");
        let got2 = db.get("consoles", 7).unwrap();
        assert_eq!(got2["nom"], json!("PS5 Pro"));

        // Tombstone distant : la ligne passe deleted=1
        let mut del = Map::new();
        del.insert("id".into(), json!(7));
        del.insert("deleted".into(), json!(1));
        let r4 = db.apply_remote_change("consoles", 7, &del, "remote:4", "").unwrap();
        assert_eq!(r4, "tombstone");
        assert_eq!(db.get_opt("consoles", 7).unwrap().unwrap()["deleted"], json!(1));
    }
}
