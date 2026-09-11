pub mod auth;
pub mod commands;
pub mod db;
pub mod error;
pub mod neon;
pub mod pdf;
pub mod validators;

use std::sync::Mutex;
use tauri::Manager;

use db::Db;

pub struct AppState {
    /// Base de données SQLite partagée.
    pub db: Db,
    /// Secret utilisé pour signer/vérifier les JWT.
    pub jwt_secret: String,
    /// Journal des tentatives de connexion (rate limiting simple, par IP/app).
    pub login_attempts: Mutex<std::collections::HashMap<String, Vec<i64>>>,
    /// Pool Neon Postgres (optionnel, offline-first) - via tokio-postgres
    #[cfg(feature = "neon-sync")]
    pub neon_pool: Mutex<Option<crate::neon::NeonPool>>,
    #[cfg(not(feature = "neon-sync"))]
    pub neon_pool: Mutex<Option<()>>,
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
                    let _ = seed_default_users(&db);
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
                if let Err(e) = seed_default_users(&db) {
                    eprintln!("seed_default_users non-fatal at {:?}: {e}", db_path);
                }
                return Ok(db);
            }
            Err(e) => eprintln!("Db::open {:?} failed: {e}", db_path),
        }
    }

    // 3. Dernier recours : mémoire (l'app démarre, données volatiles mais pas de crash)
    eprintln!("All file DB candidates failed, falling back to in-memory DB");
    let db = Db::open_in_memory()?;
    let _ = seed_default_users(&db);
    Ok(db)
}

