// Supabase sync - offline-first via tokio-postgres + rustls 0.19 (pure Rust)
use serde_json::{Value, json};
use crate::error::{ApiError, ApiResult};

use tauri::{AppHandle, Emitter};

/// Payload standard de progression de sync (envoyé au WebView via emit).
/// Tous les chiffres viennent du travail RÉELLEMENT effectué (spec mission.md §7) :
/// jamais de compteur simulé.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncProgress {
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub percent: u32,
    pub processed: u64,
    pub message: String,
    pub status: String,
}

impl SyncProgress {
    pub fn new(phase: &str, message: &str, status: &str) -> Self {
        SyncProgress { phase: phase.into(), current: 0, total: 0, percent: 0, processed: 0, message: message.into(), status: status.into() }
    }
    pub fn with_counts(mut self, current: u64, total: u64, processed: u64) -> Self {
        self.current = current;
        self.total = total;
        self.processed = processed;
        self.percent = if total == 0 { if self.status == "completed" { 100 } else { 0 } } else { ((current * 100) / total).min(100) as u32 };
        self
    }
}

/// Émet un événement de progression au WebView (fire-and-forget : jamais
/// d'erreur si aucun listener). Message = "sync-progress" (phase intermédiaire)
/// ou "sync-completed" / "sync-error" (fin).
pub fn emit_progress(app: &AppHandle, p: &SyncProgress, final_event: Option<&str>) {
    let payload = json!({
        "phase": p.phase,
        "current": p.current,
        "total": p.total,
        "percent": p.percent,
        "processed": p.processed,
        "message": p.message,
        "status": p.status,
    });
    let ev = final_event.unwrap_or("sync-progress");
    if let Err(e) = app.emit(ev, payload.clone()) {
        eprintln!("[sync] emit {} failed: {}", ev, e);
    }
    if final_event.is_some() {
        // Les vues qui n'écoutent que sync-progress reçoivent quand même l'état final.
        let _ = app.emit("sync-progress", payload);
    }
}

#[cfg(feature = "supabase-sync")]
use tokio_postgres::{Client, NoTls};

#[cfg(feature = "supabase-sync")]
#[derive(Clone)]
pub struct SupabasePool {
    pub client: std::sync::Arc<Client>,
}

// Timeouts courts OBLIGATOIRES : une connexion Supabase morte (fermée par le serveur après idle,
// ou changement réseau mobile) fait HANGUER une query pendant des minutes (retransmission TCP).
// C'était la cause du "Timeout IPC (Android WebView)" : auth_login/test_supabase_connection ne répondaient jamais.
#[cfg(feature = "supabase-sync")]
const SUPABASE_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);
#[cfg(feature = "supabase-sync")]
const SUPABASE_QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);

/// Message d'erreur Supabase LISIBLE : le Display générique des erreurs serveur est
/// "db error" (Kind::Db) — le VRAI message SQL (ex: "column deleted does not exist",
/// "COALESCE types integer and boolean cannot be matched") est dans la cause (DbError).
#[cfg(feature = "supabase-sync")]
fn supabase_error_str(e: tokio_postgres::Error) -> String {
    match e.into_source() {
        Some(src) => format!("{}", src),
        None => "erreur serveur (cause inconnue)".to_string(),
    }
}

#[cfg(feature = "supabase-sync")]
pub async fn supabase_query(pool: &SupabasePool, sql: &str, params: &[&(dyn tokio_postgres::types::ToSql + Sync)]) -> Result<Vec<tokio_postgres::Row>, String> {
    match tokio::time::timeout(SUPABASE_QUERY_TIMEOUT, pool.client.query(sql, params)).await {
        Ok(Ok(rows)) => Ok(rows),
        Ok(Err(e)) => Err(supabase_error_str(e)),
        Err(_) => Err(format!("timeout après {}s (connexion morte ?)", SUPABASE_QUERY_TIMEOUT.as_secs())),
    }
}

/// Ping rapide de la connexion Supabase (SELECT 1 avec timeout court)
#[cfg(feature = "supabase-sync")]
pub async fn ping(pool: &SupabasePool) -> Result<(), String> {
    match tokio::time::timeout(SUPABASE_QUERY_TIMEOUT, pool.client.query_one("SELECT 1", &[])).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("timeout après {}s (connexion morte)", SUPABASE_QUERY_TIMEOUT.as_secs())),
    }
}

/// Reconnexion Supabase en arrière-plan après connexion morte.
/// Supabase ferme les connexions inactives ; sans ça, le pool reste mort jusqu'au redémarrage de l'app.
#[cfg(feature = "supabase-sync")]
pub fn schedule_reconnect(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    let Some(state) = app.try_state::<crate::AppState>() else { return };
    // Un seul reconnect à la fois (les commandes en erreur spament toutes reconnect sinon)
    if state.supabase_reconnecting.swap(true, Ordering::SeqCst) { return; }
    // Pool indisponible pendant la reconnexion (les appels passeront en offline au lieu de hanguer)
    if let Ok(mut guard) = state.supabase_pool.lock() {
        *guard = None;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::logger::log_cloud("reconnexion Supabase en arrière-plan...");
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match init_supabase_pool().await {
            Some(pool) => {
                crate::logger::log_cloud("reconnexion Supabase OK, pool restauré");
                if let Some(s) = handle.try_state::<crate::AppState>() {
                    if let Ok(mut guard) = s.supabase_pool.lock() {
                        *guard = Some(pool);
                    }
                }
            }
            None => crate::logger::log_cloud("reconnexion Supabase ÉCHOUÉE (offline) — vérifier internet / DNS / port 5432, réessai au prochain usage"),
        }
        if let Some(s) = handle.try_state::<crate::AppState>() {
            s.supabase_reconnecting.store(false, Ordering::SeqCst);
        }
    });
}

/// Reconnexion BLOQUANTE (await) pour le retry du login : contrairement à
/// schedule_reconnect (fire-and-forget), cette version attend le résultat et
/// renvoie le nouveau pool. Bornée par les timeouts de init_supabase_pool
/// (8s connect + 6s test query). Le pool est nettoyé AVANT la tentative pour
/// qu'un échec laisse l'app en état "offline" cohérent (pas de pool mort).
#[cfg(feature = "supabase-sync")]
pub async fn reconnect_now(app: &tauri::AppHandle) -> Option<SupabasePool> {
    use tauri::Manager;
    if let Some(state) = app.try_state::<crate::AppState>() {
        if let Ok(mut guard) = state.supabase_pool.lock() {
            *guard = None;
        }
    }
    crate::logger::log_cloud("reconnexion Supabase (retry login)...");
    match init_supabase_pool().await {
        Some(pool) => {
            crate::logger::log_cloud("reconnexion Supabase OK (retry login), pool restauré");
            if let Some(state) = app.try_state::<crate::AppState>() {
                if let Ok(mut guard) = state.supabase_pool.lock() {
                    *guard = Some(pool.clone());
                }
            }
            Some(pool)
        }
        None => {
            crate::logger::log_cloud("reconnexion Supabase ÉCHOUÉE (retry login) — réseau indisponible ou serveur injoignable");
            None
        }
    }
}

/// URL de connexion cloud (Postgres).
/// SUPABASE (remplace Supabase) : on utilise le SESSION POOLER (port 5432) —
/// - certificat Let's Encrypt public (vérifié par webpki-roots, pas de CA custom),
/// - IPv4 (le direct db.xxx.supabase.co est IPv6-only sur les nouveaux projets ->
///   ne se connecte jamais depuis Android),
/// - compatible protocole simple (pas de prepared statements persistants).
/// L'URL est aussi injectée par le workflow GitHub via la variable DATABASE_URL
/// (option_env! au build) ; ce fallback garantit que l'APK marche même sans secret.
#[cfg(feature = "supabase-sync")]
pub const FALLBACK_URL: &str = "postgresql://postgres.tyvqidhbgqveaftlvjrn:TkL.w78%265%26-Lr%40_@aws-1-eu-west-1.pooler.supabase.com:5432/postgres?sslmode=require";

