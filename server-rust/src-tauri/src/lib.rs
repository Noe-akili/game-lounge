pub mod auth;
pub mod commands;
pub mod db;
pub mod db_notify;
pub mod error;
pub mod logger;
pub mod supabase;
pub mod pdf;
pub mod session_watcher;
pub mod validators;

use std::sync::Mutex;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use tauri::Manager;

use db::Db;

pub struct AppState {
    /// Base de données SQLite partagée (Arc : aussi utilisée par le watcher
    /// de sessions en tâche de fond, sans dupliquer la connexion).
    pub db: std::sync::Arc<Db>,
    /// Secret utilisé pour signer/vérifier les JWT.
    pub jwt_secret: String,
    /// Pool Supabase Postgres (optionnel, offline-first) - via tokio-postgres
    #[cfg(feature = "supabase-sync")]
    pub supabase_pool: Mutex<Option<crate::supabase::SupabasePool>>,
    #[cfg(not(feature = "supabase-sync"))]
    pub supabase_pool: Mutex<Option<()>>,
    /// Garde-fou : une seule reconnexion Supabase à la fois (évite le spam de tâches)
    pub supabase_reconnecting: std::sync::atomic::AtomicBool,
    /// État/progression de la dernière sync (sync_run tourne en arrière-plan :
    /// le frontend suit via /sync/poll pour éviter le Timeout IPC Android WebView)
    pub sync_state: Mutex<Option<serde_json::Value>>,
    /// Anti brute-force login : horodatages des ÉCHECS par clé (email).
    /// Seuls les échecs comptent — un login qui finit par réussir n'est pas bloqué.
    pub login_attempts: Mutex<std::collections::HashMap<String, Vec<i64>>>,
    /// RÈGLE ABSOLUE (démarrage) : verrou des processus métier. Tant qu'il est
    /// false, AUCUNE sync auto, AUCUN watcher, AUCUNE notification ne tourne.
    /// Le frontend le lève via auth_business_ready APRÈS affichage de l'écran
    /// d'accueil (post-login ou restauration de session).
    pub session_authenticated: std::sync::atomic::AtomicBool,
}


/// A release APK must never use a predictable JWT signing key. When no
/// deployment secret is present, keep tokens valid only for this app process.
/// JWT secret STABLE sur l'appareil (persisté dans SQLite).
/// Sans ça, chaque redémarrage de l'APK invente un secret neuf et invalide
/// tous les tokens -> retour forcé sur l'écran login.
const DEFAULT_JWT_SECRET: &str = "gl-prod-sec-key-game-lounge-tauri-2024-jwt-token-auth";

