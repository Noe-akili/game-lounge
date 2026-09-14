// Auth - l'application est mono-utilisateur admin : PAS de login.
// La session admin est bootstrappée automatiquement au démarrage (auth_bootstrap_admin),
// le frontend appelle /auth/me pour la restaurer. Les commandes de connexion
// (auth_login, rate limiting, comparaison de mots de passe) ont été supprimées.
use serde_json::{Value, json};
use tauri::State;

use crate::auth as auth_core;
use crate::commands::{admin_only, claims, db, user_public};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn hash_algo(stored: &str) -> &'static str {
    if stored.starts_with("scrypt$v1") { "scrypt" }
    else if stored.starts_with("$argon2") { "argon2" }
    else if stored.starts_with("$2") { "bcrypt" }
    else { "inconnu" }
}

/// Crée (ou restaure) la session admin implicite : l'app est mono-utilisateur,
/// l'admin est connecté d'office, sans écran de connexion.
#[tauri::command]
pub fn auth_bootstrap_admin(state: State<'_, AppState>) -> ApiResult<Value> {
    let selected = db(&state).query_all("users")?.into_iter().find(|u| u.get("role").and_then(Value::as_str) == Some("admin"));
    let user = selected.unwrap_or_else(|| json!({"id": 0, "email": "admin", "role": "admin", "nom": "Administrateur"}));
    let claims = auth_core::Claims {
        id: user.get("id").and_then(Value::as_i64).unwrap_or(0),
        email: user.get("email").and_then(Value::as_str).unwrap_or("admin").to_string(),
        role: "admin".to_string(),
        nom: user.get("nom").and_then(Value::as_str).unwrap_or("Administrateur").to_string(),
        iat: 0, exp: 0,
    };
    let (token, refresh_token) = auth_core::generate_token_pair(&claims, &state.jwt_secret)?;
    Ok(json!({"token": token, "refresh_token": refresh_token, "user": user_public(&user), "source": "kiosk-admin"}))
}

/// Diagnostic : état des users locaux (algo de hash, sans jamais exposer le hash)
#[tauri::command]
pub fn auth_debug_info(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let users = db(&state).query_all("users")?;
    let list: Vec<Value> = users.iter().map(|u| {
        let hash = u.get("password_hash").and_then(Value::as_str).unwrap_or("");
        json!({
            "id": u.get("id"),
            "email": u.get("email"),
            "role": u.get("role"),
            "nom": u.get("nom"),
            "hash_algo": hash_algo(hash),
            "hash_len": hash.len(),
        })
    }).collect();
    Ok(json!({ "users": list }))
}

/// Diagnostic : liste les users côté Supabase (email + algo de hash uniquement)
#[cfg(feature = "supabase-sync")]
#[tauri::command(async)]
pub async fn auth_debug_supabase_users(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let pool_opt = { state.supabase_pool.lock().ok().and_then(|g| g.clone()) };
    let Some(pool) = pool_opt else { return Ok(json!({ "users": [], "online": false })) };
    match crate::supabase::supabase_query(&pool, "SELECT id, email, password_hash, role, nom FROM users ORDER BY id", &[]).await {
        Ok(rows) => {
            let list: Vec<Value> = rows.iter().map(|r| {
                let hash = crate::supabase::pg_col_to_string_pub(r, 2).unwrap_or_default();
                json!({
                    "id": r.try_get::<_, i64>(0).unwrap_or(0),
                    "email": crate::supabase::pg_col_to_string_pub(r, 1).unwrap_or_default(),
                    "role": crate::supabase::pg_col_to_string_pub(r, 3).unwrap_or_default(),
                    "nom": crate::supabase::pg_col_to_string_pub(r, 4).unwrap_or_default(),
                    "hash_algo": hash_algo(&hash),
                    "hash_len": hash.len(),
                })
            }).collect();
            Ok(json!({ "users": list, "online": true }))
        }
        Err(e) => Err(ApiError::internal(format!("Supabase users query: {}", e))),
    }
}
#[cfg(not(feature = "supabase-sync"))]
#[tauri::command(async)]
pub async fn auth_debug_supabase_users(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    Ok(json!({ "users": [], "online": false }))
}

#[tauri::command]
pub fn auth_refresh(state: State<'_, AppState>, refresh_token: String) -> ApiResult<Value> {
    if refresh_token.is_empty() {
        return Err(ApiError::bad_request("Refresh token requis"));
    }
    let claims = auth_core::verify_token(&refresh_token, &state.jwt_secret)?;
    let new_access = auth_core::sign_token(&claims, &state.jwt_secret)?;
    Ok(json!({ "token": new_access }))
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub fn auth_me(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let c = claims(&state, &token)?;
    let database = db(&state);
    let user = database.find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(c.id))?;
    match user {
        Some(u) => Ok(json!({ "user": user_public(&u) })),
        None => Err(ApiError::not_found("Utilisateur non trouvé")),
    }
}