#[cfg(feature = "supabase-sync")]
pub async fn get_supabase_pool(state: &crate::AppState) -> crate::error::ApiResult<SupabasePool> {
    if let Ok(guard) = state.supabase_pool.lock() {
        if let Some(ref pool) = *guard {
            return Ok(pool.clone());
        }
    }
    if let Some(pool) = init_supabase_pool().await {
        if let Ok(mut guard) = state.supabase_pool.lock() {
            *guard = Some(pool.clone());
        }
        return Ok(pool);
    }
    Err(crate::error::ApiError::new(503, "Connexion à Supabase impossible. Vérifiez votre connexion Internet."))
}

pub async fn init_supabase_pool() -> Option<SupabasePool> {
    // NOTE : option_env! retiré — le secret GitHub DATABASE_URL contenait encore
    // l'ancienne URL Supabase et aurait pris le dessus sur le fallback Supabase à chaque
    // build Android. Sur Android il n'y a pas d'env runtime : FALLBACK_URL est
    // autoritaire. Sur desktop, dotenvy charge .env -> std::env::var fonctionne.
    let url = std::env::var("DATABASE_URL").ok().filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("SUPABASE_DATABASE_URL").ok().filter(|s| !s.trim().is_empty()))
        .or_else(|| Some(FALLBACK_URL.to_string()));
    let url = match url {
        Some(u) if !u.trim().is_empty() => u,
        _ => {
            crate::logger::log_cloud("DATABASE_URL absent et fallback vide, mode offline");
            return None;
        }
    };
    crate::logger::log_cloud(&format!("DATABASE_URL présent ({} chars), tentative rustls", url.len()));
    // Tente rustls 0.19 - webpki-roots 0.21 fournit TLS_SERVER_ROOTS directement compatible
    // SUPABASE : la chaîne du pooler (*.pooler.supabase.com) remonte à "Supabase Root
    // 2021 CA", une racine PRIVÉE absente des racines publiques Mozilla -> sans l'ajouter
    // le handshake TLS échoue, le fallback NoTls est refusé par le pooler (TLS obligatoire)
    // et l'app restait OFFLINE. On embarque la racine officielle Supabase (certs/).
    let rustls_result = tokio::time::timeout(SUPABASE_CONNECT_TIMEOUT, async {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
        let supa_pem: &[u8] = include_bytes!("../certs/supabase-root-ca.pem");
        let mut pem_rd = std::io::BufReader::new(supa_pem);
        match rustls::internal::pemfile::certs(&mut pem_rd) {
            Ok(certs) => {
                let mut added = 0;
                for c in certs.iter() {
                    if root_store.add(c).is_ok() { added += 1; }
                }
                crate::logger::log_cloud(&format!("CA Supabase chargée dans le store TLS ({} certificat(s))", added));
            }
            Err(_) => crate::logger::log_cloud("WARNING: échec parsing CA Supabase (TLS Supabase va échouer)"),
        }
        let mut config = rustls::ClientConfig::new();
        config.root_store = root_store;
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(config);
        tokio_postgres::connect(&url, tls).await
    }).await;

    match rustls_result {
        Ok(Ok((client, connection))) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[supabase] connection error (rustls): {}", e);
                }
            });
            match tokio::time::timeout(SUPABASE_QUERY_TIMEOUT, client.query("SELECT 1", &[])).await {
                Ok(Ok(_)) => {
                    eprintln!("[supabase] pool connecté (rustls)");
                    return Some(SupabasePool { client: std::sync::Arc::new(client) });
                }
                Ok(Err(e)) => {
                    eprintln!("[supabase] rustls test query failed: {}, tente NoTls", e);
                }
                Err(_) => {
                    eprintln!("[supabase] rustls test query timeout, tente NoTls");
                }
            }
        }
        Ok(Err(e)) => {
            crate::logger::log_cloud(&format!("rustls connect failed: {} (tente NoTls)", e));
            eprintln!("[supabase] rustls connect failed: {}, tente NoTls", e);
        }
        Err(_) => {
            eprintln!("[supabase] rustls connect timeout ({}s), tente NoTls", SUPABASE_CONNECT_TIMEOUT.as_secs());
        }
    }

    // Fallback NoTls
    let no_tls_result = tokio::time::timeout(SUPABASE_CONNECT_TIMEOUT, tokio_postgres::connect(&url, NoTls)).await;
    match no_tls_result {
        Ok(Ok((client, connection))) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[supabase] connection error (NoTls): {}", e);
                }
            });
            match tokio::time::timeout(SUPABASE_QUERY_TIMEOUT, client.query("SELECT 1", &[])).await {
                Ok(Ok(_)) => {
                    eprintln!("[supabase] pool connecté (NoTls)");
                    Some(SupabasePool { client: std::sync::Arc::new(client) })
                }
                Ok(Err(e)) => {
                    eprintln!("[supabase] NoTls test query failed: {}, offline", e);
                    None
                }
                Err(_) => {
                    eprintln!("[supabase] NoTls test query timeout, offline");
                    None
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("[supabase] pool connect failed (offline): {}", e);
            None
        }
        Err(_) => {
            eprintln!("[supabase] pool connect timeout ({}s), offline", SUPABASE_CONNECT_TIMEOUT.as_secs());
            None
        }
    }
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn init_supabase_pool() -> Option<()> {
    eprintln!("[supabase] feature supabase-sync désactivée");
    None
}

#[derive(Debug, Clone)]
pub struct SupabaseUser {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub nom: String,
    pub created_at: Option<String>,
    /// Compte archivé côté cloud : le login doit être refusé, sinon un compte
    /// supprimé sur un autre appareil ressuscitait ici au premier login.
    pub deleted: bool,
}

/// Compte utilisateur Supabase pour le LOGIN (email + mot de passe).
/// Utilisé quand le user n'existe pas (ou n'a pas de hash) localement : on
/// valide le mot de passe contre le hash cloud, puis on réplique le user en
/// SQLite pour permettre les connexions hors ligne suivantes.
#[cfg(feature = "supabase-sync")]
pub async fn fetch_supabase_user(pool: &SupabasePool, email: &str) -> ApiResult<Option<SupabaseUser>> {
    // Email en dur (échappé) : les requêtes avec paramètres ($1) passent par le protocole
    // étendu (prepared statements) que le pooler Supabase ne supporte pas -> login cassé
    let sql = format!("SELECT id, email, password_hash, role, nom, created_at, COALESCE(deleted::int, 0) AS deleted FROM users WHERE email = '{}' LIMIT 1", escape_sql(email));
    let rows = supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Supabase query user: {}", e)))?;
    if rows.is_empty() {
        return Ok(None);
    }
    let row = &rows[0];
    // Décodage défensif : tout panic potentiel est contenu (catch_unwind), jamais propagé
    let decode = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        SupabaseUser {
            id: row.try_get::<_, i64>(0).unwrap_or(0),
            email: pg_col_to_string_pub(row, 1).unwrap_or_default(),
            password_hash: pg_col_to_string_pub(row, 2).unwrap_or_default(),
            role: pg_col_to_string_pub(row, 3).unwrap_or_else(|| "employe".to_string()),
            nom: pg_col_to_string_pub(row, 4).unwrap_or_default(),
            created_at: pg_col_to_string_pub(row, 5),
            deleted: row.try_get::<_, i32>(6).unwrap_or(0) == 1,
        }
    }));
    match decode {
        Ok(u) => Ok(Some(u)),
        Err(_) => {
            crate::logger::log_cloud("fetch_supabase_user: décodage row paniqué, user ignoré");
            Ok(None)
        }
    }
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn fetch_supabase_user(_pool: &(), _email: &str) -> ApiResult<Option<SupabaseUser>> {
    Ok(None)
}

// Lecture d'une colonne texte tolérante aux pannes : N'UTILISE PAS row.get (panique si type
// Postgres inattendu, ex: TIMESTAMPTZ décodé en String) — un panic ici tue l'app Android.
#[cfg(feature = "supabase-sync")]
pub fn pg_col_to_string_pub(row: &tokio_postgres::Row, idx: usize) -> Option<String> {
    match row.try_get::<_, Option<String>>(idx) {
        Ok(Some(s)) => Some(s),
        Ok(None) => None,
        Err(_) => None, // type inattendu (timestamp, etc.) -> on ignore la colonne, pas de panic
    }
}