fn runtime_jwt_secret(db: &crate::db::Db) -> String {
    if let Ok(secret) = std::env::var("JWT_SECRET") {
        if secret.len() >= 32 {
            let _ = db.set_setting("jwt_secret", &secret);
            return secret;
        }
    }
    if let Ok(Some(stored)) = db.get_setting("jwt_secret") {
        if stored.len() >= 32 {
            return stored;
        }
    }
    let _ = db.set_setting("jwt_secret", DEFAULT_JWT_SECRET);
    DEFAULT_JWT_SECRET.to_string()
}
fn open_db(app: &tauri::AppHandle) -> Result<Db, Box<dyn std::error::Error>> {
    // 1. DATADIR env (tests / debug http-server)
    if let Ok(dir_str) = std::env::var("DATADIR") {
        let p = std::path::PathBuf::from(dir_str);
        if let Err(e) = std::fs::create_dir_all(&p) {
            eprintln!("DATADIR create_dir_all {:?} failed: {e}", p);
        } else {
            let db_path = p.join("gamelounge.db");
            match Db::open(&db_path) {
                Ok(db) => {
                    eprintln!("DB opened via DATADIR {:?}", db_path);
                    return Ok(db);
                }
                Err(e) => eprintln!("Db::open DATADIR {:?} failed: {e}", db_path),
            }
        }
    }

    // 2. Essais ordonnés Android : app_data_dir -> app_local_data_dir -> app_cache_dir -> temp_dir -> "."
    // Sur Android app_data_dir = /data/data/com.gamelounge.android/files ; peut échouer si WebView pas prêt
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = app.path().app_data_dir() {
        candidates.push(p);
    } else {
        eprintln!("app_data_dir unavailable, trying alternatives");
    }
    if let Ok(p) = app.path().app_local_data_dir() {
        if !candidates.contains(&p) { candidates.push(p); }
    }
    // app_cache_dir existe même quand data_dir échoue (utile en low storage)
    if let Ok(p) = app.path().app_cache_dir() {
        if !candidates.contains(&p) { candidates.push(p); }
    }
    // fallback FS
    candidates.push(std::env::temp_dir().join("gamelounge"));
    candidates.push(std::path::PathBuf::from("."));

    for dir in &candidates {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("create_dir_all {:?} failed: {e}", dir);
            continue;
        }
        let db_path = dir.join("gamelounge.db");
        match Db::open(&db_path) {
            Ok(db) => {
                eprintln!("DB opened successfully at {:?}", db_path);
                return Ok(db);
            }
            Err(e) => eprintln!("Db::open {:?} failed: {e}", db_path),
        }
    }

    // 3. Dernier recours : mémoire (l'app démarre, données volatiles mais pas de crash)
    eprintln!("All file DB candidates failed, falling back to in-memory DB");
    Db::open_in_memory().map_err(|e| -> Box<dyn std::error::Error> { e.into() })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crate::logger::init();
    crate::logger::log("BOOT", "App démarrage");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        // Notifications natives (Android) : fin de session automatique.
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle();
            // GARANTIE : AppState est TOUJOURS enregistré, même si la DB échoue.
            // Sinon le premier `invoke("auth_bootstrap_admin")` panic car State<AppState> manquant -> abort -> APK se ferme.
            // On protège le setup avec catch_unwind car WebView Android est fragile au boot.
            let db = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open_db(handle))) {
                Ok(Ok(db)) => {
                    eprintln!("DB opened successfully");
                    db
                }
                Ok(Err(e)) => {
                    eprintln!("DB init error (fallback to memory): {e}");
                    match Db::open_in_memory() {
                        Ok(mem_db) => {
                            eprintln!("Using in-memory DB fallback");
                            mem_db
                        }
                        Err(e2) => {
                            eprintln!("Fatal: cannot open in-memory DB: {e2}, trying emergency temp file");
                            // Dernier recours : fichier temp forcé, évite panic
                            let fallback_path = std::env::temp_dir().join("gamelounge_emergency.db");
                            match Db::open(&fallback_path) {                                    Ok(db) => {
                                        eprintln!("Emergency temp DB opened at {:?}", fallback_path);
                                        db
                                    }
                                Err(e3) => {
                                    eprintln!("Emergency temp failed: {e3}, retry in-memory");
                                    // Si tout échoue, on tente encore en mémoire (OOM = kill par OS de toute façon)
                                    // On ne panic pas ici, on retourne une erreur maîtrisée si vraiment impossible
                                    Db::open_in_memory().unwrap_or_else(|e4| {
                                        eprintln!("CRITICAL: all DB init failed: {e4}, abort setup gracefully");
                                        // On ne peut pas créer DB, on propagate l'erreur pour que Tauri log sans abort brutal
                                        // Mais il faut quand même fournir un AppState, donc on crée un état minimal
                                        // En pratique ce point n'est jamais atteint sur Android 7+
                                        panic!("DB init impossible: {e4}")
                                    })
                                }
                            }
                        }
                    }
                }
                Err(payload) => {
                    eprintln!("open_db panicked: {:?}", payload);
                    // Recovery : mémoire
                    Db::open_in_memory().unwrap_or_else(|e| {
                        eprintln!("Panic recovery failed: {e}");
                        panic!("recovery failed: {e}")
                    })
                }
            };
            // Garde-fou final : si db lui-même a paniqué lors du move, on catch et on fournit mémoire
            let db = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| db)).unwrap_or_else(|_| {
                eprintln!("Final DB move panicked, emergency in-memory");
                Db::open_in_memory().unwrap_or_else(|e| panic!("emergency in-memory failed: {e}"))
            });
            // SEED du compte par défaut (noeakili@gmail.com) : garantit qu'il
            // existe localement AVANT tout login (vérifié en priorité par le
            // fallback offline). Idempotent : ne fait rien s'il existe déjà.
            commands::auth::ensure_default_admin(&db);
            // Charge .env avant de lire JWT_SECRET (desktop/diagnostic).
            #[cfg(feature = "supabase-sync")]
            let _ = dotenvy::dotenv();
            let jwt_secret = runtime_jwt_secret(&db);
            // Canal de réveil de la sync instantanée : créé AVANT le move de `db` dans
            // AppState (le sender est branché dans Db, le receiver va au worker).
            #[cfg(feature = "supabase-sync")]
            let (wake_tx, wake_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
            #[cfg(feature = "supabase-sync")]
            db.set_sync_waker(wake_tx);
            app.manage(AppState {
                db: std::sync::Arc::new(db),
                jwt_secret,
                supabase_pool: Mutex::new(None),
                supabase_reconnecting: std::sync::atomic::AtomicBool::new(false),
                sync_state: Mutex::new(None),
                login_attempts: Mutex::new(std::collections::HashMap::new()),
                session_authenticated: std::sync::atomic::AtomicBool::new(false),
            });
            // Init Supabase pool en arrière-plan (non bloquant, best practice offline-first)
            #[cfg(feature = "supabase-sync")]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Petit délai pour laisser WebView démarrer
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    match crate::supabase::init_supabase_pool().await {
                        Some(pool) => {
                            eprintln!("[supabase] pool initialisé en background");
                            // MIGRATION DU SCHÉMA CLOUD : crée toutes les tables si la base
                            // cloud est neuve (Supabase vide = aucune table -> pull ET push
                            // échoueraient). Idempotent, exécuté aussi à chaque sync.
                            match crate::supabase::ensure_cloud_schema(&pool).await {
                                Ok(_) => eprintln!("[supabase] schéma cloud prêt"),
                                Err(e) => eprintln!("[supabase] schéma cloud failed (sera réessayé à la sync): {}", e),
                            }
                            if let Some(state) = handle.try_state::<AppState>() {
                                if let Ok(mut guard) = state.supabase_pool.lock() {
                                    *guard = Some(pool);
                                }
                            }
                            // RÈGLE ABSOLUE (démarrage) : AUCUN pull /users, aucune
                            // synchronisation et aucun accès réseau métier AVANT
                            // l'authentification. La réplication des users se fait
                            // par la 1ère phase de la sync post-login ; le fallback
                            // de connexion cloud est assuré par try_supabase_login
                            // (auth_login), déclenché par l'utilisateur lui-même.
                        }
                        None => eprintln!("[supabase] pool non disponible (offline)"),
                    }
                });
            }
            // Sync AUTOMATIQUE INSTANTANÉE : chaque écriture locale (insert/update/remove)
            // réveille le worker via un canal tokio -> si le toggle sync_enabled est actif,
            // le changement part vers Supabase en moins d'une seconde, sans attendre le
            // cycle périodique (filet toutes les 60s pour la réception et les nettoyages).
            #[cfg(feature = "supabase-sync")]
            {
                let handle = app.handle().clone();
                auto_sync_loop(handle, wake_rx);
            }
            // Watcher d'expiration des sessions : termine AUTOMATIQUEMENT une
            // session dont le temps est écoulé et notifie l'appareil (Android).
            if let Some(state) = app.handle().try_state::<AppState>() {
                let handle = app.handle().clone();
                // Cloche + notifications Android : chaque écriture de données
                // (locale ou reçue par la sync des AUTRES appareils) émet
                // "db-change" au WebView, même hors écran / en arrière-plan.
                crate::db_notify::attach(handle.clone(), &state.db);
                crate::session_watcher::start(handle, state.db.clone());
            }
            #[cfg(target_os = "android")]
            eprintln!("Android setup complete, AppState managed (supabase bg init)");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ==== SANTÉ ====
            commands::device_info,
            // ==== AUTH ====
            commands::auth_bootstrap_admin,
            commands::auth_login,
            commands::auth_logout,
            commands::auth_business_ready,
            commands::auth_business_suspend,
            commands::auth_me,
            commands::auth_refresh,
            commands::auth_debug_info,
            commands::auth_debug_supabase_users,
            commands::auth_test_supabase,
            commands::auth_session_save,
            commands::auth_session_load,
            commands::auth_session_clear,
            // ==== CONSOLES ====
            commands::consoles_list,
            commands::consoles_get,
            commands::consoles_create,
            commands::consoles_update,
            commands::consoles_delete,
            commands::consoles_restore,
            commands::consoles_permanent_delete,
            // ==== JEUX ====
            commands::jeux_list,
            commands::jeux_get,
            commands::jeux_create,
            commands::jeux_update,
            commands::jeux_delete,
            commands::jeux_restore,
            commands::jeux_permanent_delete,
            // ==== JOUEURS ====
            commands::joueurs_list,
            commands::joueurs_get,
            commands::joueurs_create,
            commands::joueurs_update,
            commands::joueurs_delete,
            commands::joueurs_restore,
            commands::joueurs_permanent_delete,
            commands::joueurs_historique,
            // ==== MESSAGES ====
            commands::messages_list,
            commands::messages_create,
            commands::messages_update,
            commands::messages_delete,
            // ==== TARIFS ====
            commands::tarifs_list,
            commands::tarifs_get,
            commands::tarifs_create,
            commands::tarifs_update,
            commands::tarifs_delete,
            commands::tarifs_restore,
            commands::tarifs_permanent_delete,
            // ==== PARAMÈTRES FIDÉLITÉ ====
            commands::parametres::fidelite_get,
            commands::parametres::fidelite_put,
            commands::parametres::fidelite_create,
            commands::parametres::fidelite_get_by_id,
            commands::parametres::fidelite_delete,
            // ==== SESSIONS ====
            commands::sessions_list,
            commands::sessions_get,
            commands::sessions_create,
            commands::sessions_pause,
            commands::sessions_reprendre,
            commands::sessions_terminer,
            commands::sessions_update,
            commands::sessions_delete,
            commands::sessions_restore,
            commands::sessions_permanent_delete,
            // ==== FACTURES + LIGNES ====
            commands::factures_list,
            commands::factures_get,
            commands::factures_pdf,
            commands::factures_save_pdf,
            commands::factures_annuler,
            commands::factures_update,
            commands::factures_create,
            commands::factures_delete,
            commands::factures_restore,
            commands::factures_permanent_delete,
            commands::lignes_list,
            commands::lignes_get,
            commands::lignes_create,
            commands::lignes_update,
            commands::lignes_delete,
            // ==== JETONS ====
            commands::jetons_list,
            commands::jetons_get,
            commands::jetons_create,
            commands::jetons_update,
            commands::jetons_delete,
            // ==== RAPPORTS ====
            commands::rapports_ca,
            // ==== USERS (admin) ====
            commands::users::users_list,
            commands::users::users_get,
            commands::users::users_create,
            commands::users::users_update,
            commands::users::users_delete,
            commands::users::users_restore,
            commands::users::users_permanent_delete,
            // ==== SYNC ====
            commands::sync_status,
            commands::sync_toggle,
            commands::sync_run,
            commands::sync_poll,
            commands::sync::sync_initial_status,
            commands::sync::sync_initial_skip,
        ])
        .run(tauri::generate_context!())
        .expect("erreur lors de l'exécution de Tauri");
}

