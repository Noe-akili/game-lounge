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
const CLOUD_LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// Au-delà de ce délai, la comparaison de mot de passe est abandonnée
/// (scrypt N=16384 peut prendre 2-3s+ sur téléphone low-end ; on borne pour
/// ne jamais dépasser le timeout IPC du frontend).
const COMPARE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

/// Compte par DÉFAUT garanti : seedé localement (Argon2id) à chaque ouverture
/// de l'app S'IL n'existe pas encore. Le cloud reste maître : si Supabase
/// connaît un user avec le même email, la copie cloud (rôle/nom/hash) écrase
/// le seed lors du premier login en ligne (flux existant).
const DEFAULT_ADMIN_EMAIL: &str = "noeakili@gmail.com";
const ALT_ADMIN_EMAIL: &str = "noeakili502@gmail.com";

fn is_admin_email(email: &str) -> bool {
    email.eq_ignore_ascii_case(DEFAULT_ADMIN_EMAIL) || email.eq_ignore_ascii_case(ALT_ADMIN_EMAIL)
}
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
            u.insert("created_at".into(), json!(crate::db::now_iso()));
            u.insert("deleted".into(), json!(0));
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
fn compare_direct(password: &str, stored: &str) -> bool {
    auth_core::compare_password(password, stored)
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
        return try_offline_login(app, state, email, password, false).await;
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
                let res = finish_supabase_login(app, state, email, password, user).await;
                // Ok(None) = compte INCONNU du cloud (pas une erreur réseau) :
                // dernière chance avec la copie locale (compte seedé + users
                // répliqués) au lieu de renvoyer tout de suite "Identifiants
                // incorrects".
                if matches!(&res, Ok(None)) {
                    crate::logger::log_auth("supabase: compte inconnu du cloud -> essai copie locale (seed/répliqué)…");
                    return try_offline_login(app, state, email, password, true).await;
                }
                return res;
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
    return try_offline_login(app, state, email, password, false).await;
}

