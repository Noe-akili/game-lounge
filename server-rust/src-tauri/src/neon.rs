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

#[cfg(feature = "neon-sync")]
pub async fn init_neon_pool() -> Option<NeonPool> {
    const FALLBACK_URL: &str = "postgresql://neondb_owner:npg_AEay0ug9NHYj@ep-wild-cloud-axxw1ufj-pooler.c-4.us-east-2.aws.neon.tech/neondb?sslmode=require";
    let url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("NEON_DATABASE_URL"))
        .ok()
        .or_else(|| option_env!("DATABASE_URL").map(|s| s.to_string()))
        .or_else(|| option_env!("NEON_DATABASE_URL").map(|s| s.to_string()))
        .or_else(|| Some(FALLBACK_URL.to_string()));
    let url = match url {
        Some(u) if !u.is_empty() => u,
        _ => {
            eprintln!("[neon] DATABASE_URL absent, mode offline");
            return None;
        }
    };
    eprintln!("[neon] DATABASE_URL présent ({} chars), tentative rustls", url.len());
    // Tente rustls 0.19 - webpki-roots 0.21 fournit TLS_SERVER_ROOTS directement compatible
    let rustls_result = async {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
        let mut config = rustls::ClientConfig::new();
        config.root_store = root_store;
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(config);
        tokio_postgres::connect(&url, tls).await
    }.await;

    match rustls_result {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[neon] connection error (rustls): {}", e);
                }
            });
            match client.query("SELECT 1", &[]).await {
                Ok(_) => {
                    eprintln!("[neon] pool connecté (rustls)");
                    return Some(NeonPool { client: std::sync::Arc::new(client) });
                }
                Err(e) => {
                    eprintln!("[neon] rustls test query failed: {}, tente NoTls", e);
                }
            }
        }
        Err(e) => {
            eprintln!("[neon] rustls connect failed: {}, tente NoTls", e);
        }
    }

    // Fallback NoTls
    match tokio_postgres::connect(&url, NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("[neon] connection error (NoTls): {}", e);
                }
            });
            match client.query("SELECT 1", &[]).await {
                Ok(_) => {
                    eprintln!("[neon] pool connecté (NoTls)");
                    Some(NeonPool { client: std::sync::Arc::new(client) })
                }
                Err(e) => {
                    eprintln!("[neon] NoTls test query failed: {}, offline", e);
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

// Pull toutes les tables pour restauration complète si app data vidé
#[cfg(feature = "neon-sync")]
pub async fn pull_all(pool: &NeonPool) -> ApiResult<std::collections::HashMap<String, Vec<Value>>> {
    let mut all = std::collections::HashMap::new();
    let tables = ["users", "consoles", "jeux", "joueurs", "sessions_jeu", "tarifs", "factures", "jetons_transactions", "messages", "parametres_fidelite", "lignes_facture"];
    for table in tables {
        let json_rows = pool.client.query(&format!("SELECT row_to_json(t) as data FROM (SELECT * FROM {} ) t", table), &[]).await;
        match json_rows {
            Ok(jrows) => {
                let mut vec = Vec::new();
                for r in jrows {
                    if let Ok(s) = r.try_get::<_, String>(0) {
                        if let Ok(v) = serde_json::from_str::<Value>(&s) {
                            vec.push(v);
                        }
                    }
                }
                eprintln!("[neon] pull {}: {} rows", table, vec.len());
                all.insert(table.to_string(), vec);
            }
            Err(e) => {
                eprintln!("[neon] pull {} failed: {}", table, e);
                all.insert(table.to_string(), Vec::new());
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
    pool.client
        .execute(
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
    pool.client.execute(
        "INSERT INTO tarifs (id, type, duree_minutes, prix, description, console_type, jeu, actif, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW()) ON CONFLICT (id) DO UPDATE SET type=$2, duree_minutes=$3, prix=$4, description=$5, console_type=$6, jeu=$7, actif=$8",
        &[&id, &type_, &duree, &prix, &desc, &console_type, &jeu, &actif]
    ).await.map_err(|e| ApiError::internal(format!("Neon push tarif: {}", e)))?;
    Ok(())
}