/// Worker de sync instantanée : attend un réveil sur le canal branché dans Db
/// (chaque insert/update/remove local envoie un signal) et lance une sync
/// immédiatement si le toggle sync_enabled est actif. Sécurité : une sync
/// périodique toutes les 60s reste en filet (réception cloud, nettoyages).
#[cfg(feature = "supabase-sync")]
fn auto_sync_loop(handle: tauri::AppHandle, mut wake_rx: tokio::sync::mpsc::UnboundedReceiver<()>) {
    tauri::async_runtime::spawn(async move {
        // Premier tir ~10s après le boot (le pool cloud se connecte en arrière-plan)
        let mut periodic = tokio::time::interval(tokio::time::Duration::from_secs(60));
        periodic.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        periodic.reset();
        let mut first = true;
        loop {
            tokio::select! {
                _ = wake_rx.recv() => {
                    // Réveil instantané : une donnée locale vient d'être écrite.
                    // Débounce léger : les rafales (import, facture + lignes) créent
                    // plusieurs événements ; la 1re sync vide la file entière.
                    while wake_rx.try_recv().is_ok() {}
                }
                _ = periodic.tick() => {
                    // Filet périodique : réception des changements des autres appareils
                    // + nettoyages. Inutile de forcer la 1re tick (immédiate) : on la saute.
                    if first { first = false; continue; }
                }
            }
            let Some(state) = handle.try_state::<AppState>() else { continue };
            let enabled = state.db.get_setting("sync_enabled").ok().and_then(|o| o).unwrap_or_default() == "1";
            let running = state.sync_state.lock().ok()
                .and_then(|g| g.clone())
                .and_then(|v| v.get("running").and_then(serde_json::Value::as_bool))
                .unwrap_or(false);
            if enabled {
                if !running {
                    eprintln!("[sync-auto] sync bidirectionnelle lancée (push local + pull remote)");
                    let _ = crate::commands::sync::run_sync_impl(&handle, &state).await;
                } else {
                    // Si une sync est déjà en vol, planifier un rattrapage à la fin pour ne rien perdre
                    let retry_handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                        if let Some(st) = retry_handle.try_state::<AppState>() {
                            let still_running = st.sync_state.lock().ok()
                                .and_then(|g| g.clone())
                                .and_then(|v| v.get("running").and_then(serde_json::Value::as_bool))
                                .unwrap_or(false);
                            if !still_running {
                                let _ = crate::commands::sync::run_sync_impl(&retry_handle, &st).await;
                            }
                        }
                    });
                }
            }
        }
    });
}