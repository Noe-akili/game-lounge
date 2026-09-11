// NeonDB sync - best practice offline-first
// Utilise sqlx PgPool pour parler directement à Postgres (Neon)
// Si DATABASE_URL absent ou offline, sync désactivé (local-only)

#[cfg(feature = "neon-sync")]
use sqlx::PgPool;
use serde_json::{Value, json};
use crate::error::{ApiError, ApiResult};

#[cfg(feature = "neon-sync")]
pub async fn init_neon_pool() -> Option<PgPool> {
    // Best practice : récupère DATABASE_URL depuis runtime env, ou compile-time option_env! (pour APK)
    let url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("NEON_DATABASE_URL"))
        .ok()
        .or_else(|| option_env!("DATABASE_URL").map(|s| s.to_string()))
        .or_else(|| option_env!("NEON_DATABASE_URL").map(|s| s.to_string()))
        .or_else(|| option_env!("DATABASE_URL_AT_BUILD").map(|s| s.to_string()));
    let url = match url {
        Some(u) if !u.is_empty() => u,
        _ => {
            eprintln!("[neon] DATABASE_URL absent, mode offline (compile-time et runtime vides)");
            return None;
        }
    };
    // Cache : évite de retenter chaque fois si échec réseau
    match PgPool::connect(&url).await {
        Ok(pool) => {
            eprintln!("[neon] pool connecté");
            // Test simple
            if let Err(e) = sqlx::query("SELECT 1").execute(&pool).await {
                eprintln!("[neon] test query failed: {e}, offline");
                return None;
            }
            Some(pool)
        }
        Err(e) => {
            eprintln!("[neon] pool connect failed (offline): {e}");
            None
        }
    }
}

#[cfg(not(feature = "neon-sync"))]
pub async fn init_neon_pool() -> Option<()> {
    eprintln!("[neon] feature neon-sync désactivée");
    None
}

// Structure utilisateur Neon (même que local)
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
pub async fn fetch_neon_user(pool: &PgPool, email: &str) -> ApiResult<Option<NeonUser>> {
    let row = sqlx::query_as::<_, (i64, String, String, String, String, Option<chrono::NaiveDateTime>)>(
        "SELECT id, email, password_hash, role, nom, created_at FROM users WHERE email = $1 LIMIT 1"
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(format!("Neon query user: {e}")))?;

    Ok(row.map(|(id, email, password_hash, role, nom, created_at)| NeonUser {
        id,
        email,
        password_hash,
        role,
        nom,
        created_at: created_at.map(|dt| dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
    }))
}

#[cfg(not(feature = "neon-sync"))]
pub async fn fetch_neon_user(_pool: &(), _email: &str) -> ApiResult<Option<NeonUser>> {
    Ok(None)
}

// Pull all users from Neon pour cache local (background sync)
#[cfg(feature = "neon-sync")]
pub async fn pull_users(pool: &PgPool) -> ApiResult<Vec<Value>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, Option<chrono::NaiveDateTime>)>(
        "SELECT id, email, password_hash, role, nom, created_at FROM users ORDER BY id"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(format!("Neon pull users: {e}")))?;

    Ok(rows.into_iter().map(|(id,email,password_hash,role,nom,created_at)| {
        json!({
            "id": id,
            "email": email,
            "password_hash": password_hash,
            "role": role,
            "nom": nom,
            "created_at": created_at.map(|dt| dt.and_utc().to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        })
    }).collect())
}

#[cfg(not(feature = "neon-sync"))]
pub async fn pull_users(_pool: &()) -> ApiResult<Vec<Value>> {
    Ok(vec![])
}

// Push local user to Neon (upsert)
#[cfg(feature = "neon-sync")]
pub async fn push_user(pool: &PgPool, user: &Value) -> ApiResult<()> {
    let id = user.get("id").and_then(Value::as_i64).unwrap_or(0);
    let email = user.get("email").and_then(Value::as_str).unwrap_or("");
    let password_hash = user.get("password_hash").and_then(Value::as_str).unwrap_or("");
    let role = user.get("role").and_then(Value::as_str).unwrap_or("employe");
    let nom = user.get("nom").and_then(Value::as_str).unwrap_or("");
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, nom, created_at) VALUES ($1,$2,$3,$4,$5,NOW())
         ON CONFLICT (id) DO UPDATE SET email=$2, password_hash=$3, role=$4, nom=$5"
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .bind(role)
    .bind(nom)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(format!("Neon push user: {e}")))?;
    Ok(())
}
