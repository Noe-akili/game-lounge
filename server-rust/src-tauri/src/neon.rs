// NeonDB sync - offline-first via tokio-postgres + rustls 0.19 (pure Rust)
use serde_json::{Value, json};
use crate::error::{ApiError, ApiResult};

#[cfg(feature = "neon-sync")]
use tokio_postgres::{Client, NoTls};

#[cfg(feature = "neon-sync")]
#[derive(Clone)]
pub struct NeonPool {
    pub client: std::sync::Arc<Client>,
}

// Timeouts courts OBLIGATOIRES : une connexion Neon morte (fermée par le serveur après idle,
// ou changement réseau mobile) fait HANGUER une query pendant des minutes (retransmission TCP).
// C'était la cause du "Timeout IPC (Android WebView)" : auth_login/test_neon ne répondaient jamais.
#[cfg(feature = "neon-sync")]
const NEON_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
#[cfg(feature = "neon-sync")]
const NEON_QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);

#[cfg(feature = "neon-sync")]
pub async fn neon_query(pool: &NeonPool, sql: &str, params: &[&(dyn tokio_postgres::types::ToSql + Sync)]) -> Result<Vec<tokio_postgres::Row>, String> {
    match tokio::time::timeout(NEON_QUERY_TIMEOUT, pool.client.query(sql, params)).await {
        Ok(Ok(rows)) => Ok(rows),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("timeout après {}s (connexion morte ?)", NEON_QUERY_TIMEOUT.as_secs())),
    }
}

#[cfg(feature = "neon-sync")]
async fn neon_execute(pool: &NeonPool, sql: &str, params: &[&(dyn tokio_postgres::types::ToSql + Sync)]) -> Result<u64, String> {
    match tokio::time::timeout(NEON_QUERY_TIMEOUT, pool.client.execute(sql, params)).await {
        Ok(Ok(n)) => Ok(n),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("timeout après {}s (connexion morte ?)", NEON_QUERY_TIMEOUT.as_secs())),
    }
}

/// Ping rapide de la connexion Neon (SELECT 1 avec timeout court)
#[cfg(feature = "neon-sync")]
pub async fn ping(pool: &NeonPool) -> Result<(), String> {
    match tokio::time::timeout(NEON_QUERY_TIMEOUT, pool.client.query_one("SELECT 1", &[])).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("timeout après {}s (connexion morte)", NEON_QUERY_TIMEOUT.as_secs())),
    }
}

/// Reconnexion Neon en arrière-plan après connexion morte.
/// Neon ferme les connexions inactives ; sans ça, le pool reste mort jusqu'au redémarrage de l'app.
#[cfg(feature = "neon-sync")]
pub fn schedule_reconnect(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    let Some(state) = app.try_state::<crate::AppState>() else { return };
    // Un seul reconnect à la fois (les commandes en erreur spament toutes reconnect sinon)
    if state.neon_reconnecting.swap(true, Ordering::SeqCst) { return; }
    // Pool indisponible pendant la reconnexion (les appels passeront en offline au lieu de hanguer)
    if let Ok(mut guard) = state.neon_pool.lock() {
        *guard = None;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::logger::log_neon("reconnexion Neon en arrière-plan...");
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match init_neon_pool().await {
            Some(pool) => {
                crate::logger::log_neon("reconnexion Neon OK, pool restauré");
                if let Some(s) = handle.try_state::<crate::AppState>() {
                    if let Ok(mut guard) = s.neon_pool.lock() {
                        *guard = Some(pool);
                    }
                }
            }
            None => crate::logger::log_neon("reconnexion Neon échouée (offline), réessai au prochain usage"),
        }
        if let Some(s) = handle.try_state::<crate::AppState>() {
            s.neon_reconnecting.store(false, Ordering::SeqCst);
        }
    });
}