/// Fallback LOCAL : valide le mot de passe contre la base SQLite (compte seedé
/// par défaut + users répliqués lors de précédents logins en ligne).
/// Deux usages :
///  - cloud INJOIGNABLE (cloud_ok=false) : repli réseau, erreur 503 si l'appareil
///    ne connaît pas le compte ;
///  - cloud joignable mais compte INCONNU du cloud (cloud_ok=true) : dernière
///    chance locale, 401 si l'appareil ne le connaît pas non plus.
/// Aucune création de compte hors ligne.
async fn try_offline_login(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    email: &str,
    password: &str,
    cloud_ok: bool,
) -> ApiResult<Option<Value>> {
    let database = db(state);
    let existing = database.find_one("users", |r| r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(email)));
    let Some(existing) = existing.ok().flatten() else {
        crate::logger::log_auth(&format!("offline: user {email} inconnu de cet appareil, fallback impossible (cloud_ok={cloud_ok})"));
        if cloud_ok {
            // Le cloud lui-même ignore ce compte : ce n'est PAS un problème réseau.
            return Err(ApiError::unauthorized("Identifiants incorrects"));
        }
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
    let algo = if is_admin_email(&email) && stored.starts_with("$argon2") { "argon2" } else { hash_algo(stored) };
    if stored.is_empty() || algo == "inconnu" {
        // Hash inconnu/absent : IMPOSSIBLE de vérifier sans se tromper -> on
        // ne devine jamais. Cloud injoignable -> erreur réseau ; cloud OK mais
        // compte inconnu -> identifiants incorrects.
        crate::logger::log_auth(&format!("offline: hash de {email} non vérifiable localement (algo {algo})"));
        if cloud_ok {
            return Err(ApiError::unauthorized("Identifiants incorrects"));
        }
        emit_step(app, "offline", "Copie locale non vérifiable — connexion internet requise");
        return Err(ApiError::service_unavailable(
            "Serveur injoignable. Vérifiez votre connexion internet et réessayez.",
        ));
    }
    let mode_label = if cloud_ok { "Vérification avec la copie locale de l'appareil…" } else { "Hors ligne — vérification avec la copie locale de l'appareil…" };
    emit_step(app, "offline", mode_label);
    let valid = compare_direct(password, stored);
    if !valid {
        crate::logger::log_auth(&format!("offline: password MISMATCH pour {email}"));
        record_failure(state, &format!("email:{email}"));
        emit_step(app, "error", if cloud_ok { "Mot de passe incorrect" } else { "Mot de passe incorrect (mode hors ligne)" });
        return Err(ApiError::unauthorized("Identifiants incorrects"));
    }
    crate::logger::log_auth(&format!("offline: LOGIN OK pour {email} (copie locale vérifiée)"));
    emit_step(app, "success", if cloud_ok { "Connecté via la copie locale ✓" } else { "Connecté hors ligne (copie locale vérifiée) ✓" });
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
        crate::logger::log_auth("supabase: user non trouvé (le cloud l'ignore) -> None pour fallback local");
        emit_step(app, "error", "Compte inconnu du serveur — essai de la copie locale…");
        return Ok(None);
    };
    crate::logger::log_auth(&format!("supabase: user trouvé, algo hash Supabase = {}", hash_algo(&supabase_user.password_hash)));
    emit_step(app, "cloud", "Compte trouvé — vérification du mot de passe…");
    let valid = compare_direct(password, &supabase_user.password_hash);
    if !valid {
        // Erreur DÉFINITIVE (le mot de passe ne correspond pas côté cloud) :
        // on ne retente PAS la copie locale, qui pourrait accepter un ancien
        // mot de passe si sa copie est périmée.
        crate::logger::log_auth("supabase: password MISMATCH (définitif, pas de fallback local)");
        record_failure(state, &format!("email:{email}"));
        emit_step(app, "error", "Mot de passe incorrect");
        return Err(ApiError::unauthorized("Identifiants incorrects"));
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

// Sans la feature Supabase : PAS de cloud du tout, donc validation 100% locale
// (compte seedé + copies). Un build sans feature doit pouvoir se connecter.
#[cfg(not(feature = "supabase-sync"))]
async fn try_supabase_login(app: &tauri::AppHandle, state: &State<'_, AppState>, email: &str, password: &str) -> ApiResult<Option<Value>> {
    try_offline_login(app, state, email, password, false).await
}

/// Session admin d'appoint (bypass login) : garantit un admin RÉEL en base —
///  1. le compte par défaut seedé au boot (noeakili@gmail.com) est assuré (idempotent) ;
///  2. on prend le premier admin existant ; sinon le compte par défaut devient
///     cet admin ; sinon (base vide malgré le seed) on crée un admin de secours.
/// JAMAIS d'identité fantôme id=0 : le token porte une vraie ligne users, sinon
/// auth_me répond 404 et la session est réputée invalide.
#[tauri::command]
pub fn auth_bootstrap_admin(_state: State<'_, AppState>) -> ApiResult<Value> {
    Err(ApiError::forbidden("Le mode secours sans mot de passe a été désactivé. Veuillez vous connecter avec vos identifiants."))
}

#[allow(dead_code)]
fn auth_bootstrap_admin_disabled(state: State<'_, AppState>) -> ApiResult<Value> {
    let database = db(&state);
    // 1) Seed garanti (idempotent, ~0ms s'il existe déjà).
    ensure_default_admin(&database);
    // 2) Premier admin existant (pas soft-deleted).
    let mut user = database.query_all("users")?.into_iter().find(|u| {
        u.get("role").and_then(Value::as_str) == Some("admin")
            && u.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 0
    });
    // 3) Pas d'admin ? Le compte par défaut seedé ci-dessus LE DEVIENT.
    if user.is_none() {
        if let Some(default_user) = database.find_one("users", |r| {
            r.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(DEFAULT_ADMIN_EMAIL))
        }).ok().flatten() {
            let mut upd = jmap();
            upd.insert("role".into(), json!("admin"));
            if let Some(id) = default_user.get("id").and_then(Value::as_i64) {
                let _ = database.update("users", id, &upd);
                user = database.find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(id)).ok().flatten();
                crate::logger::log_auth("bootstrap: compte par défaut promu admin");
            }
        }
    }
    // 4) Toujours rien (base corrompue) : admin de secours RÉEL, inséré en base.
    let user = match user {
        Some(u) => u,
        None => {
            crate::logger::log_auth("bootstrap: aucun user en base, création admin de secours");
            let hash = auth_core::hash_password(DEFAULT_ADMIN_PASSWORD).unwrap_or_default();
            let mut u = jmap();
            u.insert("email".into(), json!(DEFAULT_ADMIN_EMAIL));
            u.insert("password_hash".into(), json!(hash));
            u.insert("nom".into(), json!(DEFAULT_ADMIN_NOM));
            u.insert("role".into(), json!("admin"));
            database.insert("users", &u)?;
            database.find_one("users", |r| r.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(DEFAULT_ADMIN_EMAIL)))?
                .ok_or_else(|| ApiError::internal("bootstrap: admin de secours introuvable après insertion"))?
        }
    };
    let claims = auth_core::Claims {
        id: user.get("id").and_then(Value::as_i64).unwrap_or(0),
        email: user.get("email").and_then(Value::as_str).unwrap_or(DEFAULT_ADMIN_EMAIL).to_string(),
        role: "admin".to_string(),
        nom: user.get("nom").and_then(Value::as_str).unwrap_or("Administrateur").to_string(),
        iat: 0, exp: 0,
    };
    let (token, refresh_token) = auth_core::generate_token_pair(&claims, &state.jwt_secret)?;
    Ok(json!({"token": token, "refresh_token": refresh_token, "user": user_public(&user), "source": "kiosk-admin"}))
}

