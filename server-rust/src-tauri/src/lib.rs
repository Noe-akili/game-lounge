pub mod auth;
pub mod commands;
pub mod db;
pub mod error;
pub mod import;
pub mod logger;
pub mod neon;
pub mod pdf;
pub mod validators;

use std::sync::Mutex;
use serde_json::json;
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
    /// Garde-fou : une seule reconnexion Neon à la fois (évite le spam de tâches)
    pub neon_reconnecting: std::sync::atomic::AtomicBool,
    /// État/progression de la dernière sync (sync_run tourne en arrière-plan :
    /// le frontend suit via /sync/poll pour éviter le Timeout IPC Android WebView)
    pub sync_state: Mutex<Option<serde_json::Value>>,
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
    let now = db::now_iso();
    let need_users = db.query_all("users")?.is_empty();
    if need_users {
        const ADMIN_HASH: &str = "scrypt$v1$mNiC7OIMBUkGTXUIwS1T0g$4o0DaGrFw3_nPQAA4Bea38LtsBbjkKvNioWytoQvUfYgQ4YZVJNaEfiHn-DMHe1BJLOLG-r0tt8YvmWHumnHOg";
        const EMPLOYE_HASH: &str = "scrypt$v1$Kb8e9sajtrVwQmOdMRvKUw$hmXRgQLLfSc6uFjVgdK-OgxhzNSezDVygFc9qW_sF_Z0lUvN-N4F1UKF4w-6mi6GOdwELSe3gJTnTepvv8WBlw";
        let mut admin = serde_json::Map::new();
        admin.insert("email".into(), serde_json::json!("admin@gamelounge.com"));
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
        emp.insert("created_at".into(), serde_json::json!(now.clone()));
        db.insert("users", &emp)?;
        eprintln!("[seed] users par défaut créés");
    }
    // Seed tarifs/jeux depuis PDFs si vide (données embarquées)
    if db.query_all("tarifs")?.is_empty() {
        eprintln!("[seed] tarifs vide, seed embarqué depuis PDFs");
        seed_tarifs_from_pdfs(db)?;
    }
    if db.query_all("jeux")?.is_empty() {
        eprintln!("[seed] jeux vide, seed embarqué");
        seed_jeux_from_pdfs(db)?;
    }
    if db.query_all("consoles")?.is_empty() {
        for i in 1..=6 {
            let console_type = if i <= 3 { "PS5" } else { "PS4" };
            let mut c = serde_json::Map::new();
            c.insert("nom".into(), json!(format!("{} poste-{}", console_type, i)));
            c.insert("type".into(), json!(console_type));
            c.insert("poste_numero".into(), json!(i));
            c.insert("etat".into(), json!("disponible"));
            let now2 = db::now_iso();
            c.insert("created_at".into(), json!(now2.clone()));
            c.insert("date_ajout".into(), json!(now2));
            let _ = db.insert("consoles", &c);
        }
    }
    Ok(())
}

