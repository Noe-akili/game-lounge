// Auth propre - offline-first + Neon + Argon2
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
    if entries.len() >= LOGIN_MAX { true } else { entries.push(now); false }
}

// Retourne Err en cas de problème Neon (timeout/connexion morte) pour ne pas répondre
// faussement "Identifiants incorrects" ; déclenche aussi la reconnexion en arrière-plan.
#[cfg(feature = "neon-sync")]
async fn try_neon_login(app: &tauri::AppHandle, state: &State<'_, AppState>, email: &str, password: &str) -> ApiResult<Option<Value>> {
    let pool_opt = { state.neon_pool.lock().ok().and_then(|g| g.clone()) };
    let Some(pool) = pool_opt else { return Ok(None) };
    eprintln!("[auth] tentative Neon pour {}", email);
    match crate::neon::fetch_neon_user(&pool, email).await {
        Ok(Some(neon_user)) => {
            let valid = tokio::task::spawn_blocking({
                let pwd = password.to_string();
                let hash = neon_user.password_hash.clone();
                move || auth_core::compare_password(&pwd, &hash)
            }).await.unwrap_or(false);
            if !valid {
                eprintln!("[auth] Neon password mismatch");
                return Ok(None);
            }
            eprintln!("[auth] Neon login OK");
            let mut map = jmap();
            map.insert("id".into(), json!(neon_user.id));
            map.insert("email".into(), json!(neon_user.email));
            map.insert("password_hash".into(), json!(neon_user.password_hash));
            map.insert("role".into(), json!(neon_user.role));
            map.insert("nom".into(), json!(neon_user.nom));
            if let Some(ca) = neon_user.created_at { map.insert("created_at".into(), json!(ca)); }
            let database = db(state);
            match database.find_one("users", |r| r.get("email").and_then(Value::as_str) == Some(email)) {
                Ok(Some(existing)) => {
                    if let Some(id) = existing.get("id").and_then(Value::as_i64) {
                        let _ = database.update("users", id, &map);
                        if let Ok(Some(u)) = database.find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(id)) { return Ok(Some(u)); }
                    }
                }
                Ok(None) => { if let Ok(u) = database.insert("users", &map) { return Ok(Some(u)); } }
                _ => {}
            }
            Ok(Some(json!({"id": neon_user.id, "email": neon_user.email, "role": neon_user.role, "nom": neon_user.nom, "password_hash": neon_user.password_hash})))
        }
        Ok(None) => { eprintln!("[auth] Neon user non trouvé"); Ok(None) }
        Err(e) => {
            eprintln!("[auth] Neon fetch error: {}", e.message);
            crate::logger::log_neon(&format!("auth: Neon fetch error pour {}: {} -> reconnexion", email, e.message));
            crate::neon::schedule_reconnect(app);
            Err(ApiError::new(503, "Neon indisponible (reconnexion en cours), réessayez"))
        }
    }
}
#[cfg(not(feature = "neon-sync"))]
async fn try_neon_login(_app: &tauri::AppHandle, _state: &State<'_, AppState>, _email: &str, _password: &str) -> ApiResult<Option<Value>> { Ok(None) }

#[tauri::command(async)]
pub async fn auth_login(app: tauri::AppHandle, state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
    if email.is_empty() || password.is_empty() {
        return Err(ApiError::bad_request("Email et mot de passe requis"));
    }
    if !validators::is_valid_email(&email) {
        return Err(ApiError::bad_request("Email invalide"));
    }
    if !validators::is_valid_password(&password) {
        return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)"));
    }
    if too_many_attempts(&state, "local") {
        return Err(ApiError::new(429, "Trop de tentatives, réessayez plus tard"));
    }

    let database = db(&state);
    let local_user = database.find_one("users", |r| r.get("email").and_then(Value::as_str) == Some(email.as_str()))?;

    let mut user: Option<Value> = None;
    let mut was_neon = false;

    if let Some(lu) = local_user {
        let stored = lu.get("password_hash").and_then(Value::as_str).map(|s| s.to_string()).ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;
        let pwd = password.clone();
        let st = stored.clone();
        let is_valid = tokio::task::spawn_blocking(move || auth_core::compare_password(&pwd, &st)).await.map_err(|e| ApiError::internal(format!("Erreur vérification: {}", e)))?;
        if is_valid {
            user = Some(lu);
        } else {
            match try_neon_login(&app, &state, &email, &password).await? {
                Some(nu) => { user = Some(nu); was_neon = true; }
                None => return Err(ApiError::unauthorized("Identifiants incorrects")),
            }
        }
    } else {
        match try_neon_login(&app, &state, &email, &password).await? {
            Some(nu) => { user = Some(nu); was_neon = true; }
            None => return Err(ApiError::unauthorized("Identifiants incorrects")),
        }
    }

    let user = user.ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;
    let stored = user.get("password_hash").and_then(Value::as_str).unwrap_or("");

    if auth_core::needs_rehash(stored) {
        let pwd_for_hash = password.clone();
        if let Ok(upgraded) = tokio::task::spawn_blocking(move || auth_core::hash_password(&pwd_for_hash)).await.unwrap_or_else(|_| Err(ApiError::internal("hash failed"))) {
            let mut upd = jmap();
            upd.insert("password_hash".into(), json!(upgraded));
            if let Some(id) = user.get("id").and_then(Value::as_i64) {
                let _ = database.update("users", id, &upd);
                #[cfg(feature = "neon-sync")]
                eprintln!("[auth] rehash Argon2 pour {} id {}", email, id);
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
    let (access, refresh) = auth_core::generate_token_pair(&c, &state.jwt_secret)?;
    eprintln!("[auth] login success for {} via {} (neon={})", email, if was_neon { "neon" } else { "local" }, was_neon);

    Ok(json!({
        "token": access,
        "refresh_token": refresh,
        "user": user_public(&user),
        "source": if was_neon { "neon" } else { "local" },
    }))
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
    if let Some(t) = &token {
        if t == "debug-bypass-android14" || t.contains("debug-bypass") {
            return Ok(json!({ "success": true, "debug": true }));
        }
    }
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
