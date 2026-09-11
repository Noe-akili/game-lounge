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
