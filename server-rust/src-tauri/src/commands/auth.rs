// Auth : authentification email/mot de passe (auth_login) + session admin
// d'appoint pour le premier démarrage (auth_bootstrap_admin).
//
// Le flux OBLIGATOIRE de l'app : aucune session locale -> écran LOGIN ->
// auth_login (SQLite locale d'abord, Supabase en secours) -> token + user
// sauvegardés -> dashboard. Le bootstrap admin n'est PAS une connexion
// automatique : il ne sert qu'au tout premier lancement (installation vierge)
// et n'est JAMAIS déclenché après un 401.
use serde_json::{Value, json};
use tauri::State;

use crate::auth as auth_core;
use crate::commands::{admin_only, claims, db, jmap, user_public};
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

/// Compte par DÉFAUT garanti : seedé localement (Argon2id) à chaque ouverture
/// de l'app S'IL n'existe pas encore. Le cloud reste maître : si Supabase
/// connaît un user avec le même email, la copie cloud (rôle/nom/hash) écrase
/// le seed lors du premier login en ligne (flux existant).
const DEFAULT_ADMIN_EMAIL: &str = "noeakili@gmail.com";
const DEFAULT_ADMIN_PASSWORD: &str = "mdp1234";
const DEFAULT_ADMIN_NOM: &str = "Noé Akili";
const DEFAULT_ADMIN_ROLE: &str = "admin";

/// Seed SYNCHRONE du compte par défaut (appelé au boot de l'app, AVANT tout
/// login). Hash Argon2id calculé une seule fois si le compte n'existe pas —
/// ~100-300ms, acceptable au boot (hors chemin UI).
fn seed_impl(db: &crate::db::Db) {
    let exists = db
        .find_one("users", |r| {
            r.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(DEFAULT_ADMIN_EMAIL))
        })
        .ok()
        .flatten()
        .is_some();
    if exists {
        crate::logger::log_auth("seed: compte par défaut déjà présent");
        return;
    }
    match auth_core::hash_password(DEFAULT_ADMIN_PASSWORD) {
        Ok(hash) => {
            let mut u = jmap();
            u.insert("email".into(), json!(DEFAULT_ADMIN_EMAIL));
            u.insert("password_hash".into(), json!(hash));
            u.insert("nom".into(), json!(DEFAULT_ADMIN_NOM));
            u.insert("role".into(), json!(DEFAULT_ADMIN_ROLE));
            match db.insert("users", &u) {
                Ok(_) => crate::logger::log_auth("seed: compte par défaut créé (noeakili@gmail.com)"),
                Err(e) => crate::logger::log_auth(&format!("seed: ÉCHEC création compte par défaut: {e}")),
            }
        }
        Err(e) => crate::logger::log_auth(&format!("seed: hash impossible: {e}")),
    }
}

/// Émet une étape de progression du login au WebView (console de l'écran
/// login, événement "login-step"). Fire-and-forget : jamais d'erreur si
/// aucun listener. Permet à l'utilisateur de voir OÙ le login bloque.
fn emit_step(app: &tauri::AppHandle, step: &str, detail: &str) {
    use tauri::Emitter;
    let _ = app.emit("login-step", json!({
        "step": step,
        "detail": detail,
        "ts": chrono::Utc::now().timestamp_millis(),
    }));
}

/// Ne compte que les ÉCHECS : trop de tentatives ne doit pas bloquer un login
/// qui finit par réussir.
fn record_failure(state: &State<'_, AppState>, key: &str) {
    let now = now_ms();
    if let Ok(mut map) = state.login_attempts.lock() {
        let entries = map.entry(key.to_string()).or_default();
        entries.retain(|t| now - *t < LOGIN_WINDOW_MS);
        entries.push(now);
    }
}