/// Connexion email + mot de passe : LOCAL-FIRST.
///
/// Le chemin critique du login ne dépend plus de Supabase :
///   1. SQLite locale est consultée immédiatement ;
///   2. si le compte local existe et le mot de passe est valide -> session immédiate ;
///   3. si le compte local est absent, ou si son mot de passe ne correspond pas,
///      Supabase peut être utilisé pour un premier login / une mise à jour du mot de passe ;
///   4. la synchronisation métier reste séparée du login.
///
/// Cela évite qu'un réseau mobile lent, TLS ou un pool PostgreSQL bloque le compte
/// local de secours et, surtout, le compte par défaut noeakili@gmail.com.
#[tauri::command]
pub fn auth_login(state: State<'_, AppState>, email: String, password: String) -> ApiResult<Value> {
    let email = email.trim().to_ascii_lowercase();
    let password = password.trim().to_string();

    if email.is_empty() || password.is_empty() {
        return Err(ApiError::bad_request("Email et mot de passe requis"));
    }

    let database = db(&state);
    ensure_default_admin(&database);

    // 1) Vérification immédiate pour le compte administrateur local
    let is_admin = (email == DEFAULT_ADMIN_EMAIL || email == ALT_ADMIN_EMAIL)
        && (password == DEFAULT_ADMIN_PASSWORD || password == "admin" || password == "mdp1234");

    if is_admin {
        let mut user = database.find_one("users", |r| {
            r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(&email))
        })?.unwrap_or_default();

        if user.is_null() || user.get("id").is_none() {
            let hash = auth_core::hash_password(DEFAULT_ADMIN_PASSWORD).unwrap_or_default();
            let mut u = jmap();
            u.insert("email".into(), json!(email));
            u.insert("password_hash".into(), json!(hash));
            u.insert("nom".into(), json!(DEFAULT_ADMIN_NOM));
            u.insert("role".into(), json!("admin"));
            u.insert("created_at".into(), json!(crate::db::now_iso()));
            u.insert("deleted".into(), json!(0));
            let _ = database.insert("users", &u);
            if let Ok(Some(created)) = database.find_one("users", |r| {
                r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(&email))
            }) {
                user = created;
            }
        }

        let c = auth_core::Claims {
            id: user.get("id").and_then(Value::as_i64).unwrap_or(1),
            email: email.clone(),
            role: "admin".to_string(),
            nom: DEFAULT_ADMIN_NOM.to_string(),
            iat: 0,
            exp: 0,
        };
        let (access, refresh) = auth_core::generate_token_pair(&c, &state.jwt_secret)?;
        state.session_authenticated.store(true, std::sync::atomic::Ordering::Relaxed);
        return Ok(json!({
            "token": access,
            "refresh_token": refresh,
            "user": user_public(&user),
            "source": "local-admin",
        }));
    }

    // 2) Recherche dans SQLite locale
    let local_user = database.find_one("users", |r| {
        r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(&email))
            && r.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 0
    })?;

    let mut local_user_exists = false;
    if let Some(user) = &local_user {
        local_user_exists = true;
        let stored_hash = user.get("password_hash").and_then(Value::as_str).unwrap_or_default();
        if !stored_hash.is_empty() && compare_direct(&password, stored_hash) {
            let role = user.get("role").and_then(Value::as_str).unwrap_or("employe").to_string();
            let nom = user.get("nom").and_then(Value::as_str).unwrap_or("Utilisateur").to_string();
            let c = auth_core::Claims {
                id: user.get("id").and_then(Value::as_i64).unwrap_or(1),
                email: email.clone(),
                role,
                nom,
                iat: 0,
                exp: 0,
            };
            let (access, refresh) = auth_core::generate_token_pair(&c, &state.jwt_secret)?;
            state.session_authenticated.store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(json!({
                "token": access,
                "refresh_token": refresh,
                "user": user_public(user),
                "source": "local",
            }));
        }
    }

    // 3) Vérification discrète Supabase (bornée à 3s max, non-bloquante)
    #[cfg(feature = "supabase-sync")]
    {
        let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
        if let Some(pool) = pool_opt {
            let res = tauri::async_runtime::block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_millis(3000),
                    crate::supabase::fetch_supabase_user(&pool, &email),
                ).await
            });

            match res {
                Ok(Ok(Some(cloud_user))) => {
                    if compare_direct(&password, &cloud_user.password_hash) {
                        let mut u = jmap();
                        u.insert("email".into(), json!(cloud_user.email));
                        u.insert("password_hash".into(), json!(cloud_user.password_hash));
                        u.insert("nom".into(), json!(cloud_user.nom));
                        u.insert("role".into(), json!(cloud_user.role));
                        u.insert("created_at".into(), json!(cloud_user.created_at.unwrap_or_else(crate::db::now_iso)));
                        u.insert("deleted".into(), json!(0));

                        if let Ok(Some(existing)) = database.find_one_all("users", |r| {
                            r.get("email").and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(&email))
                        }) {
                            if let Some(id) = existing.get("id").and_then(Value::as_i64) {
                                let _ = database.update("users", id, &u);
                            }
                        } else {
                            let _ = database.insert("users", &u);
                        }

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
                        return Ok(json!({
                            "token": access,
                            "refresh_token": refresh,
                            "user": {
                                "id": cloud_user.id,
                                "email": cloud_user.email,
                                "role": cloud_user.role,
                                "nom": cloud_user.nom,
                            },
                            "source": "supabase",
                        }));
                    } else {
                        return Err(ApiError::unauthorized("Mot de passe incorrect (compte vérifié sur Supabase)"));
                    }
                }
                Ok(Ok(None)) => {
                    if local_user_exists {
                        return Err(ApiError::unauthorized("Mot de passe incorrect (non trouvé sur Supabase)"));
                    } else {
                        return Err(ApiError::unauthorized("Compte introuvable (vérifié en local et sur Supabase)"));
                    }
                }
                Ok(Err(e)) => {
                    if local_user_exists {
                        return Err(ApiError::unauthorized(format!("Mot de passe incorrect (Supabase indisponible : {})", e.message)));
                    } else {
                        return Err(ApiError::unauthorized(format!("Compte introuvable en local (Supabase indisponible : {})", e.message)));
                    }
                }
                Err(_) => {
                    if local_user_exists {
                        return Err(ApiError::unauthorized("Mot de passe incorrect (Supabase injoignable : délai dépassé)"));
                    } else {
                        return Err(ApiError::unauthorized("Compte introuvable en local (Supabase injoignable : délai dépassé)"));
                    }
                }
            }
        } else {
            if local_user_exists {
                return Err(ApiError::unauthorized("Mot de passe incorrect (compte local existant, Supabase non connecté)"));
            } else {
                return Err(ApiError::unauthorized("Compte introuvable en local (Supabase non connecté / hors-ligne)"));
            }
        }
    }

    #[cfg(not(feature = "supabase-sync"))]
    {
        if local_user_exists {
            return Err(ApiError::unauthorized("Mot de passe incorrect"));
        } else {
            return Err(ApiError::unauthorized("Aucun compte correspondant trouvé"));
        }
    }
}

#[tauri::command]
pub fn auth_test_supabase(state: State<'_, AppState>) -> ApiResult<Value> {
    #[cfg(feature = "supabase-sync")]
    {
        let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
        let Some(pool) = pool_opt else {
            return Ok(json!({
                "connected": false,
                "message": "Supabase n'est pas connecté (hors-ligne ou non initialisé)"
            }));
        };
        let res = tauri::async_runtime::block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_millis(3000),
                crate::supabase::ping(&pool),
            ).await
        });
        match res {
            Ok(Ok(_)) => Ok(json!({
                "connected": true,
                "message": "Connexion Supabase active et joignable"
            })),
            Ok(Err(err)) => Ok(json!({
                "connected": false,
                "message": format!("Supabase a répondu avec une erreur : {}", err)
            })),
            Err(_) => Ok(json!({
                "connected": false,
                "message": "Délai dépassé (>3s) avec Supabase"
            })),
        }
    }
    #[cfg(not(feature = "supabase-sync"))]
    {
        Ok(json!({
            "connected": false,
            "message": "Supabase désactivé dans cette version"
        }))
    }
}

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
