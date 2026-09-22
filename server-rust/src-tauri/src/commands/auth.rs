// Auth — CONNEXION 100 % EN LIGNE.
//
// Règle absolue de cette version : aucun compte, aucun mot de passe (même haché)
// ne vit sur l'appareil. Le flux est :
//   aucune session locale -> écran LOGIN -> auth_login demande au cloud de
//   vérifier le mot de passe -> jeton + identité (id, email, nom, rôle) gardés
//   localement -> écran d'accueil.
//
// Conséquences assumées :
//   * sans Internet, on ne peut pas OUVRIR une session (message explicite) ;
//   * une session déjà ouverte continue de fonctionner hors ligne ;
//   * un compte supprimé par un administrateur est détecté (account_watcher),
//     l'appareil efface alors toutes ses données et revient à l'écran de login.
use serde_json::{Value, json};
use tauri::State;

use crate::auth as auth_core;
use crate::commands::{claims, db, user_public};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

const LOGIN_WINDOW_MS: i64 = 15 * 60 * 1000;
/// Au-delà de ce nombre d'échecs sur 15 minutes, le compte est mis en pause
/// côté appareil : cela protège le cloud d'un essai de mots de passe en rafale.
const MAX_ECHECS: usize = 10;

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

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

fn trop_d_echecs(state: &State<'_, AppState>, key: &str) -> bool {
    let now = now_ms();
    state
        .login_attempts
        .lock()
        .ok()
        .and_then(|map| map.get(key).cloned())
        .map(|v| v.iter().filter(|t| now - **t < LOGIN_WINDOW_MS).count() >= MAX_ECHECS)
        .unwrap_or(false)
}

/// Comparaison du mot de passe saisi avec le hash reçu du cloud.
fn compare_direct(password: &str, stored: &str) -> bool {
    auth_core::compare_password(password, stored)
}

fn hash_algo(stored: &str) -> &'static str {
    if stored.starts_with("scrypt$v1") { "scrypt" }
    else if stored.starts_with("$argon2") { "argon2" }
    else if stored.starts_with("$2") { "bcrypt" }
    else { "inconnu" }
}

/// Identité de la session courante, gardée en clair MAIS SANS SECRET dans
/// app_settings : le surveillant de compte s'en sert pour demander au cloud
/// « ce compte existe-t-il encore ? ».
pub fn enregistrer_session(database: &crate::db::Db, id: i64, email: &str, nom: &str, role: &str) {
    let _ = database.set_setting(
        "session_user",
        &json!({ "id": id, "email": email, "nom": nom, "role": role }).to_string(),
    );
}

/// Connexion email + mot de passe : EN LIGNE UNIQUEMENT.
///
/// 1. le cloud est interrogé (le hash ne quitte jamais le serveur vers SQLite) ;
/// 2. le mot de passe saisi est comparé au hash renvoyé, en mémoire ;
/// 3. en cas de succès : jeton signé + identité mise en cache (id, email, nom,
///    rôle) — jamais de mot de passe sur l'appareil ;
/// 4. sans réseau : erreur 503 claire, aucune tentative de vérification locale.
#[tauri::command]
pub fn auth_login(state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
    let email = email.trim().to_ascii_lowercase();
    let password = password.trim().to_string();

    if email.is_empty() || password.is_empty() {
        return Err(ApiError::bad_request("Email et mot de passe requis"));
    }
    let cle_echecs = format!("email:{email}");
    if trop_d_echecs(&state, &cle_echecs) {
        return Err(ApiError::new(
            429,
            "Trop de tentatives échouées. Patientez quelques minutes avant de réessayer.",
        ));
    }

    // Connexion cloud obligatoire (pool déjà chaud, ou monté à la volée).
    let pool = tauri::async_runtime::block_on(async {
        crate::supabase::get_supabase_pool(&state).await
    })
    .map_err(|_| {
        ApiError::service_unavailable(
            "Connexion impossible sans Internet : les comptes sont vérifiés en ligne.",
        )
    })?;

    // Lecture bornée : un réseau mobile mort ne doit pas figer l'écran de login.
    let cloud = tauri::async_runtime::block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_millis(9000),
            crate::supabase::fetch_supabase_user(&pool, &email),
        )
        .await
    })
    .map_err(|_| {
        ApiError::service_unavailable(
            "Le serveur n'a pas répondu à temps. Vérifiez votre connexion et réessayez.",
        )
    })??;

    let Some(cloud_user) = cloud else {
        record_failure(&state, &cle_echecs);
        crate::logger::log_auth(&format!("login refusé : {email} inconnu du serveur"));
        return Err(ApiError::unauthorized("Identifiants incorrects"));
    };
    if cloud_user.deleted {
        crate::logger::log_auth(&format!("login refusé : {email} archivé côté serveur"));
        return Err(ApiError::unauthorized(
            "Ce compte a été désactivé : demandez sa restauration à un administrateur",
        ));
    }
    if !compare_direct(&password, &cloud_user.password_hash) {
        record_failure(&state, &cle_echecs);
        crate::logger::log_auth(&format!(
            "login refusé : mot de passe incorrect pour {email} (algo {})",
            hash_algo(&cloud_user.password_hash)
        ));
        return Err(ApiError::unauthorized("Identifiants incorrects"));
    }

    // Identité seulement : nom, email, rôle. AUCUN hash n'est écrit en SQLite.
    let database = db(&state);
    let _ = database.cache_identite_utilisateur(
        cloud_user.id,
        &cloud_user.email,
        &cloud_user.nom,
        &cloud_user.role,
        false,
    );
    enregistrer_session(database, cloud_user.id, &cloud_user.email, &cloud_user.nom, &cloud_user.role);

    let c = auth_core::Claims {
        id: cloud_user.id,
        email: cloud_user.email.clone(),
        role: cloud_user.role.clone(),
        nom: cloud_user.nom.clone(),
        iat: 0,
        exp: 0,
    };
    let (access, refresh) = auth_core::generate_token_pair(&c, &state.jwt_secret)?;
    state.session_authenticated.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::logger::log_auth(&format!("login OK (en ligne) pour {email}"));
    Ok(json!({
        "token": access,
        "refresh_token": refresh,
        "user": {
            "id": cloud_user.id,
            "email": cloud_user.email,
            "role": cloud_user.role,
            "nom": cloud_user.nom,
        },
        "source": "cloud",
    }))
}