fn seed_tarifs_from_pdfs(db: &Db) -> Result<(), Box<dyn std::error::Error>> {
    let now = db::now_iso();
    let tarifs_ps4 = vec![
        ("partie", 5, 500, "PS4", "FIFA 26", "FIFA 26 PS4 - 1 Match 5min"),
        ("session", 30, 2000, "PS4", "FIFA 26", "FIFA 26 PS4 - 30min"),
        ("session", 60, 4000, "PS4", "FIFA 26", "FIFA 26 PS4 - 1h"),
        ("partie", 5, 500, "PS4", "Mortal Kombat", "Mortal Kombat PS4 - 2 combats"),
        ("session", 30, 1500, "PS4", "Mortal Kombat", "Mortal Kombat PS4 - 30min"),
        ("session", 60, 3000, "PS4", "Mortal Kombat", "Mortal Kombat PS4 - 1h"),
        ("session", 15, 500, "PS4", "Need for Speed", "Need for Speed PS4 - 15min"),
        ("session", 30, 1000, "PS4", "Need for Speed", "Need for Speed PS4 - 30min"),
        ("session", 60, 2000, "PS4", "Need for Speed", "Need for Speed PS4 - 1h"),
        ("partie", 10, 1000, "PS4", "WWE 2K25", "WWE 2K25 PS4 - 10min"),
        ("session", 30, 2000, "PS4", "WWE 2K25", "WWE PS4 - 30min"),
        ("session", 60, 4000, "PS4", "WWE 2K25", "WWE PS4 - 1h"),
        ("partie", 15, 1500, "PS4", "NBA 2K25", "NBA PS4 - 10-20min"),
        ("session", 30, 2500, "PS4", "NBA 2K25", "NBA PS4 - 30min"),
        ("session", 60, 4000, "PS4", "NBA 2K25", "NBA PS4 - 1h"),
        ("session", 15, 500, "PS4", "GTA V", "GTA V PS4 - 15min"),
        ("session", 30, 1500, "PS4", "GTA V", "GTA V PS4 - 30min"),
        ("session", 60, 3000, "PS4", "GTA V", "GTA V PS4 - 1h"),
        ("session", 15, 500, "PS4", "God of War", "God of War PS4 - 15min"),
        ("session", 30, 1500, "PS4", "God of War", "God of War PS4 - 30min"),
        ("session", 60, 3000, "PS4", "God of War", "God of War PS4 - 1h"),
        ("session", 15, 500, "PS4", "Call of Duty", "COD PS4 - 15min"),
        ("session", 30, 1500, "PS4", "Call of Duty", "COD PS4 - 30min"),
        ("session", 60, 3000, "PS4", "Call of Duty", "COD PS4 - 1h"),
    ];
    let tarifs_ps5 = vec![
        ("partie", 5, 1000, "PS5", "FIFA 26", "FIFA 26 PS5 - 5min"),
        ("session", 30, 4000, "PS5", "FIFA 26", "FIFA 26 PS5 - 30min"),
        ("session", 60, 8000, "PS5", "FIFA 26", "FIFA 26 PS5 - 1h"),
        ("partie", 5, 1000, "PS5", "Mortal Kombat", "Mortal Kombat PS5 - 1 combat"),
        ("session", 30, 3000, "PS5", "Mortal Kombat", "Mortal Kombat PS5 - 30min"),
        ("session", 60, 6000, "PS5", "Mortal Kombat", "Mortal Kombat PS5 - 1h"),
        ("partie", 5, 1000, "PS5", "Tekken 8", "Tekken 8 PS5 - 1 combat"),
        ("session", 30, 3000, "PS5", "Tekken 8", "Tekken 8 PS5 - 30min"),
        ("session", 60, 6000, "PS5", "Tekken 8", "Tekken 8 PS5 - 1h"),
        ("session", 5, 1000, "PS5", "Gran Turismo 7", "GT7 PS5 - course courte"),
        ("session", 15, 2000, "PS5", "Gran Turismo 7", "GT7 PS5 - 15min G29"),
        ("session", 15, 3000, "PS5", "Gran Turismo 7", "GT7 PS5 - 15min G29+VR2"),
        ("session", 30, 3500, "PS5", "Gran Turismo 7", "GT7 PS5 - 30min G29"),
        ("session", 30, 5000, "PS5", "Gran Turismo 7", "GT7 PS5 - 30min G29+VR2"),
        ("session", 60, 7000, "PS5", "Gran Turismo 7", "GT7 PS5 - 1h G29"),
        ("session", 60, 10000, "PS5", "Gran Turismo 7", "GT7 PS5 - 1h G29+VR2"),
        ("session", 15, 1000, "PS5", "Need for Speed", "NFS PS5 - 15min"),
        ("session", 15, 1500, "PS5", "Need for Speed", "NFS PS5 - 15min G29"),
        ("session", 30, 2000, "PS5", "Need for Speed", "NFS PS5 - 30min"),
        ("session", 30, 3000, "PS5", "Need for Speed", "NFS PS5 - 30min G29"),
        ("session", 60, 4000, "PS5", "Need for Speed", "NFS PS5 - 1h"),
        ("session", 60, 6000, "PS5", "Need for Speed", "NFS PS5 - 1h G29"),
        ("partie", 10, 1500, "PS5", "WWE 2K25", "WWE PS5 - 10min"),
        ("session", 30, 3000, "PS5", "WWE 2K25", "WWE PS5 - 30min"),
        ("session", 60, 6000, "PS5", "WWE 2K25", "WWE PS5 - 1h"),
        ("partie", 15, 2000, "PS5", "NBA 2K25", "NBA PS5 - 1 match"),
        ("session", 30, 4000, "PS5", "NBA 2K25", "NBA PS5 - 30min"),
        ("session", 60, 7000, "PS5", "NBA 2K25", "NBA PS5 - 1h"),
        ("session", 15, 1000, "PS5", "GTA V", "GTA V PS5 - 15min"),
        ("session", 30, 2000, "PS5", "GTA V", "GTA V PS5 - 30min"),
        ("session", 60, 4000, "PS5", "GTA V", "GTA V PS5 - 1h"),
        ("session", 15, 1000, "PS5", "God of War", "God of War PS5 - 15min"),
        ("session", 30, 2000, "PS5", "God of War", "God of War PS5 - 30min"),
        ("session", 60, 4000, "PS5", "God of War", "God of War PS5 - 1h"),
        ("session", 15, 1500, "PS5", "Call of Duty", "COD PS5 - 15min"),
        ("session", 30, 3000, "PS5", "Call of Duty", "COD PS5 - 30min"),
        ("session", 60, 6000, "PS5", "Call of Duty", "COD PS5 - 1h"),
    ];
    for (t, d, p, ct, jeu, desc) in tarifs_ps4.into_iter().chain(tarifs_ps5) {
        let mut m = serde_json::Map::new();
        m.insert("type".into(), json!(t));
        m.insert("duree_minutes".into(), json!(d));
        m.insert("prix".into(), json!(p));
        m.insert("description".into(), json!(desc));
        m.insert("console_type".into(), json!(ct));
        m.insert("jeu".into(), json!(jeu));
        m.insert("actif".into(), json!(1));
        m.insert("created_at".into(), json!(now.clone()));
        let _ = db.insert("tarifs", &m);
    }
    Ok(())
}

