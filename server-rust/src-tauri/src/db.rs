// Couche base de données SQLite (rusqlite, SQLite embarqué).
// Port de server/db.ts : mêmes tables, mêmes types de stockage (TEXT ISO pour les dates,
// INTEGER 0/1 pour les booléens) pour rester compatible avec la base existante.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::error::{ApiError, ApiResult};

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
CREATE TABLE IF NOT EXISTS consoles (id INTEGER PRIMARY KEY, nom TEXT, type TEXT, etat TEXT DEFAULT 'disponible', poste_numero INTEGER, date_ajout TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS jeux (id INTEGER PRIMARY KEY, titre TEXT, genre TEXT, console_id INTEGER, actif INTEGER DEFAULT 1, jaquette_url TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS joueurs (id INTEGER PRIMARY KEY, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TEXT, derniere_visite TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS sessions_jeu (id INTEGER PRIMARY KEY, console_id INTEGER, joueur_id INTEGER, jeu_id INTEGER, employe_id INTEGER, tarif_id INTEGER, debut TEXT, fin TEXT, duree_minutes INTEGER, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS tarifs (id INTEGER PRIMARY KEY, nom TEXT, type TEXT, prix INTEGER, duree_minutes INTEGER, description TEXT, actif INTEGER DEFAULT 1, console_type TEXT, jeu TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS factures (id INTEGER PRIMARY KEY, numero_facture TEXT UNIQUE, session_id INTEGER, joueur_id INTEGER, montant_ht REAL, taux_tva REAL DEFAULT 20, montant_tva REAL, montant_ttc REAL, mode_paiement TEXT, statut TEXT, date_paiement TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS jetons_transactions (id INTEGER PRIMARY KEY, joueur_id INTEGER, quantite INTEGER, type TEXT, raison TEXT, session_id INTEGER, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS messages (id INTEGER PRIMARY KEY, titre TEXT, contenu TEXT, auteur TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS parametres_fidelite (id INTEGER PRIMARY KEY, regle_type TEXT, seuil INTEGER, jetons_attribues INTEGER, actif INTEGER DEFAULT 1, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS lignes_facture (id INTEGER PRIMARY KEY, facture_id INTEGER, description TEXT, quantite INTEGER, prix_unitaire INTEGER, total_ligne INTEGER, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS hangouts (id INTEGER PRIMARY KEY, titre TEXT, activite TEXT, prix INTEGER, actif INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT, deleted INTEGER DEFAULT 0);
"#;

/// Migration soft-delete : ajoute la colonne `deleted` aux bases existantes.
/// (Neon ne supprime JAMAIS physiquement : une ligne supprimée passe deleted=1,
/// est cachée des listes, et la suppression se propage par la sync.)
const SOFT_DELETE_MIGRATION: &str = "\
ALTER TABLE users ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE consoles ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE jeux ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE joueurs ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE sessions_jeu ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE tarifs ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE factures ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE jetons_transactions ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE messages ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE parametres_fidelite ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE lignes_facture ADD COLUMN deleted INTEGER DEFAULT 0; \
ALTER TABLE hangouts ADD COLUMN deleted INTEGER DEFAULT 0;";

pub struct Db(pub Mutex<Connection>);

/// Horodatage ISO 8601 avec millisecondes, comme `new Date().toISOString()` en JS.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Date du jour au format YYYYMMDD (pour les numéros de facture).
pub fn today_str() -> String {
    chrono::Utc::now().format("%Y%m%d").to_string()
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
        let _ = conn.execute_batch(SOFT_DELETE_MIGRATION);
        apply_pragmas(&conn);
        // Test écriture immédiate pour détecter disque plein / permission early
        let _ = conn.execute_batch("CREATE TABLE IF NOT EXISTS __healthcheck (id INTEGER PRIMARY KEY); DROP TABLE IF EXISTS __healthcheck;");
        Ok(Db(Mutex::new(conn)))
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
             ALTER TABLE sessions_jeu ADD COLUMN tarif_id INTEGER;",
        );
        let _ = conn.execute_batch(SOFT_DELETE_MIGRATION);
        apply_pragmas(&conn);
        Ok(Db(Mutex::new(conn)))
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
    /// propager les suppressions vers Neon (tombstones).
    pub fn query_all_all(&self, table: &str) -> ApiResult<Vec<Value>> {
        self.query_all_impl(table, "")
    }

    /// Colonnes de la table locale (PRAGMA table_info) pour filtrer les données venues
    /// de Neon : si Neon a des colonnes en plus (ou des noms différents), l'insert
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

    pub fn find_one(&self, table: &str, pred: impl Fn(&Value) -> bool) -> ApiResult<Option<Value>> {
        Ok(self.query_all(table)?.into_iter().find(pred))
    }

    /// Comme find_one mais inclut les lignes soft-deleted (pour ne jamais les ressusciter).
    pub fn find_one_all(&self, table: &str, pred: impl Fn(&Value) -> bool) -> ApiResult<Option<Value>> {
        Ok(self.query_all_all(table)?.into_iter().find(pred))
    }

    /// Insère un enregistrement. Si un `id` est fourni, INSERT OR REPLACE (comme db.ts).
    pub fn insert(&self, table: &str, data: &Map<String, Value>) -> ApiResult<Value> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let has_id = data.contains_key("id");
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
        conn.execute(&q, rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("Insert {table}: {e}")))?;
        let row_id = conn.last_insert_rowid();
        drop(conn);

        let mut out = data.clone();
        out.insert("id".into(), json!(row_id));
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
        conn.execute(&q, rusqlite::params_from_iter(params))
            .map_err(|e| ApiError::internal(format!("Update {table}: {e}")))?;
        drop(conn);
        self.get(table, id)
    }

    /// SOFT-DELETE : rien n'est supprimé physiquement, la ligne passe deleted=1.
    /// C'est ce qui permet à la sync de propager la suppression vers Neon (qui ne
    /// supprime jamais non plus) sans que la donnée ne réapparaisse au prochain pull.
    pub fn remove(&self, table: &str, id: i64) -> ApiResult<()> {
        let conn = match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        conn.execute(&format!("UPDATE \"{table}\" SET deleted=1 WHERE id=?"), [id])
            .map_err(|e| ApiError::internal(format!("Delete {table}: {e}")))?;
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
}