fn clear_failures(state: &State<'_, AppState>, key: &str) {
    if let Ok(mut map) = state.login_attempts.lock() {
        map.remove(key);
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
/// Seed lisible depuis l'extérieur du module (lib.rs au boot).
pub fn ensure_default_admin(db: &crate::db::Db) { seed_impl(db) }

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
    let mut pool_opt = { state.supabase_pool.lock().ok().and_then(|g| g.clone()) };
    // Le pool s'initialise 1,5s après le boot PUIS dépend du réseau mobile (peut
    // prendre 10s+ à monter). Au premier login juste après l'ouverture de l'app il
    // est donc souvent absent : au lieu d'échouer immédiatement ("Identifiants
    // incorrects" trompeur), on TENTE une initialisation à la volée (borne 8s par
    // init_supabase_pool). Résultat mis en cache dans AppState pour les logins suivants.
    if pool_opt.is_none() {
        crate::logger::log_auth("supabase: pool absent au login, tentative d'initialisation à la volée...");
        let init = crate::supabase::init_supabase_pool().await;
        if let Some(p) = init {
            crate::logger::log_auth("supabase: pool créé au login");
            if let Ok(mut guard) = state.supabase_pool.lock() {
                *guard = Some(p.clone());
            }
            pool_opt = Some(p);
        }
    }
    let Some(mut pool) = pool_opt else {
        crate::logger::log_auth("supabase: pool non disponible (offline) — init à la volée a échoué, fallback local…");
        // CLOUD INJOIGNABLE DÈS LE DÉPART : repli sur la copie locale des
        // users déjà connus de cet appareil (répliqués lors d'un précédent
        // login en ligne). Si l'appareil ne connaît pas ce user -> 503 réseau.
        return try_offline_login(app, state, email, password).await;
    };
    crate::logger::log_auth(&format!("supabase: fetch user {}", email));
    emit_step(app, "cloud", "Serveur joint — recherche du compte…");
    // RETRY 1 fois : la cause n°1 d'échec est une connexion morte (Supabase ferme
    // les connexions idle). On redemande un pool tout neuf et on retente la
    // requête UNE fois avant de renvoyer le 503 à l'utilisateur.
    let mut last_err: Option<ApiError> = None;
    for attempt in 1..=2u32 {
        match crate::supabase::fetch_supabase_user(&pool, email).await {
            Ok(user) => {
                if attempt > 1 {
                    crate::logger::log_auth("supabase: retry 1 a réussi (connexion morte restaurée)");
                }
                return finish_supabase_login(app, state, email, password, user).await;
            }
            Err(e) => {
                // Classification lisible : timeout réseau vs erreur SQL vs autre
                let is_timeout = e.message.contains("timeout");
                let is_sql = e.status == 500 && !is_timeout;
                let kind = if is_timeout { "timeout réseau (serveur injoignable ou trop lent)"
                          } else if is_sql { "erreur SQL/serveur Postgres"
                          } else { "erreur inconnue" };
                crate::logger::log_auth(&format!(
                    "supabase: tentative {attempt}/2 ÉCHOUÉE pour {email} [{kind}]: {}",
                    e.message
                ));
                last_err = Some(e);
                if attempt == 1 {
                    emit_step(app, "cloud", "Connexion instable — nouvelle tentative…");
                    crate::logger::log_auth("supabase: reconnexion du pool et retry...");
                    // Reconnexion SYNCHRONE (bornée ~14s max par les timeouts de
                    // init_supabase_pool) : récupère un pool tout neuf ou échoue.
                    match crate::supabase::reconnect_now(app).await {
                        Some(fresh) => {
                            crate::logger::log_auth("supabase: nouveau pool disponible pour le retry");
                            emit_step(app, "cloud", "Serveur rejoint — nouvel essai…");
                            pool = fresh;
                        }
                        None => {
                            crate::logger::log_auth("supabase: reconnexion échouée (réseau toujours indisponible), abandon du retry");
                            emit_step(app, "error", "Impossible de joindre le serveur");
                            break;
                        }
                    }
                }
            }
        }
    }
    // 503 explicite (PAS "Identifiants incorrects") : l'utilisateur comprend
    // que c'est le RÉSEAU/le cloud, pas son mot de passe. Dernier recours avant
    // d'échouer : repli sur la copie locale si l'appareil connaît ce user.
    let detail = last_err.map(|e| e.message).unwrap_or_else(|| "cause inconnue".into());
    crate::logger::log_auth(&format!("supabase: LOGIN IMPOSSIBLE après retry pour {email}: {detail} — fallback local…"));
    return try_offline_login(app, state, email, password).await;
}

/// Fallback OFFLINE : valide le mot de passe contre les users DÉJÀ répliqués
/// sur CET appareil (lors d'un précédent login en ligne). Supabase reste la
/// vérification principale ; ce repli ne joue QUE si le cloud est injoignable.
/// Aucune création de compte hors ligne : un user inconnu de l'appareil ne
/// peut PAS se connecter sans réseau -> erreur réseau explicite (503).
#[cfg(feature = "supabase-sync")]
async fn try_offline_login(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    email: &str,
    password: &str,
) -> ApiResult<Option<Value>> {
    let database = db(state);
    let existing = database.find_one("users", |r| r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(email)));
    let Some(existing) = existing.ok().flatten() else {
        crate::logger::log_auth(&format!("offline: user {email} inconnu de cet appareil, fallback impossible"));
        emit_step(app, "offline", "Hors ligne et appareil ne connaissant pas ce compte");
        return Err(ApiError::service_unavailable(
            "Serveur injoignable. Vérifiez votre connexion internet et réessayez.",
        ));
    };
    if existing.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1 {
        crate::logger::log_auth(&format!("offline: user {email} soft-deleted, fallback refusé"));
        return Err(ApiError::unauthorized("Identifiants incorrects"));
    }
    let stored = existing.get("password_hash").and_then(Value::as_str).unwrap_or("");
    let algo = if email.eq_ignore_ascii_case(DEFAULT_ADMIN_EMAIL) && stored.starts_with("$argon2") { "argon2" } else { hash_algo(stored) };
    if stored.is_empty() || algo == "inconnu" {
        // Hash inconnu/absent : IMPOSSIBLE de vérifier sans se tromper -> on
        // ne devine jamais, on renvoie l'erreur réseau.
        crate::logger::log_auth(&format!("offline: hash de {email} non vérifiable localement (algo {algo})"));
        emit_step(app, "offline", "Copie locale non vérifiable — connexion internet requise");
        return Err(ApiError::service_unavailable(
            "Serveur injoignable. Vérifiez votre connexion internet et réessayez.",
        ));
    }
    emit_step(app, "offline", "Hors ligne — vérification avec la copie locale de l'appareil…");
    let valid = compare_bounded(password.to_string(), stored.to_string()).await;
    if !valid {
        crate::logger::log_auth(&format!("offline: password MISMATCH pour {email}"));
        emit_step(app, "error", "Mot de passe incorrect (mode hors ligne)");
        return Err(ApiError::unauthorized("Identifiants incorrects"));
    }
    crate::logger::log_auth(&format!("offline: LOGIN OK pour {email} (copie locale vérifiée)"));
    emit_step(app, "success", "Connecté hors ligne (copie locale vérifiée) ✓");
    let mut nu = existing.clone();
    if let Some(obj) = nu.as_object_mut() {
        obj.insert("_source".into(), json!("offline"));
    }
    Ok(Some(nu))
}