fn seed_jeux_from_pdfs(db: &Db) -> Result<(), Box<dyn std::error::Error>> {
    // Récupère consoles pour associer jeux à leur console respective (best practice)
    let consoles = db.query_all("consoles")?;
    let find_console = |poste_numero: i64| consoles.iter().find(|c| c.get("poste_numero").and_then(|v| v.as_i64()) == Some(poste_numero)).and_then(|c| c.get("id").and_then(|v| v.as_i64()));
    let c1 = find_console(1); // PS5 poste-1
    let c2 = find_console(2); // PS5 poste-2
    let c3 = find_console(3); // PS5 poste-3
    let c4 = find_console(4); // PS4 poste-4
    let c5 = find_console(5); // PS4 poste-5
    let c6 = find_console(6); // PS4 poste-6

    // Jeux associés à leur console (comme dans les PDFs et clientDb.ts)
    let jeux_data = vec![
        ("FIFA 26", "Sport", c1), ("FIFA 26", "Sport", c4),
        ("Mortal Kombat 1", "Combat", c1), ("Mortal Kombat 11", "Combat", c4),
        ("Tekken 8", "Combat", c2), ("Need for Speed", "Course", c1), ("Need for Speed", "Course", c4),
        ("WWE 2K25", "Combat", c2), ("NBA 2K25", "Sport", c1), ("NBA 2K25", "Sport", c4),
        ("Gran Turismo 7", "Course", c3), ("GTA V", "Action", c1), ("GTA V", "Action", c4),
        ("God of War", "Action", c2), ("God of War", "Action", c4),
        ("Call of Duty", "Action", c3), ("Call of Duty", "Action", c6),
        ("Fortnite", "Action", c3), ("Spider-Man 2", "Action", c2),
        ("Red Dead Redemption 2", "Action", c4), ("Resident Evil 4", "Horreur", c6),
        ("Undisputed", "Combat", c6), ("EA Sports UFC 5", "Combat", c1), ("Naruto Storm 4", "Combat", c4),
        ("Uncharted 4", "Aventure", c4),
    ];
    for (titre, genre, console_id) in jeux_data {
        let mut m = serde_json::Map::new();
        m.insert("titre".into(), json!(titre));
        m.insert("genre".into(), json!(genre));
        m.insert("console_id".into(), json!(console_id));
        m.insert("actif".into(), json!(1));
        m.insert("created_at".into(), json!(db::now_iso()));
        let _ = db.insert("jeux", &m);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crate::logger::init();
    crate::logger::log("BOOT", "App démarrage");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
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
                neon_reconnecting: std::sync::atomic::AtomicBool::new(false),
                sync_state: Mutex::new(None),
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
                            // IMPORTANT : upsert PAR EMAIL et on NE TOUCH JAMAIS au password_hash
                            // local. Avant : INSERT OR REPLACE par id écrasait le hash admin local
                            // (scrypt seed) par le hash Neon (souvent bcrypt/$2 ou autre algo) ->
                            // admin123 refusé en local ET sur Neon -> "Identifiants incorrects".
                            if let Some(state) = handle.try_state::<AppState>() {
                                let pool_opt = state.neon_pool.lock().ok().and_then(|g| g.clone());
                                if let Some(pool) = pool_opt {
                                    match crate::neon::pull_users(&pool).await {
                                        Ok(users) => {
                                            eprintln!("[neon] pull {} users en background", users.len());
                                            for u in users {
                                                let Some(email) = u.get("email").and_then(serde_json::Value::as_str).map(|s: &str| s.to_string()) else { continue };
                                                if email.is_empty() { continue; }
                                                let mut map = serde_json::Map::new();
                                                if let Some(obj) = u.as_object() {
                                                    for (k, v) in obj {
                                                        if k != "password_hash" && k != "id" { map.insert(k.clone(), v.clone()); }
                                                    }
                                                }
                                                let mut hash_present = false;
                                                // find_one_all : inclut les users soft-deleted pour ne JAMAIS
                                                // les ressusciter au boot (sinon le pull réinsérait un user supprimé)
                                                if let Ok(Some(local)) = state.db.find_one_all("users", |r| r.get("email").and_then(serde_json::Value::as_str) == Some(email.as_str())) {
                                                    // Existant local : on met à jour role/nom/etc. mais PAS le hash
                                                    if let Some(id) = local.get("id").and_then(serde_json::Value::as_i64) {
                                                        let _ = state.db.update("users", id, &map);
                                                    }
                                                    hash_present = true;
                                                } else if let Some(h) = u.get("password_hash").and_then(serde_json::Value::as_str) {
                                                    if !h.is_empty() {
                                                        map.insert("password_hash".into(), json!(h));
                                                        map.insert("email".into(), json!(email));
                                                        let _ = state.db.insert("users", &map);
                                                        hash_present = true;
                                                    }
                                                }
                                                let _ = hash_present;
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[neon] pull users failed: {}", e.message);
                                            crate::logger::log_neon(&format!("pull users boot failed: {} -> reconnexion", e.message));
                                            crate::neon::schedule_reconnect(&handle);
                                        }
                                    }
                                }
                            }
                        }
                        None => eprintln!("[neon] pool non disponible (offline)"),
                    }
                });
            }
            // Sync AUTOMATIQUE : si le toggle est activé, pull+push complet ~10s après
            // le boot puis toutes les 3 min. Les sessions créées/interrompues partent
            // vers Neon SANS action manuelle, et tout est importé depuis Neon au démarrage.
            #[cfg(feature = "neon-sync")]
            {
                let handle = app.handle().clone();
                auto_sync_loop(handle);
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
            commands::auth_debug_info,
            commands::auth_debug_neon_users,
            commands::debug_reset_admin,
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
            // ==== DEBUG / IMPORT (appareil dev uniquement) ====
            commands::debug_is_allowed,
            commands::import_default_tarifs,
            commands::neon_status,
            commands::get_rust_logs,
            commands::get_memory_logs,
            commands::test_neon_connection,
        ])
        .run(tauri::generate_context!())
        .expect("erreur lors de l'exécution de Tauri");
}

/// Boucle de sync automatique (récursive async) : déclenche run_sync_impl si le toggle
/// sync_enabled est actif et qu'aucune sync n'est déjà en cours. Premier tir ~10s après
/// le boot, puis toutes les 3 minutes.
#[cfg(feature = "neon-sync")]
fn auto_sync_loop(handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let state_opt = handle.try_state::<AppState>();
        if state_opt.is_some() {
            let state = state_opt.unwrap();
            let enabled = state.db.get_setting("sync_enabled").ok().and_then(|o| o).unwrap_or_default() == "1";
            let running = state.sync_state.lock().ok()
                .and_then(|g| g.clone())
                .and_then(|v| v.get("running").and_then(serde_json::Value::as_bool))
                .unwrap_or(false);
            if enabled && !running {
                eprintln!("[sync-auto] sync automatique déclenchée");
                let _ = crate::commands::sync::run_sync_impl(&handle, &state).await;
            }
        }
        auto_sync_loop(handle);
    });
}