#[cfg(feature = "neon-sync")]
pub async fn init_neon_pool() -> Option<NeonPool> {
    const FALLBACK_URL: &str = "postgresql://neondb_owner:npg_AEay0ug9NHYj@ep-wild-cloud-axxw1ufj-pooler.c-4.us-east-2.aws.neon.tech/neondb?sslmode=require";
    let url = std::env::var("DATABASE_URL").ok().filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("NEON_DATABASE_URL").ok().filter(|s| !s.trim().is_empty()))
        .or_else(|| option_env!("DATABASE_URL").map(|s| s.to_string()).filter(|s| !s.trim().is_empty()))
        .or_else(|| option_env!("NEON_DATABASE_URL").map(|s| s.to_string()).filter(|s| !s.trim().is_empty()))
        .or_else(|| Some(FALLBACK_URL.to_string()));
    let url = match url {
        Some(u) if !u.trim().is_empty() => u,
        _ => {
            crate::logger::log_neon("DATABASE_URL absent et fallback vide, mode offline");
            return None;
        }
    };
    crate::logger::log_neon(&format!("DATABASE_URL présent ({} chars), tentative rustls", url.len()));
    // Tente rustls 0.19 - webpki-roots 0.21 fournit TLS_SERVER_ROOTS directement compatible
    let rustls_result = tokio::time::timeout(NEON_CONNECT_TIMEOUT, async {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
        let mut config = rustls::ClientConfig::new();
        config.root_store = root_store;
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(config);
        tokio_postgres::connect(&url, tls).await
    }).await;

    match rustls_result {
        Ok(Ok((client, connection))) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[neon] connection error (rustls): {}", e);
                }
            });
            match tokio::time::timeout(NEON_QUERY_TIMEOUT, client.query("SELECT 1", &[])).await {
                Ok(Ok(_)) => {
                    eprintln!("[neon] pool connecté (rustls)");
                    return Some(NeonPool { client: std::sync::Arc::new(client) });
                }
                Ok(Err(e)) => {
                    eprintln!("[neon] rustls test query failed: {}, tente NoTls", e);
                }
                Err(_) => {
                    eprintln!("[neon] rustls test query timeout, tente NoTls");
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("[neon] rustls connect failed: {}, tente NoTls", e);
        }
        Err(_) => {
            eprintln!("[neon] rustls connect timeout ({}s), tente NoTls", NEON_CONNECT_TIMEOUT.as_secs());
        }
    }

    // Fallback NoTls
    let no_tls_result = tokio::time::timeout(NEON_CONNECT_TIMEOUT, tokio_postgres::connect(&url, NoTls)).await;
    match no_tls_result {
        Ok(Ok((client, connection))) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[neon] connection error (NoTls): {}", e);
                }
            });
            match tokio::time::timeout(NEON_QUERY_TIMEOUT, client.query("SELECT 1", &[])).await {
                Ok(Ok(_)) => {
                    eprintln!("[neon] pool connecté (NoTls)");
                    Some(NeonPool { client: std::sync::Arc::new(client) })
                }
                Ok(Err(e)) => {
                    eprintln!("[neon] NoTls test query failed: {}, offline", e);
                    None
                }
                Err(_) => {
                    eprintln!("[neon] NoTls test query timeout, offline");
                    None
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("[neon] pool connect failed (offline): {}", e);
            None
        }
        Err(_) => {
            eprintln!("[neon] pool connect timeout ({}s), offline", NEON_CONNECT_TIMEOUT.as_secs());
            None
        }
    }
}

#[cfg(not(feature = "neon-sync"))]
pub async fn init_neon_pool() -> Option<()> {
    eprintln!("[neon] feature neon-sync désactivée");
    None
}

#[derive(Debug, Clone)]
pub struct NeonUser {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub nom: String,
    pub created_at: Option<String>,
}

/// Lecture d'une colonne texte tolérante aux pannes : N'UTILISE PAS row.get (panique si type
/// Postgres inattendu, ex: TIMESTAMPTZ décodé en String) — un panic ici tue l'app Android.
#[cfg(feature = "neon-sync")]
pub fn pg_col_to_string_pub(row: &tokio_postgres::Row, idx: usize) -> Option<String> {
    match row.try_get::<_, Option<String>>(idx) {
        Ok(Some(s)) => Some(s),
        Ok(None) => None,
        Err(_) => None, // type inattendu (timestamp, etc.) -> on ignore la colonne, pas de panic
    }
}

#[cfg(feature = "neon-sync")]
fn pg_col_to_string(row: &tokio_postgres::Row, idx: usize) -> Option<String> {
    pg_col_to_string_pub(row, idx)
}

#[cfg(feature = "neon-sync")]
pub async fn fetch_neon_user(pool: &NeonPool, email: &str) -> ApiResult<Option<NeonUser>> {
    let rows = neon_query(pool, "SELECT id, email, password_hash, role, nom, created_at FROM users WHERE email = $1 LIMIT 1", &[&email])
        .await
        .map_err(|e| ApiError::internal(format!("Neon query user: {}", e)))?;
    if rows.is_empty() {
        return Ok(None);
    }
    let row = &rows[0];
    // Décodage défensif : tout panic potentiel est contenu (catch_unwind), jamais propagé
    let decode = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        NeonUser {
            id: row.try_get::<_, i64>(0).unwrap_or(0),
            email: pg_col_to_string(row, 1).unwrap_or_default(),
            password_hash: pg_col_to_string(row, 2).unwrap_or_default(),
            role: pg_col_to_string(row, 3).unwrap_or_else(|| "employe".to_string()),
            nom: pg_col_to_string(row, 4).unwrap_or_default(),
            created_at: pg_col_to_string(row, 5),
        }
    }));
    match decode {
        Ok(u) => Ok(Some(u)),
        Err(_) => {
            crate::logger::log_neon("fetch_neon_user: décodage row paniqué, user ignoré");
            Ok(None)
        }
    }
}

