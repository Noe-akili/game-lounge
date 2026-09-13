// Auth propre - offline-first + Supabase + Argon2
use serde_json::{Value, json};
use tauri::State;

use crate::auth as auth_core;
use crate::commands::{claims, db, jmap, user_public};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

const LOGIN_WINDOW_MS: i64 = 15 * 60 * 1000;
const LOGIN_MAX_FAILURES: usize = 20;
/// Au-delà de ce délai, la comparaison de mot de passe est abandonnée
/// (scrypt N=16384 peut prendre 2-3s+ sur téléphone low-end ; on borne pour
/// ne jamais dépasser le timeout IPC du frontend).
const COMPARE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

/// Ne compte que les ÉCHECS : trop Many attempts ne doit pas bloquer un login
/// qui finit par réussir (comportement avant : chaque réussite comptait aussi).
fn record_failure(state: &State<'_, AppState>, key: &str) {
    let now = now_ms();
    if let Ok(mut map) = state.login_attempts.lock() {
        let entries = map.entry(key.to_string()).or_default();
        entries.retain(|t| now - *t < LOGIN_WINDOW_MS);
        entries.push(now);
    }
}

fn too_many_failures(state: &State<'_, AppState>, key: &str) -> bool {
    let now = now_ms();
    match state.login_attempts.lock() {
        Ok(mut map) => {
            let entries = map.entry(key.to_string()).or_default();
            entries.retain(|t| now - *t < LOGIN_WINDOW_MS);
            entries.len() >= LOGIN_MAX_FAILURES
        }
        Err(_) => false, // mutex poisonné : on ne bloque personne
    }
}

/// Comparaison mot de passe BORNÉE : si le hash est trop lent (ou le thread
/// spawn_blocking ne répond pas), on abandonne au lieu de hanguer 20s+.
async fn compare_bounded(password: String, stored: String) -> bool {
    match tokio::time::timeout(COMPARE_TIMEOUT, tokio::task::spawn_blocking(move || {
        auth_core::compare_password(&password, &stored)
    })).await {
        Ok(Ok(valid)) => valid,
        Ok(Err(_)) => false,
        Err(_) => {
            crate::logger::log("auth", "compare_password TIMEOUT (>5s) - abandon");
            false
        }
    }
}

/// Vérifie que le hash stocké est dans un algo supporté (scrypt/argon2/bcrypt).
/// Un hash inconnu (ex: plain text, vide, autre algo) -> comparaison inutile.
fn hash_algo(stored: &str) -> &'static str {
    if stored.starts_with("scrypt$v1") { "scrypt" }
    else if stored.starts_with("$argon2") { "argon2" }
    else if stored.starts_with("$2") { "bcrypt" }
    else { "inconnu" }
}

