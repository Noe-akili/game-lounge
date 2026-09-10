// POST /api/auth/login, logout, me
// Port de server/server.ts (auth).

use serde_json::{Value, json};
use tauri::State;

use crate::auth as auth_core;
use crate::commands::{claims, db, jmap, user_public};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

const LOGIN_WINDOW_MS: i64 = 15 * 60 * 1000;
const LOGIN_MAX: usize = 20;

fn too_many_attempts(state: &State<'_, AppState>, key: &str) -> bool {
    let now = chrono::Utc::now().timestamp_millis();
    let mut map = match state.login_attempts.lock() {
        Ok(m) => m,
        Err(_) => return true,
    };
    let entries = map.entry(key.to_string()).or_default();
    entries.retain(|t| now - t < LOGIN_WINDOW_MS);
    if entries.len() >= LOGIN_MAX {
        true
    } else {
        entries.push(now);
        false
    }
}

/// POST /api/auth/login
#[tauri::command]
pub fn auth_login(state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
    if email.is_empty() || password.is_empty() {
        return Err(ApiError::bad_request("Email et mot de passe requis"));
    }
    if !validators::is_valid_email(&email) {
        return Err(ApiError::bad_request("Email invalide"));
    }
    if !validators::is_valid_password(&password) {
        return Err(ApiError::bad_request(
            "Mot de passe invalide (min 6 caractères, au moins une lettre)",
        ));
    }
    if too_many_attempts(&state, "local") {
        return Err(ApiError::new(429, "Trop de tentatives, réessayez plus tard"));
    }

    let db = db(&state);
    let user = db.find_one("users", |r| {
        r.get("email").and_then(Value::as_str) == Some(email.as_str())
    })?;
    let user = match user {
        Some(u) => u,
        None => return Err(ApiError::unauthorized("Identifiants incorrects")),
    };

    let stored = user
        .get("password_hash")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;

    if !auth_core::compare_password(&password, &stored) {
        return Err(ApiError::unauthorized("Identifiants incorrects"));
    }

    // Mise à niveau des anciens hashs bcrypt vers scrypt au premier login réussi.
    if !auth_core::is_scrypt_hash(&stored) {
        if let Ok(upgraded) = auth_core::hash_password(&password) {
            let mut upd = jmap();
            upd.insert("password_hash".into(), json!(upgraded));
            if let Some(id) = user.get("id").and_then(Value::as_i64) {
                let _ = db.update("users", id, &upd);
            }
        }
    }

    let c = auth_core::Claims {
        id: user.get("id").and_then(Value::as_i64).unwrap_or(0),
        email: user.get("email").and_then(Value::as_str).unwrap_or("").to_string(),
        role: user.get("role").and_then(Value::as_str).unwrap_or("employe").to_string(),
        nom: user.get("nom").and_then(Value::as_str).unwrap_or("").to_string(),
        iat: 0,
        exp: 0,
    };
    let token = auth_core::sign_token(&c, &state.jwt_secret)?;

    Ok(json!({
        "token": token,
        "user": user_public(&user),
    }))
}

/// POST /api/auth/logout
#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    Ok(json!({ "success": true }))
}

/// GET /api/auth/me
#[tauri::command]
pub fn auth_me(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let c = claims(&state, &token)?;
    let db = db(&state);
    let user = db.find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(c.id))?;
    match user {
        Some(u) => Ok(json!({ "user": user_public(&u) })),
        None => Err(ApiError::not_found("Utilisateur non trouvé")),
    }
}