#[cfg(feature = "supabase-sync")]
fn pg_col_to_string(row: &tokio_postgres::Row, idx: usize) -> Option<String> {
    pg_col_to_string_pub(row, idx)
}

/// Lecture d'une colonne ENTIÈRE tolérante aux types Postgres rencontrés.
///
/// INDISPENSABLE : `pg_col_to_string_pub` renvoie None pour tout ce qui n'est pas
/// du texte, et donc pour un BIGINT/BIGSERIAL. `sequence` et `record_id` de
/// `sync_changes` étant des BIGINT, ils étaient décodés à 0 :
/// - `pull_delta` voyait « séquence 0 » -> « Changement cloud invalide à la
///   séquence 0 (curseur conservé à 0) » à CHAQUE cycle,
/// - `sync_max_sequence` renvoyait 0 -> le curseur ne dépassait jamais 0,
/// - `sync_changes_count_after` renvoyait 0 -> aucune progression delta.
/// Résultat : les modifications envoyées par un appareil n'arrivaient JAMAIS sur
/// les autres. On tente i64, puis i32 (int4), puis bool (0/1), puis texte.
#[cfg(feature = "supabase-sync")]
pub fn pg_col_to_i64(row: &tokio_postgres::Row, idx: usize) -> Option<i64> {
    if let Ok(v) = row.try_get::<_, i64>(idx) {
        return Some(v);
    }
    if let Ok(v) = row.try_get::<_, Option<i64>>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<_, i32>(idx) {
        return Some(v as i64);
    }
    if let Ok(v) = row.try_get::<_, bool>(idx) {
        return Some(if v { 1 } else { 0 });
    }
    pg_col_to_string_pub(row, idx).and_then(|s| s.trim().parse::<i64>().ok())
}

#[cfg(feature = "supabase-sync")]
#[cfg(feature = "supabase-sync")]
pub async fn pull_app_settings(pool: &SupabasePool) -> Result<Vec<(String, String)>, String> {
    let rows = supabase_query(pool, "SELECT key, value FROM app_settings", &[]).await?;
    let mut res = Vec::new();
    for r in rows {
        let k: String = r.get(0);
        let v: String = r.get(1);
        res.push((k, v));
    }
    Ok(res)
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn pull_app_settings(_pool: &()) -> Result<Vec<(String, String)>, String> {
    Ok(Vec::new())
}

#[cfg(feature = "supabase-sync")]
pub async fn set_app_setting(pool: &SupabasePool, key: &str, value: &str) -> Result<(), String> {
    let sql = "INSERT INTO app_settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value";
    let params: [&(dyn tokio_postgres::types::ToSql + Sync); 2] = [&key, &value];
    supabase_query(pool, sql, params.as_slice()).await?;
    Ok(())
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn set_app_setting(_pool: &(), _key: &str, _value: &str) -> Result<(), String> {
    Ok(())
}

pub async fn pull_users(pool: &SupabasePool) -> ApiResult<Vec<Value>> {
    // `deleted` fait partie du SELECT : sans elle, toute ligne pullée paraît
    // active et un compte archivé ailleurs était ressuscité sur cet appareil.
    let rows = supabase_query(pool, "SELECT id, email, password_hash, role, nom, created_at, COALESCE(deleted::int, 0) AS deleted FROM users ORDER BY id", &[])
        .await
        .map_err(|e| ApiError::internal(format!("Supabase pull users: {}", e)))?;
    // Décodage défensif : try_get + String partout (pas de chrono), catch_unwind par ligne.
    // Avant : row.get::<_, Option<chrono::NaiveDateTime>>(5) PANIQUAIT si la colonne Postgres
    // est TIMESTAMPTZ (déjà mappé chrono) mais type inattendu -> app tuée en background.
    let mut out = Vec::new();
    for row in rows.iter() {
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            json!({
                "id": row.try_get::<_, i64>(0).unwrap_or(0),
                "email": pg_col_to_string(row, 1).unwrap_or_default(),
                "password_hash": pg_col_to_string(row, 2).unwrap_or_default(),
                "role": pg_col_to_string(row, 3).unwrap_or_else(|| "employe".to_string()),
                "nom": pg_col_to_string(row, 4).unwrap_or_default(),
                "created_at": pg_col_to_string(row, 5),
                "deleted": row.try_get::<_, i32>(6).unwrap_or(0),
            })
        }));
        match decoded {
            Ok(v) => out.push(v),
            Err(_) => {
                crate::logger::log_cloud("pull_users: ligne paniquée au décodage, ignorée");
            }
        }
    }
    Ok(out)
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn pull_users(_pool: &()) -> ApiResult<Vec<Value>> {
    Ok(vec![])
}

/// Échappe une valeur pour insertion SQL en dur (protocole simple) : ' -> ''
#[cfg(feature = "supabase-sync")]
pub fn escape_sql(s: &str) -> String {
    s.split("'").collect::<Vec<_>>().join("''")
}

/// Exécution en protocole SIMPLE (batch_execute) : PAS de prepared statement.
/// Le pooler Supabase (PgBouncer) ne supporte pas les prepared statements persistants
/// de tokio-postgres ("prepared statement PGBOUNCER_N does not exist") — le protocole
/// simple les évite totalement.
#[cfg(feature = "supabase-sync")]
pub async fn supabase_batch_execute(pool: &SupabasePool, sql: &str) -> Result<(), String> {
    match tokio::time::timeout(SUPABASE_QUERY_TIMEOUT, pool.client.batch_execute(sql)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(supabase_error_str(e)),
        Err(_) => Err(format!("timeout après {}s (connexion morte ?)", SUPABASE_QUERY_TIMEOUT.as_secs())),
    }
}

/// MIGRATION COMPLÈTE DU SCHÉMA CLOUD (idempotente) : crée toutes les tables si
/// elles n'existent pas, en miroir du schéma SQLite local (mêmes noms de colonnes
/// pour que le filtrage par colonnes fonctionne dans les deux sens).
///
/// SUPABASE : une base neuve n'a AUCUNE table -> sans ça, pull ET push échouent
/// ("relation does not exist") et la sync ne fait rien. Cette migration est
/// exécutée au boot (avant le premier login) et au début de chaque sync.
///
/// Types choisis pour un aller-retour parfait avec le local :
/// - id BIGINT IDENTITY : accepte les ids explicites du push (ON CONFLICT (id)) ET
///   l'auto-incrément pour les nouvelles lignes ; décodé en i64 sans perte.
/// - dates en TEXT : le local stocke des ISO 8601 et le décodage (pg_col_to_string)
///   attend du texte — pas de TIMESTAMPTZ qui casserait le décodage défensif.
/// - montants REAL en DOUBLE PRECISION : le push envoie des textes '12.5' qui ne
///   castent pas en INTEGER mais castent en DOUBLE PRECISION.
/// - deleted INTEGER 0/1 : identique au local (is_deleted_value gère aussi boolean).
#[cfg(feature = "supabase-sync")]
pub async fn ensure_cloud_schema(pool: &SupabasePool) -> Result<(), String> {
    let ddl = r#"
CREATE TABLE IF NOT EXISTS "users" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, email TEXT UNIQUE, password_hash TEXT, nom TEXT, role TEXT DEFAULT 'employe', created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "consoles" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, nom TEXT, type TEXT, etat TEXT DEFAULT 'disponible', poste_numero BIGINT, date_ajout TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "jeux" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, titre TEXT, genre TEXT, console_id BIGINT, actif INTEGER DEFAULT 1, jaquette_url TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "joueurs" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, nom TEXT, telephone TEXT, email TEXT, jetons_solde INTEGER DEFAULT 0, date_inscription TEXT, derniere_visite TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "sessions_jeu" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, console_id BIGINT, joueur_id BIGINT, jeu_id BIGINT, employe_id BIGINT, tarif_id BIGINT, debut TEXT, fin TEXT, duree_minutes INTEGER, duree_secondes INTEGER DEFAULT 0, duree_allouee INTEGER DEFAULT 60, montant INTEGER, tarif_prix INTEGER, jetons_gagnes INTEGER DEFAULT 0, statut TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "tarifs" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, nom TEXT, type TEXT, prix INTEGER, duree_minutes INTEGER, description TEXT, actif INTEGER DEFAULT 1, console_type TEXT, jeu TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "factures" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, numero_facture TEXT UNIQUE, session_id BIGINT, joueur_id BIGINT, montant_ht DOUBLE PRECISION, taux_tva DOUBLE PRECISION DEFAULT 20, montant_tva DOUBLE PRECISION, montant_ttc DOUBLE PRECISION, mode_paiement TEXT, statut TEXT, date_paiement TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "jetons_transactions" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, joueur_id BIGINT, quantite INTEGER, type TEXT, raison TEXT, session_id BIGINT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "messages" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, titre TEXT, contenu TEXT, auteur TEXT, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "parametres_fidelite" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, regle_type TEXT, seuil INTEGER, jetons_attribues INTEGER, valeur_jeton BIGINT DEFAULT 100, actif INTEGER DEFAULT 1, deleted INTEGER DEFAULT 0);
ALTER TABLE "parametres_fidelite" ADD COLUMN IF NOT EXISTS valeur_jeton BIGINT DEFAULT 100;
CREATE TABLE IF NOT EXISTS "lignes_facture" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, facture_id BIGINT, description TEXT, quantite INTEGER, prix_unitaire INTEGER, total_ligne INTEGER, created_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "hangouts" (id BIGINT PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY, titre TEXT, activite TEXT, prix INTEGER, actif INTEGER DEFAULT 1, created_at TEXT, updated_at TEXT, deleted INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS "app_settings" (key TEXT PRIMARY KEY, value TEXT);
"#;
    match supabase_batch_execute(pool, ddl).await {
        Ok(_) => {
            crate::logger::log_cloud("migration schéma cloud OK (tables créées/vérifiées)");
            Ok(())
        }
        Err(e) => {
            eprintln!("[supabase] migration schéma cloud failed: {}", e);
            crate::logger::log_cloud(&format!("migration schéma cloud failed: {}", e));
            Err(e)
        }
    }
}