#[cfg(not(feature = "neon-sync"))]
pub async fn fetch_neon_user(_pool: &(), _email: &str) -> ApiResult<Option<NeonUser>> {
    Ok(None)
}

#[cfg(feature = "neon-sync")]
pub async fn pull_users(pool: &NeonPool) -> ApiResult<Vec<Value>> {
    let rows = neon_query(pool, "SELECT id, email, password_hash, role, nom, created_at FROM users ORDER BY id", &[])
        .await
        .map_err(|e| ApiError::internal(format!("Neon pull users: {}", e)))?;
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
            })
        }));
        match decoded {
            Ok(v) => out.push(v),
            Err(_) => {
                crate::logger::log_neon("pull_users: ligne paniquée au décodage, ignorée");
            }
        }
    }
    Ok(out)
}

#[cfg(not(feature = "neon-sync"))]
pub async fn pull_users(_pool: &()) -> ApiResult<Vec<Value>> {
    Ok(vec![])
}

// Pull toutes les tables pour restauration complète si app data vidé.
// IMPORTANT : on utilise json_agg (fonction STANDARD Postgres) au lieu de
// row_to_json (fonction custom qui n'existe pas forcément sur Neon -> la requête
// échouait -> sync_run abort -> AUCUNE donnée écrite localement).
// Une table absente du schéma Neon est ignorée (log + continue) ; seule une erreur
// de connexion est propagée (déclenche la reconnexion en arrière-plan).
#[cfg(feature = "neon-sync")]
pub async fn pull_all(pool: &NeonPool) -> ApiResult<std::collections::HashMap<String, Vec<Value>>> {
    let mut all = std::collections::HashMap::new();
    let tables = ["users", "consoles", "jeux", "joueurs", "sessions_jeu", "tarifs", "factures", "jetons_transactions", "messages", "parametres_fidelite", "lignes_facture"];
    for table in tables {
        let sql = format!("SELECT COALESCE(json_agg(t)::text, '[]') FROM (SELECT * FROM \"{}\") t", table);
        let json_rows = neon_query(pool, &sql, &[]).await;
        match json_rows {
            Ok(jrows) => {
                let mut vec = Vec::new();
                if !jrows.is_empty() {
                    if let Ok(s) = jrows[0].try_get::<_, String>(0) {
                        if let Ok(v) = serde_json::from_str::<Value>(&s) {
                            if let Some(arr) = v.as_array() {
                                for item in arr {
                                    vec.push(item.clone());
                                }
                            }
                        }
                    }
                }
                eprintln!("[neon] pull {}: {} rows", table, vec.len());
                all.insert(table.to_string(), vec);
            }
            Err(e) => {
                // Table absente / colonne manquante -> on ignore la table (offline-first).
                // Erreur de connexion -> on propage pour déclencher la reconnexion.
                if e.to_lowercase().contains("does not exist") || e.to_lowercase().contains("undefined") {
                    eprintln!("[neon] pull {} ignorée (schéma Neon): {}", table, e);
                    crate::logger::log_neon(&format!("pull {} ignorée (schéma): {}", table, e));
                    all.insert(table.to_string(), Vec::new());
                } else {
                    return Err(ApiError::internal(format!("Neon pull {}: {}", table, e)));
                }
            }
        }
    }
    Ok(all)
}

#[cfg(not(feature = "neon-sync"))]
pub async fn pull_all(_pool: &()) -> ApiResult<std::collections::HashMap<String, Vec<Value>>> {
    Ok(std::collections::HashMap::new())
}

#[cfg(feature = "neon-sync")]
pub async fn push_user(pool: &NeonPool, user: &Value) -> ApiResult<()> {
    let id: i64 = user.get("id").and_then(Value::as_i64).unwrap_or(0);
    let email: &str = user.get("email").and_then(Value::as_str).unwrap_or("");
    let password_hash: &str = user.get("password_hash").and_then(Value::as_str).unwrap_or("");
    let role: &str = user.get("role").and_then(Value::as_str).unwrap_or("employe");
    let nom: &str = user.get("nom").and_then(Value::as_str).unwrap_or("");
    neon_execute(pool,
            "INSERT INTO users (id, email, password_hash, role, nom, created_at) VALUES ($1,$2,$3,$4,$5,NOW()) ON CONFLICT (id) DO UPDATE SET email=$2, password_hash=$3, role=$4, nom=$5",
            &[&id, &email, &password_hash, &role, &nom],
        )
        .await
        .map_err(|e| ApiError::internal(format!("Neon push user: {}", e)))?;
    Ok(())
}

