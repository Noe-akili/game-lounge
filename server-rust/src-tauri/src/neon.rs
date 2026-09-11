// NeonDB sync - best practice offline-first via tokio-postgres
// Si DATABASE_URL absent ou offline, sync désactivé (local-only)
// Utilise tokio-postgres (évite conflit rusqlite sqlite)

#[cfg(feature = "neon-sync")]
use tokio_postgres::{NoTls, Client};
use serde_json::{Value, json};
use crate::error::{ApiError, ApiResult};

#[cfg(feature = "neon-sync")]
#[derive(Clone)]
pub struct NeonPool {
    pub client: std::sync::Arc<Client>,
}

#[cfg(feature = "neon-sync")]
pub async fn init_neon_pool() -> Option<NeonPool> {
    let url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("NEON_DATABASE_URL"))
        .ok()
        .or_else(|| option_env!("DATABASE_URL").map(|s| s.to_string()))
        .or_else(|| option_env!("NEON_DATABASE_URL").map(|s| s.to_string()));
    let url = match url {
        Some(u) if !u.is_empty() => u,
        _ => {
            eprintln!("[neon] DATABASE_URL absent, mode offline");
            return None;
        }
    };
    // Tente connexion avec NoTls (Neon supporte aussi TLS mais NoTls suffit pour test, sinon fallback offline)
    // Pour TLS natif, on pourrait utiliser postgres-native-tls, mais on garde NoTls pour simplicité Android
    // Si échec, on reste offline
    match tokio_postgres::connect(&url, NoTls).await {
        Ok((client, connection)) => {
            // Spawn connection handling
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[neon] connection error: {}", e);
                }
            });
            // Test query
            match client.query("SELECT 1", &[]).await {
                Ok(_) => {
                    eprintln!("[neon] pool connecté (tokio-postgres NoTls)");
                    Some(NeonPool { client: std::sync::Arc::new(client) })
                }
                Err(e) => {
                    eprintln!("[neon] test query failed (offline/TLS requis): {}, offline", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("[neon] pool connect failed (offline): {}", e);
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

#[cfg(feature = "neon-sync")]
pub async fn fetch_neon_user(pool: &NeonPool, email: &str) -> ApiResult<Option<NeonUser>> {
    let rows = pool.client
        .query("SELECT id, email, password_hash, role, nom, created_at FROM users WHERE email = $1 LIMIT 1", &[&email])
        .await
        .map_err(|e| ApiError::internal(format!("Neon query user: {}", e)))?;
    if rows.is_empty() {
        return Ok(None);
    }
    let row = &rows[0];
    Ok(Some(NeonUser {
        id: row.get::<_, i64>(0),
        email: row.get::<_, String>(1),
        password_hash: row.get::<_, String>(2),
        role: row.get::<_, String>(3),
        nom: row.get::<_, String>(4),
        created_at: row.get::<_, Option<chrono::NaiveDateTime>>(5).map(|dt| dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
    }))
}

#[cfg(not(feature = "neon-sync"))]
pub async fn fetch_neon_user(_pool: &(), _email: &str) -> ApiResult<Option<NeonUser>> {
    Ok(None)
}

#[cfg(feature = "neon-sync")]
pub async fn pull_users(pool: &NeonPool) -> ApiResult<Vec<Value>> {
    let rows = pool.client
        .query("SELECT id, email, password_hash, role, nom, created_at FROM users ORDER BY id", &[])
        .await
        .map_err(|e| ApiError::internal(format!("Neon pull users: {}", e)))?;
    Ok(rows.iter().map(|row| {
        let created_at: Option<chrono::NaiveDateTime> = row.get(5);
        json!({
            "id": row.get::<_, i64>(0),
            "email": row.get::<_, String>(1),
            "password_hash": row.get::<_, String>(2),
            "role": row.get::<_, String>(3),
            "nom": row.get::<_, String>(4),
            "created_at": created_at.map(|dt| dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        })
    }).collect())
}

#[cfg(not(feature = "neon-sync"))]
pub async fn pull_users(_pool: &()) -> ApiResult<Vec<Value>> {
    Ok(vec![])
}

#[cfg(feature = "neon-sync")]
pub async fn push_user(pool: &NeonPool, user: &Value) -> ApiResult<()> {
    let id: i64 = user.get("id").and_then(Value::as_i64).unwrap_or(0);
    let email: &str = user.get("email").and_then(Value::as_str).unwrap_or("");
    let password_hash: &str = user.get("password_hash").and_then(Value::as_str).unwrap_or("");
    let role: &str = user.get("role").and_then(Value::as_str).unwrap_or("employe");
    let nom: &str = user.get("nom").and_then(Value::as_str).unwrap_or("");
    pool.client
        .execute(
            "INSERT INTO users (id, email, password_hash, role, nom, created_at) VALUES ($1,$2,$3,$4,$5,NOW()) ON CONFLICT (id) DO UPDATE SET email=$2, password_hash=$3, role=$4, nom=$5",
            &[&id, &email, &password_hash, &role, &nom],
        )
        .await
        .map_err(|e| ApiError::internal(format!("Neon push user: {}", e)))?;
    Ok(())
}
