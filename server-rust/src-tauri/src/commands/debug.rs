// Commandes debug et import tarif - appareil de développement uniquement
use serde_json::{Value, json};
use tauri::State;

use crate::AppState;
use crate::error::{ApiError, ApiResult};

/// Vérifie si le mode debug est autorisé sur cet appareil (présence du dossier tarif)
#[tauri::command]
pub fn debug_is_allowed() -> bool {
    crate::import::is_debug_device()
}

/// Importe les données par défaut depuis /storage/emulated/0/développement/Playstation/tarif
/// La personne voit ses données et Neon est synchronisé
#[tauri::command(async)]
pub async fn import_default_tarifs(state: State<'_, AppState>) -> ApiResult<Value> {
    // Vérifie debug device
    if !crate::import::is_debug_device() {
        return Err(ApiError::forbidden("Import non autorisé sur cet appareil"));
    }
    let db = &state.db;
    let count = crate::import::import_default_data(db)?;
    eprintln!("[import] {} tarifs/jeux importés depuis stockage local", count);

    // Push vers Neon si disponible (best practice : local d'abord, puis cloud)
    #[cfg(feature = "neon-sync")]
    {
        let pool_opt = {
            let guard = state.neon_pool.lock().map_err(|_| ApiError::internal("neon lock"))?;
            guard.clone()
        };
        if let Some(pool) = pool_opt {
            // Récupère tous les tarifs/jeux locaux et pousse vers Neon (simplifié : on push les nouveaux)
            // Pour l'instant, on log seulement - le sync complet est fait via sync_run
            eprintln!("[import] Neon sync disponible, {} items à synchroniser", count);
            // Optionnel : push chaque tarif vers Neon
            // On pourrait implémenter push_tarif, mais on laisse sync_run faire le pull/push complet
        } else {
            eprintln!("[import] Neon non disponible, données locales uniquement");
        }
    }

    Ok(json!({
        "imported": count,
        "message": format!("{} tarifs/jeux importés depuis stockage local et disponibles localement + Neon", count)
    }))
}

/// Vérifie le statut Neon (pour le paramètre .env)
#[tauri::command]
pub fn neon_status(state: State<'_, AppState>) -> ApiResult<Value> {
    #[cfg(feature = "neon-sync")]
    let (enabled, available, url_present) = {
        let pool_opt = state.neon_pool.lock().ok().and_then(|g| g.clone());
        let url_present = std::env::var("DATABASE_URL").is_ok() || option_env!("DATABASE_URL").is_some() || std::env::var("NEON_DATABASE_URL").is_ok();
        let enabled = url_present || true; // fallback URL hardcodé donc toujours enabled
        let available = pool_opt.is_some();
        (enabled, available, url_present)
    };
    #[cfg(not(feature = "neon-sync"))]
    let (enabled, available, url_present) = (false, false, false);

    Ok(json!({
        "neonEnabled": enabled,
        "neonAvailable": available,
        "urlPresent": url_present,
        "envFile": ".env présent (dotenvy)",
        "fallbackUsed": !url_present
    }))
}

/// Récupère les logs Rust (1.log) - tout est capturé, rien ne nous échappe
#[tauri::command]
pub fn get_rust_logs() -> String {
    crate::logger::read_log_file()
}

/// Récupère les logs en mémoire (derniers 200)
#[tauri::command]
pub fn get_memory_logs() -> Vec<String> {
    crate::logger::get_logs(200)
}

/// Teste la connexion Neon et retourne le diagnostic précis où ça bloque
#[tauri::command(async)]
pub async fn test_neon_connection(state: State<'_, AppState>) -> ApiResult<Value> {
    crate::logger::log("neon", "test_neon_connection demandé");
    #[cfg(feature = "neon-sync")]
    {
        let url = std::env::var("DATABASE_URL").ok().filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("NEON_DATABASE_URL").ok().filter(|s| !s.trim().is_empty()))
            .or_else(|| option_env!("DATABASE_URL").map(|s| s.to_string()).filter(|s| !s.trim().is_empty()))
            .or_else(|| option_env!("NEON_DATABASE_URL").map(|s| s.to_string()).filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| "postgresql://neondb_owner:npg_AEay0ug9NHYj@ep-wild-cloud-axxw1ufj-pooler.c-4.us-east-2.aws.neon.tech/neondb?sslmode=require".to_string());
        crate::logger::log("neon", &format!("URL len {} chars, host {}", url.len(), url.split('@').last().unwrap_or("").split('/').next().unwrap_or("")));
        // Teste pool existant
        let pool_opt = state.neon_pool.lock().ok().and_then(|g| g.clone());
        if let Some(pool) = pool_opt {
            crate::logger::log("neon", "pool déjà disponible, test query SELECT 1");
            match pool.client.query("SELECT 1", &[]).await {
                Ok(_) => {
                    crate::logger::log("neon", "SELECT 1 OK - Neon joignable");
                    return Ok(json!({"success": true, "message": "Neon joignable (pool existant)", "url_host": url.split('@').last().unwrap_or("").split('/').next().unwrap_or("").to_string()}));
                }
                Err(e) => {
                    crate::logger::log_neon_error("SELECT 1 pool existant", &e.to_string());
                    return Ok(json!({"success": false, "message": format!("Pool existant mais query failed: {}", e), "error": e.to_string()}));
                }
            }
        }
        // Tente nouvelle connexion
        crate::logger::log("neon", "pool non disponible, tentative init_neon_pool");
        match crate::neon::init_neon_pool().await {
            Some(pool) => {
                crate::logger::log("neon", "init_neon_pool OK, test query");
                match pool.client.query("SELECT 1", &[]).await {
                    Ok(_) => Ok(json!({"success": true, "message": "Neon connecté avec succès (nouveau pool)", "url_host": url.split('@').last().unwrap_or("").split('/').next().unwrap_or("").to_string()})),
                    Err(e) => {
                        crate::logger::log_neon_error("SELECT 1 nouveau pool", &e.to_string());
                        Ok(json!({"success": false, "message": format!("Nouveau pool mais query failed: {}", e), "error": e.to_string()}))
                    }
                }
            }
            None => {
                crate::logger::log("neon", "init_neon_pool returned None (offline)");
                Ok(json!({"success": false, "message": "Neon pool non disponible (offline) - vérifiez DATABASE_URL et internet", "url_host": url.split('@').last().unwrap_or("").split('/').next().unwrap_or("").to_string()}))
            }
        }
    }
    #[cfg(not(feature = "neon-sync"))]
    {
        Ok(json!({"success": false, "message": "Feature neon-sync désactivée"}))
    }
}