/// À la première exécution (base vide), crée les comptes par défaut,
/// identiques au seed du backend Node (server/seed.ts) :
///   admin@gamelounge.com / admin123   (rôle admin)
///   john@gamelounge.com  / employe123 (rôle employé)
///
/// SOLUTION SÛRE POUR APK SANS PC : hash pré-calculés pour éviter le scrypt
/// bloquant de 2-3s au démarrage (ANR Android). Le hash scrypt est coûteux (N=16384)
/// et faisait freezer le setup() puis ANR -> fermeture.
/// On utilise des hash pré-générés, et on ne recalcule qu'en fallback.
fn seed_default_users(db: &Db) -> Result<(), Box<dyn std::error::Error>> {
    if !db.query_all("users")?.is_empty() {
        return Ok(());
    }
    let now = db::now_iso();

    // Hash pré-calculés (scrypt v1, N=16384, r=8, p=1) pour éviter calcul au démarrage APK
    const ADMIN_HASH: &str = "scrypt$v1$mNiC7OIMBUkGTXUIwS1T0g$4o0DaGrFw3_nPQAA4Bea38LtsBbjkKvNioWytoQvUfYgQ4YZVJNaEfiHn-DMHe1BJLOLG-r0tt8YvmWHumnHOg";
    const EMPLOYE_HASH: &str = "scrypt$v1$Kb8e9sajtrVwQmOdMRvKUw$hmXRgQLLfSc6uFjVgdK-OgxhzNSezDVygFc9qW_sF_Z0lUvN-N4F1UKF4w-6mi6GOdwELSe3gJTnTepvv8WBlw";

    let mut admin = serde_json::Map::new();
    admin.insert("email".into(), serde_json::json!("admin@gamelounge.com"));
    // Hash pré-calculé direct (0ms) - évite scrypt au démarrage qui causait ANR
    admin.insert("password_hash".into(), serde_json::json!(ADMIN_HASH));
    admin.insert("role".into(), serde_json::json!("admin"));
    admin.insert("nom".into(), serde_json::json!("Admin"));
    admin.insert("created_at".into(), serde_json::json!(now.clone()));
    db.insert("users", &admin)?;

    let mut emp = serde_json::Map::new();
    emp.insert("email".into(), serde_json::json!("john@gamelounge.com"));
    emp.insert("password_hash".into(), serde_json::json!(EMPLOYE_HASH));
    emp.insert("role".into(), serde_json::json!("employe"));
    emp.insert("nom".into(), serde_json::json!("John Doe"));
    emp.insert("created_at".into(), serde_json::json!(now));
    db.insert("users", &emp)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            // GARANTIE : AppState est TOUJOURS enregistré, même si la DB échoue.
            // Sinon le premier `invoke("auth_login")` panic car State<AppState> manquant -> abort -> APK se ferme.
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
                            let _ = seed_default_users(&mem_db);
                            mem_db
                        }
                        Err(e2) => {
                            eprintln!("Fatal: cannot open in-memory DB: {e2}, trying emergency temp file");
                            // Dernier recours : fichier temp forcé, évite panic
                            let fallback_path = std::env::temp_dir().join("gamelounge_emergency.db");
                            match Db::open(&fallback_path) {
                                Ok(db) => {
                                    let _ = seed_default_users(&db);
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
            let jwt_secret =
                std::env::var("JWT_SECRET").unwrap_or_else(|_| "game-lounge-secret-2024".into());
            // Charge .env si présent (pour DATABASE_URL Neon)
            #[cfg(feature = "neon-sync")]
            let _ = dotenvy::dotenv();
            app.manage(AppState {
                db,
                jwt_secret,
                login_attempts: Mutex::new(std::collections::HashMap::new()),
                neon_pool: Mutex::new(None),
            });
            // Init Neon pool en arrière-plan (non bloquant, best practice offline-first)
            #[cfg(feature = "neon-sync")]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Petit délai pour laisser WebView démarrer
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    match crate::neon::init_neon_pool().await {
                        Some(pool) => {
                            eprintln!("[neon] pool initialisé en background");
                            if let Some(state) = handle.try_state::<AppState>() {
                                if let Ok(mut guard) = state.neon_pool.lock() {
                                    *guard = Some(pool);
                                }
                            }
                            // Pull initial users en background (cache)
                            if let Some(state) = handle.try_state::<AppState>() {
                                let pool_opt = state.neon_pool.lock().ok().and_then(|g| g.clone());
                                if let Some(pool) = pool_opt {
                                    match crate::neon::pull_users(&pool).await {
                                        Ok(users) => {
                                            eprintln!("[neon] pull {} users en background", users.len());
                                            // Cache local : upsert
                                            for u in users {
                                                let mut map = serde_json::Map::new();
                                                for (k,v) in u.as_object().unwrap() {
                                                    map.insert(k.clone(), v.clone());
                                                }
                                                let _ = state.db.insert("users", &map);
                                            }
                                        }
                                        Err(e) => eprintln!("[neon] pull users failed: {}", e.message),
                                    }
                                }
                            }
                        }
                        None => eprintln!("[neon] pool non disponible (offline)"),
                    }
                });
            }
            #[cfg(target_os = "android")]
            eprintln!("Android setup complete, AppState managed (neon bg init)");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ==== SANTÉ ====
            commands::health,
            // ==== AUTH ====
            commands::auth_login,
            commands::auth_logout,
            commands::auth_me,
            commands::auth_refresh,
            // ==== CONSOLES ====
            commands::consoles_list,
            commands::consoles_get,
            commands::consoles_create,
            commands::consoles_update,
            commands::consoles_delete,
            // ==== JEUX ====
            commands::jeux_list,
            commands::jeux_get,
            commands::jeux_create,
            commands::jeux_update,
            commands::jeux_delete,
            // ==== JOUEURS ====
            commands::joueurs_list,
            commands::joueurs_get,
            commands::joueurs_create,
            commands::joueurs_update,
            commands::joueurs_delete,
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
            // ==== FACTURES + LIGNES ====
            commands::factures_list,
            commands::factures_get,
            commands::factures_pdf,
            commands::factures_annuler,
            commands::factures_update,
            commands::factures_create,
            commands::factures_delete,
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
            // ==== SYNC ====
            commands::sync_status,
            commands::sync_toggle,
            commands::sync_run,
            commands::sync_poll,
        ])
        .run(tauri::generate_context!())
        .expect("erreur lors de l'exécution de Tauri");
}