// Retourne Err en cas de problème Supabase (timeout/connexion morte) pour ne pas répondre
// faussement "Identifiants incorrects" ; déclenche aussi la reconnexion en arrière-plan.
#[cfg(feature = "supabase-sync")]
async fn try_supabase_login(app: &tauri::AppHandle, state: &State<'_, AppState>, email: &str, password: &str) -> ApiResult<Option<Value>> {
    let pool_opt = { state.supabase_pool.lock().ok().and_then(|g| g.clone()) };
    let Some(pool) = pool_opt else {
        crate::logger::log_auth("supabase: pool non disponible (offline), skip");
        return Ok(None);
    };
    crate::logger::log_auth(&format!("supabase: fetch user {}", email));
    match crate::supabase::fetch_supabase_user(&pool, email).await {
        Ok(Some(supabase_user)) => {
            crate::logger::log_auth(&format!("supabase: user trouvé, algo hash Supabase = {}", hash_algo(&supabase_user.password_hash)));
            let valid = compare_bounded(password.to_string(), supabase_user.password_hash.clone()).await;
            if !valid {
                crate::logger::log_auth("supabase: password MISMATCH");
                return Ok(None);
            }
            crate::logger::log_auth("supabase: password OK");
            let mut map = jmap();
            map.insert("id".into(), json!(supabase_user.id));
            map.insert("email".into(), json!(supabase_user.email));
            map.insert("password_hash".into(), json!(supabase_user.password_hash));
            map.insert("role".into(), json!(supabase_user.role));
            map.insert("nom".into(), json!(supabase_user.nom));
            if let Some(ca) = supabase_user.created_at { map.insert("created_at".into(), json!(ca)); }
            // Persiste en local : met à jour role/nom mais NE REMPLACE PAS un hash local
            // existant par le hash Supabase (algos possiblement incompatibles).
            let database = db(state);
            match database.find_one_all("users", |r| r.get("email").and_then(Value::as_str) == Some(email)) {
                Ok(Some(existing)) => {
                    // Utilisateur soft-deleted : login refusé, on ne le ressuscite pas
                    if existing.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1 {
                        crate::logger::log_auth(&format!("supabase: user {} soft-deleted, login refusé", email));
                        return Ok(None);
                    }
                    let mut updates = jmap();
                    updates.insert("role".into(), json!(supabase_user.role));
                    updates.insert("nom".into(), json!(supabase_user.nom));
                    if let Some(id) = existing.get("id").and_then(Value::as_i64) {
                        let _ = database.update("users", id, &updates);
                        if let Ok(Some(u)) = database.find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(id)) { return Ok(Some(u)); }
                    }
                }
                Ok(None) => { if let Ok(u) = database.insert("users", &map) { return Ok(Some(u)); } }
                _ => {}
            }
            Ok(Some(json!({"id": supabase_user.id, "email": supabase_user.email, "role": supabase_user.role, "nom": supabase_user.nom, "password_hash": supabase_user.password_hash})))
        }
        Ok(None) => { crate::logger::log_auth("supabase: user non trouvé"); Ok(None) }
        Err(e) => {
            crate::logger::log_auth(&format!("supabase: fetch error pour {}: {} -> reconnexion", email, e.message));
            crate::supabase::schedule_reconnect(app);
            Err(ApiError::new(503, "Supabase indisponible (reconnexion en cours), réessayez"))
        }
    }
}
#[cfg(not(feature = "supabase-sync"))]
async fn try_supabase_login(_app: &tauri::AppHandle, _state: &State<'_, AppState>, _email: &str, _password: &str) -> ApiResult<Option<Value>> { Ok(None) }

#[tauri::command(async)]
pub async fn auth_login(app: tauri::AppHandle, state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
    let t0 = now_ms();
    // Étapes de diagnostic chronométrées (mission §1) : permettent de dire exactement
    // "Login total: Xs, Supabase Auth: Ys, SQLite: Zs" dans 1.log. AUCUN mot de
    // passe ni hash n'apparaît jamais dans ces logs (mission §15).
    crate::logger::log_auth(&format!("LOGIN_START {} ({} ms)", email, now_ms() - t0));

    if email.is_empty() || password.is_empty() {
        return Err(ApiError::bad_request("Email et mot de passe requis"));
    }
    if !validators::is_valid_email(&email) {
        return Err(ApiError::bad_request("Email invalide"));
    }
    if !validators::is_valid_password(&password) {
        return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)"));
    }
    if too_many_failures(&state, "local") {
        crate::logger::log_auth("login refusé: trop d'échecs (rate limit)");
        return Err(ApiError::new(429, "Trop de tentatives, réessayez plus tard"));
    }

    let database = db(&state);
    let t_find = now_ms();
    let local_user = database.find_one("users", |r| r.get("email").and_then(Value::as_str) == Some(email.as_str()))?;
    crate::logger::log_auth(&format!("AUTH_DB_LOOKUP {} ms, trouvé={}", t_find - t0, local_user.is_some()));
    crate::logger::log_auth("AUTH_START");

    let user: Option<Value>;
    let mut was_supabase = false;

    if let Some(lu) = local_user {
        // BUG FIX : un user local SANS hash (importé de Supabase avec hash vide/absent) échouait
        // immédiatement ici (ok_or_else) SANS jamais tenter Supabase -> login impossible même
        // avec le bon mot de passe. On tente maintenant Supabase dans ce cas.
        let stored_opt = lu.get("password_hash").and_then(Value::as_str);
        let t1 = now_ms();
        let is_valid = match stored_opt {
            Some(stored) => {
                crate::logger::log_auth(&format!("hash local: algo={}, len={}", hash_algo(stored), stored.len()));
                compare_bounded(password.clone(), stored.to_string()).await
            }
            None => {
                crate::logger::log_auth("hash local ABSENT -> tentative Supabase");
                false
            }
        };
        crate::logger::log_auth(&format!("compare local: {} ms, valid={}", now_ms() - t1, is_valid));
        if is_valid {
            user = Some(lu);
        } else {
            // Hash local incompatible ou mot de passe différent -> tente Supabase
            let t2 = now_ms();
            let supabase_result = try_supabase_login(&app, &state, &email, &password).await;
            crate::logger::log_auth(&format!("try_supabase_login: {} ms, ok={}", now_ms() - t2, supabase_result.is_ok()));
            match supabase_result? {
                Some(nu) => { user = Some(nu); was_supabase = true; }
                None => {
                    record_failure(&state, "local");
                    return Err(ApiError::unauthorized("Identifiants incorrects"));
                }
            }
        }
    } else {
        let t2 = now_ms();
        let supabase_result = try_supabase_login(&app, &state, &email, &password).await;
        crate::logger::log_auth(&format!("try_supabase_login (pas de local): {} ms, ok={}", now_ms() - t2, supabase_result.is_ok()));
        match supabase_result? {
            Some(nu) => { user = Some(nu); was_supabase = true; }
            None => {
                record_failure(&state, "local");
                return Err(ApiError::unauthorized("Identifiants incorrects"));
            }
        }
    }

    let user = user.ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;
    crate::logger::log_auth(&format!("AUTH_SUCCESS ({} ms)", now_ms() - t0));
    let stored = user.get("password_hash").and_then(Value::as_str).unwrap_or("");

    // Rehash Argon2 seulement si hash local reconnu mais legacy (scrypt/bcrypt) :
    // on ne rehash JAMAIS un hash "inconnu" (on ne pourrait plus jamais vérifier).
    if auth_core::needs_rehash(stored) {
        let t3 = now_ms();
        let pwd_for_hash = password.clone();
        if let Ok(upgraded) = tokio::task::spawn_blocking(move || auth_core::hash_password(&pwd_for_hash)).await.unwrap_or_else(|_| Err(ApiError::internal("hash failed"))) {
            let mut upd = jmap();
            upd.insert("password_hash".into(), json!(upgraded));
            if let Some(id) = user.get("id").and_then(Value::as_i64) {
                let _ = database.update("users", id, &upd);
                crate::logger::log_auth(&format!("rehash Argon2 pour {} id {} ({} ms)", email, id, now_ms() - t3));
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
    crate::logger::log_auth(&format!("SESSION_SAVED ({} ms)", now_ms() - t0));
    crate::logger::log_auth(&format!("LOGIN_RETURN {} via {} (login total {} ms)", email, if was_supabase { "supabase" } else { "local" }, now_ms() - t0));

    Ok(json!({
        "token": access,
        "refresh_token": refresh,
        "user": user_public(&user),
        "source": if was_supabase { "supabase" } else { "local" },
    }))
}

/// Diagnostic : état des users locaux (algo de hash, sans jamais exposer le hash)
#[tauri::command]
pub fn auth_debug_info(state: State<'_, AppState>) -> ApiResult<Value> {
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
    let failures = state.login_attempts.lock().map(|m| m.get("local").cloned().unwrap_or_default().len()).unwrap_or(0);
    Ok(json!({ "users": list, "login_failures_recent": failures }))
}

/// Diagnostic : liste les users côté Supabase (email + algo de hash uniquement)
#[cfg(feature = "supabase-sync")]
#[tauri::command(async)]
pub async fn auth_debug_supabase_users(state: State<'_, AppState>) -> ApiResult<Value> {
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
pub async fn auth_debug_supabase_users(_state: State<'_, AppState>) -> ApiResult<Value> {
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
