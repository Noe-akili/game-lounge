mod auth;
mod commands;
mod db;
mod error;
mod pdf;
mod validators;

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
}

fn open_db(app: &tauri::AppHandle) -> Result<Db, Box<dyn std::error::Error>> {
    let mut dir: Option<std::path::PathBuf> = std::env::var("DATADIR").ok().map(std::path::PathBuf::from);
    if dir.is_none() {
        dir = Some(
            app.path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
    }
    let path = dir.as_ref().unwrap();
    std::fs::create_dir_all(path)?;
    let db_path = path.join("gamelounge.db");
    let db = Db::open(&db_path)?;
    seed_default_users(&db)?;
    Ok(db)
}

/// À la première exécution (base vide), crée les comptes par défaut,
/// identiques au seed du backend Node (server/seed.ts) :
///   admin@gamelounge.com / admin123   (rôle admin)
///   john@gamelounge.com  / employe123 (rôle employé)
fn seed_default_users(db: &Db) -> Result<(), Box<dyn std::error::Error>> {
    if !db.query_all("users")?.is_empty() {
        return Ok(());
    }
    let now = db::now_iso();

    let mut admin = serde_json::Map::new();
    admin.insert("email".into(), serde_json::json!("admin@gamelounge.com"));
    admin.insert(
        "password_hash".into(),
        serde_json::json!(auth::hash_password("admin123")?),
    );
    admin.insert("role".into(), serde_json::json!("admin"));
    admin.insert("nom".into(), serde_json::json!("Admin"));
    admin.insert("created_at".into(), serde_json::json!(now.clone()));
    db.insert("users", &admin)?;

    let mut emp = serde_json::Map::new();
    emp.insert("email".into(), serde_json::json!("john@gamelounge.com"));
    emp.insert(
        "password_hash".into(),
        serde_json::json!(auth::hash_password("employe123")?),
    );
    emp.insert("role".into(), serde_json::json!("employe"));
    emp.insert("nom".into(), serde_json::json!("John Doe"));
    emp.insert("created_at".into(), serde_json::json!(now));
    db.insert("users", &emp)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            match open_db(handle) {
                Ok(db) => {
                    let jwt_secret =
                        std::env::var("JWT_SECRET").unwrap_or_else(|_| "game-lounge-secret-2024".into());
                    app.manage(AppState {
                        db,
                        jwt_secret,
                        login_attempts: Mutex::new(std::collections::HashMap::new()),
                    });
                }
                Err(e) => {
                    eprintln!("DB init error: {e}");
                    let handle = app.handle();
                    let path = handle
                        .path()
                        .app_data_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."));
                    let db_path = path.join("gamelounge.db");
                    match Db::open(&db_path) {
                        Ok(db) => {
                            eprintln!("Recovered with fresh DB at {db_path:?}");
                            let jwt_secret =
                                std::env::var("JWT_SECRET").unwrap_or_else(|_| "game-lounge-secret-2024".into());
                            app.manage(AppState {
                                db,
                                jwt_secret,
                                login_attempts: Mutex::new(std::collections::HashMap::new()),
                            });
                        }
                        Err(e2) => {
                            eprintln!("Fatal: cannot open DB: {e2}");
                        }
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ==== SANTÉ ====
            commands::health,
            // ==== AUTH ====
            commands::auth_login,
            commands::auth_logout,
            commands::auth_me,
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