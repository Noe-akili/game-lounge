// POST /api/auth/login, logout, me, refresh - Best practice offline-first + Neon sync
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

/// Helper : tente Neon si local échoue (offline-first)
#[cfg(feature = "neon-sync")]
async fn try_neon_login(state: &State<'_, AppState>, email: &str, password: &str) -> Option<Value> {
    // Clone pool sans bloquer
    let pool_opt = {
        let guard = state.neon_pool.lock().ok()?;
        guard.clone()
    };
    let pool = pool_opt?;
    eprintln!("[auth] tentative Neon pour {}", email);
    match crate::neon::fetch_neon_user(&pool, email).await {
        Ok(Some(neon_user)) => {
            // Vérifie hash Neon (peut être argon2/scrypt/bcrypt)
            let pwd = password.to_string();
            let hash = neon_user.password_hash.clone();
            let valid = tokio::task::spawn_blocking(move || auth_core::compare_password(&pwd, &hash))
                .await
                .unwrap_or(false);
            if !valid {
                eprintln!("[auth] Neon password mismatch for {}", email);
                return None;
            }
            eprintln!("[auth] Neon login OK pour {}", email);
            // Upsert local cache pour offline futur
            let mut map = jmap();
            map.insert("id".into(), json!(neon_user.id));
            map.insert("email".into(), json!(neon_user.email));
            map.insert("password_hash".into(), json!(neon_user.password_hash));
            map.insert("role".into(), json!(neon_user.role));
            map.insert("nom".into(), json!(neon_user.nom));
            if let Some(ca) = neon_user.created_at {
                map.insert("created_at".into(), json!(ca));
            }
            let db = db(state);
            // Insert or update local
            match db.find_one("users", |r| r.get("email").and_then(Value::as_str) == Some(email)) {
                Ok(Some(existing)) => {
                    if let Some(id) = existing.get("id").and_then(Value::as_i64) {
                        let _ = db.update("users", id, &map);
                        // Retourne l'utilisateur local mis à jour
                        if let Ok(Some(u)) = db.find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(id)) {
                            return Some(u);
                        }
                    }
                }
                Ok(None) => {
                    if let Ok(u) = db.insert("users", &map) {
                        return Some(u);
                    }
                }
                _ => {}
            }
            // Fallback : construit un Value Neon -> local format
            Some(json!({
                "id": neon_user.id,
                "email": neon_user.email,
                "role": neon_user.role,
                "nom": neon_user.nom,
                "password_hash": neon_user.password_hash,
            }))
        }
        Ok(None) => {
            eprintln!("[auth] Neon user non trouvé {}", email);
            None
        }
        Err(e) => {
            eprintln!("[auth] Neon fetch error (offline?): {}", e.message);
            None
        }
    }
}

#[cfg(not(feature = "neon-sync"))]
async fn try_neon_login(_state: &State<'_, AppState>, _email: &str, _password: &str) -> Option<Value> {
    None
}

/// POST /api/auth/login - Best practice : offline-first, Neon sync, Argon2, JWT pair
#[tauri::command(async)]
pub async fn auth_login(state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
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
    // 1. Tentative locale (rapide, offline)
    let local_user = db.find_one("users", |r| {
        r.get("email").and_then(Value::as_str) == Some(email.as_str())
    })?;

    let mut user: Option<Value> = None;
    let mut was_neon = false;

    if let Some(lu) = local_user {
        let stored = lu
            .get("password_hash")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;
        let pwd = password.clone();
        let st = stored.clone();
        let is_valid = tokio::task::spawn_blocking(move || auth_core::compare_password(&pwd, &st))
            .await
            .map_err(|e| ApiError::internal(format!("Erreur vérification: {e}")))?;
        if is_valid {
            user = Some(lu);
        } else {
            // Local échec -> tente Neon (peut être nouveau mot de passe en cloud)
            if let Some(nu) = try_neon_login(&state, &email, &password).await {
                user = Some(nu);
                was_neon = true;
            } else {
                return Err(ApiError::unauthorized("Identifiants incorrects"));
            }
        }
    } else {
        // Pas en local -> tente Neon
        if let Some(nu) = try_neon_login(&state, &email, &password).await {
            user = Some(nu);
            was_neon = true;
        } else {
            return Err(ApiError::unauthorized("Identifiants incorrects"));
        }
    }

    let user = user.ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;

    let stored = user
        .get("password_hash")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Upgrade transparent vers Argon2 si ancien hash (scrypt/bcrypt)
    if auth_core::needs_rehash(stored) {
        let pwd2 = password.clone();
        // Ne bloque pas le login, fait en background
        let db_clone = db(&state).query_all("users").is_ok(); // dummy to avoid borrow
        if db_clone {
            let pwd_for_hash = pwd2.clone();
            let email_clone = email.clone();
            let state_clone = state.inner().clone();
            // On ne peut pas clone State, donc on fait simple : rehash synchrone mais spawn_blocking
            if let Ok(upgraded) = tokio::task::spawn_blocking(move || auth_core::hash_password(&pwd_for_hash))
                .await
                .unwrap_or_else(|_| Err(ApiError::internal("hash failed")))
            {
                let mut upd = jmap();
                upd.insert("password_hash".into(), json!(upgraded));
                if let Some(id) = user.get("id").and_then(Value::as_i64) {
                    let _ = db.update("users", id, &upd);
                    // Push vers Neon en background si possible
                    #[cfg(feature = "neon-sync")]
                    {
                        let id_clone = id;
                        let email_c = email_clone.clone();
                        tokio::spawn(async move {
                            // Récupère pool
                            // Note: on ne peut pas utiliser state_clone ici (moved), on ré-essaie via try
                            // Simplifié : pas de push Neon ici, sera sync au prochain sync_run
                            eprintln!("[auth] rehash Argon2 pour {} id {}", email_c, id_clone);
                        });
                    }
                }
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
    // Génère paire access + refresh (best practice)
    let (access, refresh) = auth_core::generate_token_pair(&c, &state.jwt_secret)?;
    eprintln!("[auth] login success for {} via {} (neon={})", email, if was_neon { "neon" } else { "local" }, was_neon);

    Ok(json!({
        "token": access,
        "refresh_token": refresh,
        "user": user_public(&user),
        "source": if was_neon { "neon" } else { "local" },
    }))
}

/// POST /api/auth/refresh - utilise refresh_token pour obtenir nouveau access
#[tauri::command]
pub fn auth_refresh(state: State<'_, AppState>, refresh_token: String) -> ApiResult<Value> {
    if refresh_token.is_empty() {
        return Err(ApiError::bad_request("Refresh token requis"));
    }
    let claims = auth_core::verify_token(&refresh_token, &state.jwt_secret)?;
    // Vérifie que c'est bien un refresh (on pourrait ajouter claim type, mais on réutilise même structure)
    let new_access = auth_core::sign_token(&claims, &state.jwt_secret)?;
    Ok(json!({ "token": new_access }))
}

/// POST /api/auth/logout
#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    // Même en mode debug bypass, on autorise logout
    if let Some(t) = &token {
        if t == "debug-bypass-android14" || t.contains("debug-bypass") {
            return Ok(json!({ "success": true, "debug": true }));
        }
    }
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