/// Validation du mot de passe + réplication du user vers SQLite, une fois le
/// user récupéré de Supabase (séparé pour rendre le retry de fetch lisible).
#[cfg(feature = "supabase-sync")]
async fn finish_supabase_login(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    email: &str,
    password: &str,
    supabase_user: Option<crate::supabase::SupabaseUser>,
) -> ApiResult<Option<Value>> {
    let Some(supabase_user) = supabase_user else {
        crate::logger::log_auth("supabase: user non trouvé");
        emit_step(app, "error", "Compte introuvable sur le serveur");
        return Ok(None);
    };
    crate::logger::log_auth(&format!("supabase: user trouvé, algo hash Supabase = {}", hash_algo(&supabase_user.password_hash)));
    emit_step(app, "cloud", "Compte trouvé — vérification du mot de passe…");
    let valid = compare_bounded(password.to_string(), supabase_user.password_hash.clone()).await;
    if !valid {
        crate::logger::log_auth("supabase: password MISMATCH");
        emit_step(app, "error", "Mot de passe incorrect");
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
    // Après validation cloud, conserve le hash distant localement pour permettre
    // une connexion hors ligne ; il est ensuite migré vers Argon2id localement.
    let database = db(state);
    match database.find_one_all("users", |r| r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(email))) {
        Ok(Some(existing)) => {
            // Utilisateur soft-deleted : login refusé, on ne le ressuscite pas
            if existing.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1 {
                crate::logger::log_auth(&format!("supabase: user {} soft-deleted, login refusé", email));
                return Ok(None);
            }
            let mut updates = jmap();
            updates.insert("role".into(), json!(supabase_user.role));
            updates.insert("nom".into(), json!(supabase_user.nom));
            updates.insert("password_hash".into(), json!(supabase_user.password_hash));
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

#[cfg(not(feature = "supabase-sync"))]
async fn try_supabase_login(_app: &tauri::AppHandle, _state: &State<'_, AppState>, _email: &str, _password: &str) -> ApiResult<Option<Value>> { Ok(None) }

/// Session admin d'appoint : réservée au TOUT PREMIER lancement (installation
/// vierge) pour permettre d'entrer dans l'app et créer le vrai compte admin.
/// N'est PAS une connexion automatique et n'est JAMAIS appelée après un 401.
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

/// Connexion email + mot de passe : vérification Supabase directe.
/// AUCUNE recherche SQLite préalable : le flux est Obligatoire ->
/// backend Rust/Tauri -> Supabase -> verification email+mot de passe.
/// Si correct -> session immédiate + réplication user vers SQLite -> token/user.
/// Si incorrect -> "Identifiants incorrects" immédiatement.
/// Ne JAMAIS supprimer la vérification Supabase.
#[tauri::command(async)]
pub async fn auth_login(app: tauri::AppHandle, state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
    let email = email.trim().to_ascii_lowercase();
    let rate_key = format!("email:{}", email);
    let t0 = now_ms();
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
    if too_many_failures(&state, &rate_key) {
        crate::logger::log_auth("login refusé: trop d'échecs (rate limit)");
        return Err(ApiError::new(429, "Trop de tentatives, réessayez plus tard"));
    }

    emit_step(&app, "backend", "Identifiants reçus — vérification en cours…");

    // --- ALLER DIRECTEMENT vers Supabase, aucun check SQLite préalable ---
    let t1 = now_ms();
    let supabase_result = try_supabase_login(&app, &state, &email, &password).await;
    crate::logger::log_auth(&format!("try_supabase_login: {} ms, ok={}", now_ms() - t1, supabase_result.is_ok()));

    let mut user: Option<Value> = None;
    let mut was_supabase = false;
    let database = db(&state);

    match supabase_result? {
        Some(nu) => {
            // User validé par Supabase : réplique vers SQLite pour offline capability
            let database = db(&state);
            let mut map = jmap();
            map.insert("id".into(), json!(nu.get("id").unwrap_or(&Value::Null)));
            map.insert("email".into(), json!(nu.get("email").unwrap_or(&Value::Null)));
            map.insert("password_hash".into(), json!(nu.get("password_hash").unwrap_or(&Value::Null)));
            map.insert("role".into(), json!(nu.get("role").unwrap_or(&Value::Null)));
            map.insert("nom".into(), json!(nu.get("nom").unwrap_or(&Value::Null)));

            // Insert ou upsert local : on crée le user local pour permettre
            // les connexions hors offline suivantes (offline-first).
            let existing = database.find_one("users", |r| r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(&email)))?;
            if let Some(existing) = existing {
                // Si l'utilisateur est soft-deleted, on refuse le login
                if existing.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1 {
                    crate::logger::log_auth(&format!("login: user {} soft-deleted, refused", email));
                    record_failure(&state, &rate_key);
                    return Err(ApiError::unauthorized("Identifiants incorrects"));
                }
                // Met à jour les infos (role, nom, hash) depuis Supabase
                let mut updates = jmap();
                updates.insert("role".into(), json!(nu.get("role").unwrap_or(&Value::Null)));
                updates.insert("nom".into(), json!(nu.get("nom").unwrap_or(&Value::Null)));
                updates.insert("password_hash".into(), json!(nu.get("password_hash").unwrap_or(&Value::Null)));
                let _ = database.update("users", existing.get("id").and_then(Value::as_i64).unwrap(), &updates);
            } else {
                let _ = database.insert("users", &map);
            }

            was_supabase = nu.get("_source").and_then(Value::as_str) != Some("offline");
            user = Some(nu);
        }
        None => {
            record_failure(&state, &rate_key);
            emit_step(&app, "error", "Compte non reconnu (ou serveur injoignable)");
            return Err(ApiError::unauthorized("Identifiants incorrects"));
        }
    }

    let user = user.ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"))?;
    clear_failures(&state, &rate_key);
    crate::logger::log_auth(&format!("AUTH_SUCCESS ({} ms)", now_ms() - t0));
    emit_step(&app, "success", "Connexion validée — création de la session…");
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
    let failures = state.login_attempts.lock().map(|m| m.get("local").cloned().unwrap_or_default().len()).unwrap_or(0);
    Ok(json!({ "users": list, "login_failures_recent": failures }))
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

/// RÈGLE ABSOLUE (démarrage) : lève le verrou des processus métier.
/// Appelé par le frontend UNIQUEMENT après affichage de l'écran d'accueil
/// (login réussi ou session restaurée) : à partir de là seulement, la sync
/// automatique, le watcher de sessions et les notifications s'activent.
#[tauri::command]
pub fn auth_business_ready(state: State<'_, AppState>) -> ApiResult<Value> {
    state.session_authenticated.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::logger::log("session", "business processes ENABLED (accueil affiché)");
    Ok(json!({ "business_ready": true }))
}

/// RÈGLE ABSOLUE (démarrage) : abaisse le verrou des processus métier
/// (déconnexion / session expirée). La sync auto, le watcher et les
/// notifications se remettent en veille jusqu'à la prochaine authentification.
#[tauri::command]
pub fn auth_business_suspend(state: State<'_, AppState>) -> ApiResult<Value> {
    state.session_authenticated.store(false, std::sync::atomic::Ordering::Relaxed);
    crate::logger::log("session", "business processes SUSPENDED (déconnexion)");
    Ok(json!({ "business_ready": false }))
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