#[cfg(feature = "neon-sync")]
pub async fn push_tarif(pool: &NeonPool, tarif: &Value) -> ApiResult<()> {
    let id: i64 = tarif.get("id").and_then(Value::as_i64).unwrap_or(0);
    let type_: &str = tarif.get("type").and_then(Value::as_str).unwrap_or("session");
    let duree: i64 = tarif.get("duree_minutes").and_then(Value::as_i64).unwrap_or(30);
    let prix: i64 = tarif.get("prix").and_then(Value::as_i64).unwrap_or(0);
    let desc: &str = tarif.get("description").and_then(Value::as_str).unwrap_or("");
    let console_type: &str = tarif.get("console_type").and_then(Value::as_str).unwrap_or("PS5");
    let jeu: &str = tarif.get("jeu").and_then(Value::as_str).unwrap_or("");
    let actif: i64 = tarif.get("actif").and_then(Value::as_i64).unwrap_or(1);
    neon_execute(pool,
        "INSERT INTO tarifs (id, type, duree_minutes, prix, description, console_type, jeu, actif, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW()) ON CONFLICT (id) DO UPDATE SET type=$2, duree_minutes=$3, prix=$4, description=$5, console_type=$6, jeu=$7, actif=$8",
        &[&id, &type_, &duree, &prix, &desc, &console_type, &jeu, &actif]
    ).await.map_err(|e| ApiError::internal(format!("Neon push tarif: {}", e)))?;
    Ok(())
}

/// Colonnes d'une table Neon via information_schema (standard Postgres).
/// Utilisé pour ne jamais envoyer vers Neon une colonne qui n'existe pas côté cloud
/// (sinon erreur SQL silencieuse -> "l'envoi ne fonctionne pas").
#[cfg(feature = "neon-sync")]
pub async fn neon_table_columns(pool: &NeonPool, table: &str) -> Result<Vec<String>, String> {
    let rows = neon_query(
        pool,
        "SELECT column_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = $1 ORDER BY ordinal_position",
        &[&table],
    ).await;
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

#[cfg(feature = "neon-sync")]
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

/// Push générique d'une table locale vers Neon : upsert par id, colonnes filtrées
/// sur le schéma Neon (information_schema). C'est ce qui rend l'ENVOI possible :
/// avant, sync_run ne faisait que pull, push_user/push_tarif n'étaient jamais appelés.
#[cfg(feature = "neon-sync")]
pub async fn push_table(pool: &NeonPool, table: &str, rows: &Vec<Value>) -> ApiResult<usize> {
    let cols = neon_table_columns(pool, table)
        .await
        .map_err(|e| ApiError::internal(format!("Neon colonnes {}: {}", table, e)))?;
    let mut pushed = 0;
    for row in rows {
        let Some(obj) = row.as_object() else { continue };
        let Some(id_v) = obj.get("id") else { continue };
        let id = id_v.as_i64().unwrap_or(0);
        if id == 0 { continue; }
        // Colonnes présentes dans la ligne ET dans Neon, valeurs non nulles
        let mut keys: Vec<String> = Vec::new();
        for k in obj.keys() {
            if *k == "id" || !cols.contains(k) { continue; }
            if obj[k].is_null() { continue; }
            keys.push(k.to_string());
        }
        if keys.is_empty() { continue; }
        let placeholders: Vec<&str> = keys.iter().map(|k| "?").collect();
        let updates: Vec<String> = keys.iter().map(|k| format!("{}=EXCLUDED.{}", k, k)).collect();
        let sql = format!(
            "INSERT INTO \"{table}\" (id,{}) VALUES (?,{}) ON CONFLICT (id) DO UPDATE SET {}",
            keys.join(","),
            placeholders.join(","),
            updates.join(",")
        );
        // Tous les params en texte : Postgres caste text -> type de la colonne à l'insert
        let mut text_params: Vec<String> = Vec::with_capacity(keys.len() + 1);
        text_params.push(format!("{}", id));
        for k in keys {
            text_params.push(val_to_text(&obj[&k]));
        }
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::with_capacity(text_params.len());
        for i in 0..text_params.len() {
            params.push(&text_params[i]);
        }
        match neon_execute(pool, &sql, &params).await {
            Ok(_) => pushed += 1,
            Err(e) => {
                eprintln!("[neon] push {} #{} failed: {}", table, id, e);
                crate::logger::log_neon(&format!("push {} #{}: {}", table, id, e));
            }
        }
    }
    Ok(pushed)
}