/// Index unique sur id (anti-doublons upsert). Exécuté une seule fois par table via
/// le flag cloud_migration_v2 : les tables créées par ensure_cloud_schema ont déjà
/// id PRIMARY KEY, l'index ne sert que pour les bases cloud anciennes.
#[cfg(feature = "supabase-sync")]
pub async fn ensure_table_unique_id(pool: &SupabasePool, table: &str) -> Result<(), String> {
    let sql = format!("CREATE UNIQUE INDEX IF NOT EXISTS uq_{}_id ON \"{}\" (id)", table, table);
    match supabase_batch_execute(pool, &sql).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Ajoute la colonne soft-delete `deleted` sur les tables Supabase (idempotent).
/// Rien n'est JAMAIS supprimé physiquement sur Supabase : une ligne supprimée passe
/// deleted=1 et est simplement cachée (pull exclut deleted=true).
#[cfg(feature = "supabase-sync")]
pub async fn ensure_deleted_columns(pool: &SupabasePool) {
    let tables = ["users", "consoles", "jeux", "joueurs", "sessions_jeu", "tarifs", "factures", "jetons_transactions", "messages", "parametres_fidelite", "lignes_facture"];
    for table in tables {
        let sql = format!("ALTER TABLE \"{}\" ADD COLUMN IF NOT EXISTS deleted INTEGER NOT NULL DEFAULT 0", table);
        match supabase_batch_execute(pool, &sql).await {
            Ok(_) => {}
            Err(e) => eprintln!("[supabase] ensure deleted {} failed: {}", table, e),
        }
    }
    // Durée allouée (expiration auto des sessions) : ajout idempotent sur les bases
    // cloud créées avant cette colonne.
    let sql = "ALTER TABLE \"sessions_jeu\" ADD COLUMN IF NOT EXISTS duree_allouee INTEGER DEFAULT 60";
    if let Err(e) = supabase_batch_execute(pool, sql).await {
        eprintln!("[supabase] ensure duree_allouee failed: {}", e);
    }
    // Précision seconde du temps joué (idempotent) — duree_minutes reste pour compat.
    let sql = "ALTER TABLE \"sessions_jeu\" ADD COLUMN IF NOT EXISTS duree_secondes INTEGER DEFAULT 0";
    if let Err(e) = supabase_batch_execute(pool, sql).await {
        eprintln!("[supabase] ensure duree_secondes failed: {}", e);
    }
    // Visuels des entités (item 6) : sticker joueur + image console/jeu.
    for sql in [
        "ALTER TABLE \"joueurs\" ADD COLUMN IF NOT EXISTS sticker TEXT",
        "ALTER TABLE \"consoles\" ADD COLUMN IF NOT EXISTS image_url TEXT",
        "ALTER TABLE \"jeux\" ADD COLUMN IF NOT EXISTS image_url TEXT",
    ] {
        if let Err(e) = supabase_batch_execute(pool, sql).await {
            eprintln!("[supabase] ensure image columns failed: {}", e);
        }
    }
    // FIX utilisateurs fantômes : l'unicité email ne doit s'appliquer qu'aux
    // lignes VIVANTES. Avec UNIQUE(email) simple, recréer un utilisateur
    // supprimé faisait échouer le push (batch entier perdu) ou bloquait la
    // recréation locale. Nécessite le privilège CREATE INDEX (owner/postgres).
    if let Err(e) = supabase_batch_execute(
        pool,
        "DROP INDEX IF EXISTS idx_users_email_alive; CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_alive ON \"users\" (email) WHERE COALESCE(deleted, 0) = 0",
    )
    .await
    {
        crate::logger::log_cloud(&format!("ensure idx_users_email_alive failed (à exécuter manuellement dans l'éditeur SQL Supabase): {}", e));
    }
    // RLS : la table journal est écrite par les appareils (service postgres).
    // Les tables métier restent accessibles via la connexion postgres ; on
    // documente les politiques attendues si RLS est activé sur le projet.
    if let Err(e) = supabase_batch_execute(
        pool,
        "ALTER TABLE \"sync_changes\" ENABLE ROW LEVEL SECURITY; \
         DROP POLICY IF EXISTS sync_changes_all ON \"sync_changes\"; \
         CREATE POLICY sync_changes_all ON \"sync_changes\" FOR ALL USING (true) WITH CHECK (true)",
    )
    .await
    {
        crate::logger::log_cloud(&format!("ensure sync_changes RLS failed: {}", e));
    }
}

// ===== MOTEUR DELTA SYNC (étapes 2-4 de ~/mod.md) =====

/// Schéma du moteur delta sync côté Supabase : journal partagé IMMUABLE
/// (sync_changes, spec §2) + registre des appareils (sync_peers, pour le
/// nettoyage §12). Idempotent, exécuté à chaque sync.
#[cfg(feature = "supabase-sync")]
pub async fn ensure_sync_schema(pool: &SupabasePool) -> Result<(), String> {
    let ddl = r#"
CREATE TABLE IF NOT EXISTS "sync_changes" (
    sequence BIGSERIAL PRIMARY KEY,
    change_id TEXT NOT NULL UNIQUE,
    device_id TEXT NOT NULL,
    device_sequence BIGINT NOT NULL,
    operation TEXT NOT NULL,
    entity TEXT NOT NULL,
    record_id BIGINT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_sync_changes_seq ON sync_changes(sequence);
CREATE TABLE IF NOT EXISTS "sync_peers" (
    device_id TEXT PRIMARY KEY,
    last_received BIGINT DEFAULT 0,
    last_uploaded BIGINT DEFAULT 0,
    last_seen TIMESTAMPTZ DEFAULT now()
);
"#;
    match supabase_batch_execute(pool, ddl).await {
        Ok(_) => {
            crate::logger::log_cloud("schéma delta sync OK (sync_changes + sync_peers)");
            Ok(())
        }
        Err(e) => {
            eprintln!("[supabase] migration delta sync failed: {}", e);
            Err(e)
        }
    }
}

/// Dernière séquence du journal cloud (0 si vide) — pour initialiser le curseur.
#[cfg(feature = "supabase-sync")]
pub async fn sync_max_sequence(pool: &SupabasePool) -> Result<i64, String> {
    match supabase_query(pool, "SELECT COALESCE(MAX(sequence), 0) FROM sync_changes", &[]).await {
        Ok(rows) => {
            if rows.is_empty() {
                Ok(0)
            } else {
                Ok(pg_col_to_i64(&rows[0], 0).unwrap_or(0))
            }
        }
        Err(e) => Err(e),
    }
}

/// Nombre de changements restants dans le journal après un curseur (delta sync).
/// Sert au TOTAL de la barre de progression delta — un seul COUNT au lieu de
/// télécharger la base (mission §4 : ne jamais re-télécharger tout).
#[cfg(feature = "supabase-sync")]
pub async fn sync_changes_count_after(pool: &SupabasePool, after: i64) -> Result<i64, String> {
    let sql = format!("SELECT COUNT(*) FROM sync_changes WHERE sequence > {}", after);
    match supabase_query(pool, &sql, &[]).await {
        Ok(rows) => {
            if rows.is_empty() {
                Ok(0)
            } else {
                Ok(pg_col_to_i64(&rows[0], 0).unwrap_or(0))
            }
        }
        Err(e) => Err(e),
    }
}

/// Script SQL d'UN événement outbox : journal sync_changes (idempotent via
/// ON CONFLICT DO NOTHING) + application à la table cible (upsert ou tombstone).
/// Retourne None si l'événement est invalide. Protocole SIMPLE (batch_execute).
#[cfg(feature = "supabase-sync")]
fn build_change_script(
    change: &Value,
    cols_cache: &std::collections::HashMap<String, Vec<String>>,
) -> Option<String> {
    let Some(obj) = change.as_object() else { return None };
    let change_id = obj.get("change_id").and_then(Value::as_str).unwrap_or_default();
    let device_id = obj.get("device_id").and_then(Value::as_str).unwrap_or_default();
    let device_seq = obj.get("device_sequence").and_then(Value::as_i64).unwrap_or(0);
    let operation = obj.get("operation").and_then(Value::as_str).unwrap_or_default();
    let entity = obj.get("table_name").and_then(Value::as_str).unwrap_or_default();
    let record_id = obj.get("record_id").and_then(Value::as_i64).unwrap_or(0);
    let payload_str = obj.get("payload").and_then(Value::as_str).unwrap_or_default();
    if change_id.is_empty() || entity.is_empty() || record_id == 0 || payload_str.is_empty() {
        return None;
    }
    let mut out = format!(
        "INSERT INTO sync_changes (change_id, device_id, device_sequence, operation, entity, record_id, payload, created_at) VALUES ('{}', '{}', {}, '{}', '{}', {}, '{}'::jsonb, now()) ON CONFLICT (change_id) DO NOTHING;",
        escape_sql(change_id),
        escape_sql(device_id),
        device_seq,
        escape_sql(operation),
        escape_sql(entity),
        record_id,
        escape_sql(payload_str),
    );
    if operation == "DELETE" {
        let is_perm = serde_json::from_str::<Value>(payload_str)
            .ok()
            .and_then(|v| v.get("permanent_delete").and_then(Value::as_bool))
            .unwrap_or(false);
        if is_perm {
            let cascade = match entity {
                "factures" => format!(
                    "DELETE FROM lignes_facture WHERE facture_id = {record_id};                      DELETE FROM jetons_transactions WHERE facture_id = {record_id};                      DELETE FROM factures WHERE id = {record_id};"
                ),
                "tarifs" => format!(
                    "UPDATE sessions_jeu SET tarif_id = NULL WHERE tarif_id = {record_id};                      DELETE FROM tarifs WHERE id = {record_id};"
                ),
                "consoles" => format!(
                    "UPDATE sessions_jeu SET console_id = NULL WHERE console_id = {record_id};                      UPDATE jeux SET console_id = NULL WHERE console_id = {record_id};                      DELETE FROM consoles WHERE id = {record_id};"
                ),
                "jeux" => format!(
                    "UPDATE sessions_jeu SET jeu_id = NULL WHERE jeu_id = {record_id};                      DELETE FROM jeux WHERE id = {record_id};"
                ),
                "joueurs" => format!(
                    "DELETE FROM jetons_transactions WHERE joueur_id = {record_id};                      UPDATE factures SET joueur_id = NULL WHERE joueur_id = {record_id};                      UPDATE sessions_jeu SET joueur_id = NULL WHERE joueur_id = {record_id};                      DELETE FROM joueurs WHERE id = {record_id};"
                ),
                "sessions_jeu" => format!(
                    "DELETE FROM lignes_facture WHERE facture_id IN (SELECT id FROM factures WHERE session_id = {record_id});                      DELETE FROM factures WHERE session_id = {record_id};                      DELETE FROM jetons_transactions WHERE session_id = {record_id};                      DELETE FROM sessions_jeu WHERE id = {record_id};"
                ),
                _ => format!("DELETE FROM \"{}\" WHERE id={};", entity, record_id),
            };
            out.push_str(&cascade);
            return Some(out);
        } else {
            out.push_str(&format!("UPDATE \"{}\" SET deleted=1 WHERE id={};", entity, record_id));
            return Some(out);
        }
    }
    let Some(payload) = serde_json::from_str::<Value>(payload_str).ok() else { return None };
    let Some(pmap) = payload.as_object() else { return None };
    let entity_s = entity.to_string();
    let cols = cols_cache.get(&entity_s).map(|c| c.clone()).unwrap_or_default();
    let mut keys: Vec<String> = Vec::new();
    for k in pmap.keys() {
        if *k == "id" || !cols.contains(k) {
            continue;
        }
        if pmap[k].is_null() {
            continue;
        }
        keys.push(k.to_string());
    }
    keys.sort();
    if keys.is_empty() {
        return None;
    }
    let mut col_list: Vec<String> = vec!["id".to_string()];
    let mut val_list: Vec<String> = vec![format!("{}", record_id)];
    for k in &keys {
        let Some(v) = pmap.get(k.as_str()) else { continue };
        col_list.push(k.clone());
        val_list.push(format!("'{}'", escape_sql(&val_to_text(v))));
    }
    let updates: Vec<String> = keys
        .iter()
        .filter(|c| *c != "id")
        .map(|c| format!("{}=EXCLUDED.{}", c, c))
        .collect();
    out.push_str(&format!(
        "INSERT INTO \"{}\" ({}) VALUES ({}) ON CONFLICT (id) DO UPDATE SET {};",
        entity,
        col_list.join(","),
        val_list.join(","),
        updates.join(","),
    ));
    Some(out)
}

/// Push d'un batch d'événements outbox vers Supabase : le script complet part
/// en UN seul aller-retour (protocole simple, atomique). Si un événement est
/// invalide (colonne inconnue...), retry événement par événement pour isoler.
/// Retourne (change_ids ACKed, change_ids en échec DÉFINITIF).
///
/// IMPORTANT : seuls les changements réellement invalides (script
/// non constructible avec un schéma cloud connu) sont renvoyés en échec
/// définitif. Tout échec d'exécution (réseau, timeout, contrainte momentanée)
/// laisse le changement PENDING : il sera rejoué, jamais perdu.
#[cfg(feature = "supabase-sync")]
static CLOUD_COLS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>,
> = std::sync::OnceLock::new();

#[cfg(feature = "supabase-sync")]
/// Colonnes d'une table cloud, avec cache mémoire (un seul aller-retour réseau
/// par table et par lancement de l'application).
pub async fn table_columns_cached(pool: &SupabasePool, table: &str) -> Vec<String> {
    let cache = CLOUD_COLS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(cols) = guard.get(table) {
            if !cols.is_empty() {
                return cols.clone();
            }
        }
    }
    let cols = supabase_table_columns(pool, table).await.unwrap_or_default();
    if !cols.is_empty() {
        if let Ok(mut guard) = cache.lock() {
            guard.insert(table.to_string(), cols.clone());
        }
    }
    cols
}

#[cfg(feature = "supabase-sync")]
pub async fn push_outbox_batch(
    pool: &SupabasePool,
    changes: &Vec<Value>,
) -> Result<(Vec<String>, Vec<String>), String> {
    if changes.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Colonnes cloud par table (cache : un lot touche souvent 1-2 tables)
    let mut tables: Vec<String> = Vec::new();
    for change in changes {
        let entity = change.get("table_name").and_then(Value::as_str).unwrap_or_default();
        if !entity.is_empty() && !tables.contains(&entity.to_string()) {
            tables.push(entity.to_string());
        }
    }
    let mut cols_cache: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for t in tables {
        // Colonnes mises en cache POUR TOUTE LA DURÉE DU PROCESSUS : la structure
        // des tables cloud ne change pas en cours de route, et cette requête
        // partait à chaque lot envoyé (un aller-retour réseau par table et par
        // lot). Sur une connexion mobile, c'était une part importante du temps
        // de synchronisation.
        let cols = table_columns_cached(pool, &t).await;
        cols_cache.insert(t, cols);
    }
    let mut scripts: Vec<String> = Vec::new();
    for change in changes {
        if let Some(s) = build_change_script(change, &cols_cache) {
            scripts.push(s);
        }
    }
    // Un changement n'est DÉFINITIVEMENT rejeté (FAILED) que si son script est
    // invalide ALORS QUE les colonnes cloud de sa table ont bien été lues. Si le
    // schéma n'a pas pu être lu (réseau coupé, table pas encore migrée), l'échec
    // est TRANSITOIRE : on laisse PENDING pour réessayer.
    //
    // C'est LA correction du bug « suppression définitive appliquée en local mais
    // jamais sur Supabase » : auparavant, le moindre échec réseau marquait les
    // changements FAILED, et l'outbox n'étant relu qu'en PENDING, ils étaient
    // perdus à jamais (jamais renvoyés).
    let invalide_definitif = |c: &Value| -> bool {
        if build_change_script(c, &cols_cache).is_some() {
            return false;
        }
        let entity = c.get("table_name").and_then(Value::as_str).unwrap_or_default();
        cols_cache
            .get(entity)
            .map(|cols| !cols.is_empty())
            .unwrap_or(false)
    };
    let change_id_of = |c: &Value| -> Option<String> {
        c.get("change_id").and_then(Value::as_str).map(str::to_string)
    };
    if scripts.is_empty() {
        // Aucun script constructible : on ne condamne que les vrais invalides,
        // les autres restent PENDING (rejoués au prochain cycle).
        let failed = changes
            .iter()
            .filter(|c| invalide_definitif(c))
            .filter_map(change_id_of)
            .collect::<Vec<_>>();
        return Ok((Vec::new(), failed));
    }
    let valid_ids = changes
        .iter()
        .filter_map(|c| {
            build_change_script(c, &cols_cache)?;
            c.get("change_id").and_then(Value::as_str).map(str::to_string)
        })
        .collect::<Vec<_>>();
    let failed_invalid = changes
        .iter()
        .filter(|c| invalide_definitif(c))
        .filter_map(change_id_of)
        .collect::<Vec<_>>();
    match supabase_batch_execute(pool, &scripts.join("\n")).await {
        Ok(_) => Ok((valid_ids, failed_invalid)),
        Err(_) => {
            // Échec global : retry un par un pour isoler les erreurs applicatives.
            // Un échec d'EXÉCUTION n'est jamais définitif : le changement reste
            // PENDING (rejoué plus tard), sinon une coupure réseau ferait
            // disparaître définitivement la donnée ou la suppression.
            let mut acked: Vec<String> = Vec::new();
            for change in changes {
                let Some(script) = build_change_script(change, &cols_cache) else { continue };
                if supabase_batch_execute(pool, &script).await.is_ok() {
                    acked.push(change.get("change_id").and_then(Value::as_str).unwrap_or_default().to_string());
                }
            }
            Ok((acked, failed_invalid))
        }
    }
}

/// Pull delta : SEULEMENT les changements plus récents que le curseur (spec §4).
/// Retourne des objets {sequence, change_id, device_id, operation, entity,
/// record_id, payload, created_at} triés par séquence croissante.
#[cfg(feature = "supabase-sync")]
pub async fn pull_delta(pool: &SupabasePool, after: i64, limit: i64) -> Result<Vec<Value>, String> {
    let sql = format!(
        "SELECT sequence, change_id, device_id, operation, entity, record_id, payload::text, created_at::text FROM sync_changes WHERE sequence > {} ORDER BY sequence ASC LIMIT {}",
        after,
        limit,
    );
    match supabase_query(pool, &sql, &[]).await {
        Ok(rows) => {
            let mut out: Vec<Value> = Vec::with_capacity(rows.len());
            for row in rows {
                let mut m = serde_json::Map::new();
                m.insert("sequence".into(), json!(pg_col_to_i64(&row, 0).unwrap_or(0)));
                m.insert("change_id".into(), json!(pg_col_to_string_pub(&row, 1).unwrap_or_default()));
                m.insert("device_id".into(), json!(pg_col_to_string_pub(&row, 2).unwrap_or_default()));
                m.insert("operation".into(), json!(pg_col_to_string_pub(&row, 3).unwrap_or_default()));
                m.insert("entity".into(), json!(pg_col_to_string_pub(&row, 4).unwrap_or_default()));
                m.insert("record_id".into(), json!(pg_col_to_i64(&row, 5).unwrap_or(0)));
                let payload_str = pg_col_to_string_pub(&row, 6).unwrap_or_else(|| "{}".to_string());
                m.insert("payload".into(), serde_json::from_str::<Value>(&payload_str).ok().unwrap_or(Value::Object(serde_json::Map::new())));
                m.insert("created_at".into(), json!(pg_col_to_string_pub(&row, 7).unwrap_or_default()));
                out.push(Value::Object(m));
            }
            Ok(out)
        }
        Err(e) => Err(e),
    }
}

/// Enregistre le curseur de cet appareil (pour le nettoyage du journal §12).
#[cfg(feature = "supabase-sync")]
pub async fn peer_register(pool: &SupabasePool, device_id: &str, last_received: i64, last_uploaded: i64) -> Result<(), String> {
    let sql = format!(
        "INSERT INTO sync_peers (device_id, last_received, last_uploaded, last_seen) VALUES ('{}', {}, {}, now()) ON CONFLICT (device_id) DO UPDATE SET last_received = EXCLUDED.last_received, last_uploaded = EXCLUDED.last_uploaded, last_seen = now()",
        escape_sql(device_id),
        last_received,
        last_uploaded,
    );
    match supabase_batch_execute(pool, &sql).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Nettoyage du journal partagé : supprime les changements que TOUS les
/// appareils connus ont dépassés (marge 1000). Sûr : si un appareil n'a jamais
/// reporté son curseur (MIN=0), rien n'est supprimé (spec §12).
#[cfg(feature = "supabase-sync")]
pub async fn sync_changes_cleanup(pool: &SupabasePool) -> Result<(), String> {
    let sql = "DELETE FROM sync_changes WHERE sequence < (SELECT COALESCE(MIN(last_received), 0) FROM sync_peers) - 1000 AND (SELECT COALESCE(MIN(last_received), 0) FROM sync_peers) > 1000";
    match supabase_batch_execute(pool, sql).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

// Pull toutes les tables pour restauration complète si app data vidé.
// IMPORTANT : on utilise json_agg (fonction STANDARD Postgres) au lieu de
// row_to_json (fonction custom qui n'existe pas forcément sur Supabase -> la requête
// échouait -> sync_run abort -> AUCUNE donnée écrite localement).
// Les lignes soft-deleted (deleted=true) ne sont JAMAIS repullées -> une donnée
// supprimée ne réapparaît pas après sync.
// Une table absente du schéma Supabase est ignorée (log + continue) ; seule une erreur
// de connexion est propagée (déclenche la reconnexion en arrière-plan).
/// True si la valeur `deleted` d'une ligne pullée indique une suppression.
/// Gère les DEUX types possibles côté Supabase : boolean (true/false) et integer (1/0).
#[cfg(feature = "supabase-sync")]
fn is_deleted_value(v: &Value) -> bool {
    v.as_bool().unwrap_or(false) || v.as_i64().unwrap_or(0) == 1
}

/// Normalise une ligne pullée : `deleted` est ramené à l'entier 0/1 attendu par
/// SQLite, quel que soit son type côté Postgres (boolean ou integer).
///
/// IMPORTANT : les lignes archivées ne sont PLUS jetées ici. Avant, tout pull
/// faisait `if deleted { continue; }`, donc une archive faite sur un autre
/// appareil n'arrivait jamais : la ligne restait ACTIVE en local et réapparaissait
/// même après suppression. `apply_remote_change` sait traiter ces tombstones.
#[cfg(feature = "supabase-sync")]
fn normalize_pulled_row(item: &Value) -> Value {
    let mut row = item.clone();
    let is_del = row.get("deleted").map(is_deleted_value).unwrap_or(false);
    if let Some(obj) = row.as_object_mut() {
        obj.insert("deleted".into(), json!(if is_del { 1 } else { 0 }));
    }
    row
}

#[cfg(feature = "supabase-sync")]
pub async fn pull_all(pool: &SupabasePool) -> ApiResult<std::collections::HashMap<String, Vec<Value>>> {
    // COMPTES 100 % EN LIGNE : `users` est volontairement absente — un snapshot ne
    // doit jamais rapatrier de comptes (ni leurs hash) sur l'appareil.
    let tables = ["consoles", "jeux", "joueurs", "sessions_jeu", "tarifs", "factures", "jetons_transactions", "messages", "parametres_fidelite", "lignes_facture"];
    // RAPIDE : TOUTES les tables en UNE seule requête (UNION ALL de json_agg) ->
    // 1 aller-retour réseau au lieu de 11. Si une table manque (base cloud neuve),
    // la requête échoue -> fallback par table (résilient, comme avant).
    let sql = tables.iter().map(|t| {
        format!("SELECT '{}' AS t, COALESCE(json_agg(s)::text, '[]') AS j FROM (SELECT * FROM \"{}\") s", t, t)
    }).collect::<Vec<_>>().join(" UNION ALL ");
    let mut all = std::collections::HashMap::new();
    match supabase_query(pool, &sql, &[]).await {
        Ok(rows) => {
            for row in rows {
                let table = pg_col_to_string_pub(&row, 0).unwrap_or_default();
                let json_str = pg_col_to_string_pub(&row, 1).unwrap_or_else(|| "[]".to_string());
                let mut vec = Vec::new();
                if let Ok(v) = serde_json::from_str::<Value>(&json_str) {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            // Les archives sont CONSERVÉES : c'est ce qui propage
                            // les suppressions (tombstones) entre appareils.
                            vec.push(normalize_pulled_row(item));
                        }
                    }
                }
                eprintln!("[supabase] pull {}: {} rows", table, vec.len());
                all.insert(table.to_string(), vec);
            }
            Ok(all)
        }
        Err(e) => {
            eprintln!("[supabase] pull UNION ALL failed ({}), fallback par table", e);
            pull_all_fallback(pool, &tables).await
        }
    }
}

/// Fallback résilient : pull table par table (une table absente ne bloque pas les autres).
/// Seule une erreur sur TOUTES les tables (connexion morte) est propagée.
#[cfg(feature = "supabase-sync")]
pub async fn pull_table_all(pool: &SupabasePool, table: &str) -> ApiResult<Vec<Value>> {
    let sql = format!("SELECT COALESCE(json_agg(t)::text, '[]') FROM (SELECT * FROM \"{}\" ORDER BY id ASC) t", table);
    match supabase_query(pool, &sql, &[]).await {
        Ok(jrows) => {
            let mut vec = Vec::new();
            if !jrows.is_empty() {
                if let Ok(s) = jrows[0].try_get::<_, String>(0) {
                    if let Ok(v) = serde_json::from_str::<Value>(&s) {
                        if let Some(arr) = v.as_array() {
                            for item in arr {
                                let mut row = item.clone();
                                let is_del = row.get("deleted").map(is_deleted_value).unwrap_or(false);
                                if let Some(obj) = row.as_object_mut() {
                                    obj.insert("deleted".into(), json!(is_del));
                                }
                                vec.push(row);
                            }
                        }
                    }
                }
            }
            Ok(vec)
        }
        Err(e) => Err(ApiError::internal(format!("Supabase pull_table_all {table}: {e}"))),
    }
}

pub async fn pull_table(pool: &SupabasePool, table: &str) -> ApiResult<Vec<Value>> {
    let sql = format!("SELECT COALESCE(json_agg(t)::text, '[]') FROM (SELECT * FROM \"{}\") t", table);
    match supabase_query(pool, &sql, &[]).await {
        Ok(jrows) => {
            let mut vec = Vec::new();
            if !jrows.is_empty() {
                if let Ok(s) = jrows[0].try_get::<_, String>(0) {
                    if let Ok(v) = serde_json::from_str::<Value>(&s) {
                        if let Some(arr) = v.as_array() {
                            for item in arr {
                                // Archives conservées : la sync initiale doit aussi
                                // rapatrier les lignes supprimées, sinon une
                                // suppression faite ailleurs ne s'applique jamais ici.
                                vec.push(normalize_pulled_row(item));
                            }
                        }
                    }
                }
            }
            Ok(vec)
        }
        Err(e) => {
            // Table absente du schéma cloud (base neuve) -> vide, pas une erreur
            if e.contains("does not exist") {
                return Ok(Vec::new());
            }
            Err(ApiError::internal(format!("pull {table}: {e}")))
        }
    }
}

#[cfg(feature = "supabase-sync")]
async fn pull_all_fallback(pool: &SupabasePool, tables: &[&str]) -> ApiResult<std::collections::HashMap<String, Vec<Value>>> {
    let mut all = std::collections::HashMap::new();
    let mut errors = 0;
    for table in tables {
        let sql = format!("SELECT COALESCE(json_agg(t)::text, '[]') FROM (SELECT * FROM \"{}\") t", table);
        match supabase_query(pool, &sql, &[]).await {
            Ok(jrows) => {
                let mut vec = Vec::new();
                if !jrows.is_empty() {
                    if let Ok(s) = jrows[0].try_get::<_, String>(0) {
                        if let Ok(v) = serde_json::from_str::<Value>(&s) {
                            if let Some(arr) = v.as_array() {
                                for item in arr {
                                    vec.push(normalize_pulled_row(item));
                                }
                            }
                        }
                    }
                }
                eprintln!("[supabase] pull {}: {} rows", table, vec.len());
                all.insert(table.to_string(), vec);
            }
            Err(e) => {
                eprintln!("[supabase] pull {} failed: {}", table, e);
                crate::logger::log_cloud(&format!("pull {} failed: {}", table, e));
                errors += 1;
                all.insert(table.to_string(), Vec::new());
            }
        }
    }
    if errors == tables.len() && !tables.is_empty() {
        return Err(ApiError::internal("Supabase pull: toutes les tables en erreur (connexion morte ?)"));
    }
    Ok(all)
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn pull_all(_pool: &()) -> ApiResult<std::collections::HashMap<String, Vec<Value>>> {
    Ok(std::collections::HashMap::new())
}

/// Colonnes d'une table Supabase via information_schema (standard Postgres).
/// Utilisé pour ne jamais envoyer vers Supabase une colonne qui n'existe pas côté cloud
/// (sinon erreur SQL silencieuse -> "l'envoi ne fonctionne pas").
/// SANS paramètre ($1) : les requêtes avec paramètres utilisent le protocole étendu
/// (prepared statements) que le pooler Supabase ne supporte pas — le nom de table est
/// mis en dur (échappé par le format).
#[cfg(feature = "supabase-sync")]
pub async fn supabase_table_columns(pool: &SupabasePool, table: &str) -> Result<Vec<String>, String> {
    let sql = format!("SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = '{}' ORDER BY ordinal_position", table);
    let rows = supabase_query(pool, &sql, &[]).await;
    match rows {
        Ok(rs) => {
            let mut out = Vec::new();
            for r in rs {
                if let Some(s) = pg_col_to_string_pub(&r, 0) {
                    out.push(s);
                }
            }
            Ok(out)
        }
        Err(e) => Err(e),
    }
}

#[cfg(feature = "supabase-sync")]
/// Convertit une valeur sérialisée en texte : Postgres caste text -> type de la colonne
/// cible à l'insert (int, numeric, timestamptz, ...). Évite de devoir construire des
/// paramètres typés dynamiquement (le protocole tokio-postgres exige des références).
fn val_to_text(v: &Value) -> String {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                format!("{}", i)
            } else if let Some(f) = n.as_f64() {
                format!("{}", f)
            } else {
                "".to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Bool(b) => format!("{}", *b),
        _ => "".to_string(),
    }
}

/// Type du pool cloud, quelle que soit la configuration de compilation : permet
/// d'écrire des commandes (voir commands/users.rs) sans `#[cfg]` partout.
#[cfg(feature = "supabase-sync")]
pub type CloudPool = SupabasePool;
#[cfg(not(feature = "supabase-sync"))]
pub type CloudPool = ();

// ============================================================================
// COMPTES 100 % EN LIGNE
// ----------------------------------------------------------------------------
// Le cloud est la SEULE source de vérité pour la table `users`. L'appareil ne
// conserve jamais de mot de passe (ni en clair, ni haché) : il garde seulement
// un annuaire d'affichage (id, email, nom, rôle) pour pouvoir écrire « démarré
// par X » sur une session.
//
// Comme `fetch_supabase_user`, ces requêtes mettent les valeurs en dur (après
// échappement) : le pooler Supabase ne supporte pas les prepared statements du
// protocole étendu, les paramètres $1 cassent la requête.
// ============================================================================

#[cfg(feature = "supabase-sync")]
fn ligne_compte(row: &tokio_postgres::Row) -> Value {
    json!({
        "id": row.try_get::<_, i64>(0).unwrap_or(0),
        "email": pg_col_to_string(row, 1).unwrap_or_default(),
        "role": pg_col_to_string(row, 2).unwrap_or_else(|| "employe".to_string()),
        "nom": pg_col_to_string(row, 3).unwrap_or_default(),
        "created_at": pg_col_to_string(row, 4),
        "deleted": row.try_get::<_, i32>(5).unwrap_or(0),
    })
}

/// Colonnes lues pour un compte : JAMAIS `password_hash` — un hash ne doit pas
/// même transiter jusqu'à l'écran d'administration.
#[cfg(feature = "supabase-sync")]
const COLONNES_COMPTE: &str =
    "id, email, role, nom, created_at, COALESCE(deleted::int, 0)";

/// Liste des comptes, directement depuis le cloud.
#[cfg(feature = "supabase-sync")]
pub async fn cloud_users_list(pool: &SupabasePool, inclure_archives: bool) -> ApiResult<Vec<Value>> {
    let filtre = if inclure_archives { "" } else { "WHERE COALESCE(deleted::int, 0) = 0" };
    let sql = format!("SELECT {COLONNES_COMPTE} FROM users {filtre} ORDER BY id");
    let rows = supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Liste des comptes en ligne : {e}")))?;
    Ok(rows.iter().map(ligne_compte).collect())
}

/// Un compte précis (par id), directement depuis le cloud.
#[cfg(feature = "supabase-sync")]
pub async fn cloud_user_get(pool: &SupabasePool, id: i64) -> ApiResult<Option<Value>> {
    let sql = format!("SELECT {COLONNES_COMPTE} FROM users WHERE id = {id} LIMIT 1");
    let rows = supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Lecture du compte en ligne : {e}")))?;
    Ok(rows.first().map(ligne_compte))
}

/// Création (ou réactivation) d'un compte dans le cloud. Le hash est calculé sur
/// l'appareil puis envoyé ; il n'est jamais écrit en SQLite.
#[cfg(feature = "supabase-sync")]
pub async fn cloud_user_create(
    pool: &SupabasePool,
    email: &str,
    password_hash: &str,
    nom: &str,
    role: &str,
) -> ApiResult<Value> {
    let sql = format!(
        "INSERT INTO users (email, password_hash, nom, role, created_at, deleted) \
         VALUES ('{}', '{}', '{}', '{}', '{}', 0) \
         ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash, \
         nom = EXCLUDED.nom, role = EXCLUDED.role, deleted = 0 \
         RETURNING {COLONNES_COMPTE}",
        escape_sql(&email.to_lowercase()),
        escape_sql(password_hash),
        escape_sql(nom),
        escape_sql(role),
        escape_sql(&crate::db::now_iso()),
    );
    let rows = supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Création du compte en ligne : {e}")))?;
    rows.first()
        .map(ligne_compte)
        .ok_or_else(|| ApiError::internal("Le serveur n'a pas confirmé la création du compte"))
}

/// Mise à jour d'un compte dans le cloud. `champs` contient déjà des valeurs
/// prêtes (le mot de passe est transmis haché).
#[cfg(feature = "supabase-sync")]
pub async fn cloud_user_update(
    pool: &SupabasePool,
    id: i64,
    champs: &[(&str, String)],
) -> ApiResult<Value> {
    if champs.is_empty() {
        return Err(ApiError::bad_request("Aucun champ à modifier"));
    }
    let sets: Vec<String> = champs
        .iter()
        .map(|(col, val)| format!("{col} = '{}'", escape_sql(val)))
        .collect();
    let sql = format!(
        "UPDATE users SET {} WHERE id = {id} RETURNING {COLONNES_COMPTE}",
        sets.join(", ")
    );
    let rows = supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Modification du compte en ligne : {e}")))?;
    rows.first()
        .map(ligne_compte)
        .ok_or_else(|| ApiError::not_found("Ce compte n'existe plus sur le serveur"))
}

/// Archivage / restauration d'un compte dans le cloud (suppression douce).
#[cfg(feature = "supabase-sync")]
pub async fn cloud_user_set_deleted(pool: &SupabasePool, id: i64, archive: bool) -> ApiResult<()> {
    let flag = if archive { 1 } else { 0 };
    let sql = format!("UPDATE users SET deleted = {flag} WHERE id = {id}");
    supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Archivage du compte en ligne : {e}")))?;
    Ok(())
}

/// Suppression DÉFINITIVE d'un compte dans le cloud.
#[cfg(feature = "supabase-sync")]
pub async fn cloud_user_permanent_delete(pool: &SupabasePool, id: i64) -> ApiResult<()> {
    let sql = format!("DELETE FROM users WHERE id = {id}");
    supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Suppression du compte en ligne : {e}")))?;
    Ok(())
}

/// État du compte connecté : existe-t-il encore, et est-il archivé ?
/// Sert à la détection de suppression (voir `account_watcher`).
#[cfg(feature = "supabase-sync")]
pub async fn cloud_user_statut(pool: &SupabasePool, id: i64, email: &str) -> ApiResult<Value> {
    let sql = format!(
        "SELECT {COLONNES_COMPTE} FROM users WHERE id = {id} OR email = '{}' LIMIT 1",
        escape_sql(&email.to_lowercase())
    );
    let rows = supabase_query(pool, &sql, &[])
        .await
        .map_err(|e| ApiError::internal(format!("Vérification du compte : {e}")))?;
    match rows.first() {
        None => Ok(json!({ "existe": false, "archive": false })),
        Some(row) => {
            let compte = ligne_compte(row);
            let archive = compte.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1;
            Ok(json!({
                "existe": true,
                "archive": archive,
                "id": compte.get("id"),
                "email": compte.get("email"),
                "nom": compte.get("nom"),
                "role": compte.get("role"),
            }))
        }
    }
}

// ---- Variantes sans la feature cloud : la gestion des comptes est impossible,
//      et on le dit clairement au lieu de retomber sur des données locales. ----

#[cfg(not(feature = "supabase-sync"))]
fn hors_ligne<T>() -> ApiResult<T> {
    Err(ApiError::service_unavailable(
        "La gestion des comptes est en ligne uniquement (version sans cloud)",
    ))
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn get_supabase_pool(_state: &crate::AppState) -> ApiResult<()> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_users_list(_pool: &(), _inclure_archives: bool) -> ApiResult<Vec<Value>> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_user_get(_pool: &(), _id: i64) -> ApiResult<Option<Value>> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_user_create(_pool: &(), _email: &str, _hash: &str, _nom: &str, _role: &str) -> ApiResult<Value> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_user_update(_pool: &(), _id: i64, _champs: &[(&str, String)]) -> ApiResult<Value> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_user_set_deleted(_pool: &(), _id: i64, _archive: bool) -> ApiResult<()> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_user_permanent_delete(_pool: &(), _id: i64) -> ApiResult<()> {
    hors_ligne()
}

#[cfg(not(feature = "supabase-sync"))]
pub async fn cloud_user_statut(_pool: &(), _id: i64, _email: &str) -> ApiResult<Value> {
    hors_ligne()
}