/// POST /api/auth/account-check — LE COMPTE CONNECTÉ EXISTE-T-IL ENCORE ?
///
/// Appelé au démarrage, au retour de l'application au premier plan, et toutes
/// les minutes par le surveillant. Si l'administrateur a supprimé (ou désactivé)
/// le compte, TOUTES les données de l'appareil sont effacées et le frontend est
/// renvoyé sur l'écran de connexion.
/// Hors ligne, on ne conclut RIEN : pas de réseau n'est pas une suppression.
#[tauri::command(async)]
pub async fn auth_account_check(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    token: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    let resultat = crate::account_watcher::verifier(&app, &state).await;
    Ok(json!({
        "valid": !resultat.revoque,
        "revoked": resultat.revoque,
        "checked": resultat.verifie,
        "reason": resultat.raison,
        "message": resultat.message,
        "user": { "id": user.id, "email": user.email },
    }))
}

/// POST /api/auth/local-wipe — efface TOUTES les données de l'application sur
/// cet appareil (compte supprimé, ou remise à zéro demandée par l'utilisateur).
/// Volontairement sans contrôle de jeton : c'est justement quand le compte n'est
/// plus valide qu'il faut pouvoir nettoyer.
#[tauri::command]
pub fn auth_local_wipe(state: State<'_, AppState>) -> ApiResult<Value> {
    let database = db(&state);
    database.effacer_donnees_locales()?;
    let _ = database.set_setting("session_backup", "");
    let _ = database.set_setting("session_user", "");
    state.session_authenticated.store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(json!({ "wiped": true }))
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
        // Aucun compte n'est stocké localement : l'identité du jeton suffit
        // (elle a été validée en ligne au moment de la connexion).
        None => Ok(json!({ "user": {
            "id": c.id,
            "email": c.email,
            "nom": c.nom,
            "role": c.role,
        }})),
    }
}

// ===== Session persistante durable (SQLite) =====
// Le localStorage du WebView Android n'est pas garanti persistant apres un
// redemarrage de l'application. On garde donc une copie de la session dans
// la table settings de SQLite pour pouvoir restaurer la connexion.

#[tauri::command]
pub fn auth_session_save(
    state: tauri::State<'_, crate::AppState>,
    payload: String,
) -> crate::error::ApiResult<serde_json::Value> {
    let db = crate::commands::db(&state);
    db.set_setting("session_backup", &payload)
        .map_err(|e| crate::error::ApiError::new(500, &format!("Sauvegarde de session impossible: {e}")))?;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub fn auth_session_load(
    state: tauri::State<'_, crate::AppState>,
) -> crate::error::ApiResult<serde_json::Value> {
    let db = crate::commands::db(&state);
    let session = db.get_setting("session_backup").ok().flatten().unwrap_or_default();
    Ok(serde_json::json!({ "session": session }))
}

#[tauri::command]
pub fn auth_session_clear(
    state: tauri::State<'_, crate::AppState>,
) -> crate::error::ApiResult<serde_json::Value> {
    let db = crate::commands::db(&state);
    let _ = db.set_setting("session_backup", "");
    Ok(serde_json::json!({ "ok": true }))
}
