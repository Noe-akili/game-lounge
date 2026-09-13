// Commandes de diagnostic (statut cloud, logs, test connexion).
//
// MODE DÉBOGAGE PAR APPAREIL (pas de dossier tarif, pas de données embarquées) :
// l'appareil s'identifie par son `device_id` de sync (ULID stable, déjà persisté
// localement). L'admin autorise explicitement cet identifiant dans Supabase
// (table app_settings, clé "debug_devices", liste JSON d'ULID) ou localement
// pour un test immédiat. Aucune donnée métier n'est créée par ce mode.
use serde_json::{Value, json};
use tauri::State;

use crate::AppState;
use crate::error::ApiResult;

/// Vérifie le statut Supabase (pour le paramètre .env)
#[tauri::command]
pub fn supabase_status(state: State<'_, AppState>) -> ApiResult<Value> {
    #[cfg(feature = "supabase-sync")]
    let (enabled, available, url_present) = {
        let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
        let url_present = std::env::var("DATABASE_URL").map(|u| !u.trim().is_empty()).unwrap_or(false) || std::env::var("SUPABASE_DATABASE_URL").map(|u| !u.trim().is_empty()).unwrap_or(false);
        let enabled = url_present || true; // fallback URL hardcodé donc toujours enabled
        let available = pool_opt.is_some();
        (enabled, available, url_present)
    };
    #[cfg(not(feature = "supabase-sync"))]
    let (enabled, available, url_present) = (false, false, false);

    Ok(json!({
        "supabaseEnabled": enabled,
        "supabaseAvailable": available,
        "urlPresent": url_present,
        "envFile": ".env présent (dotenvy)",
        "fallbackUsed": !url_present
    }))
}

/// Identité de CET appareil : device_id de sync (ULID stable) + statut debug.
/// Le débogage est autorisé si le device_id figure dans app_settings.debug_devices
/// (liste JSON d'ULID, lue en local puis sur Supabase).
#[cfg(feature = "supabase-sync")]
#[tauri::command(async)]
pub async fn device_debug_info(state: State<'_, AppState>) -> ApiResult<Value> {
    let device_id = state.db.device_id().unwrap_or_default();
    let local_allowed = state.db.get_setting("debug_devices").ok().flatten()
        .map(|list| debug_list_contains(&list, &device_id))
        .unwrap_or(false);

    // Vérifie aussi la liste centralisée dans Supabase (admin autorise depuis un autre appareil)
    let (cloud_allowed, cloud_ok) = match cloud_debug_devices(&state).await {
        Some(list) => (debug_list_contains(&list, &device_id), true),
        None => (false, false),
    };
    let allowed = local_allowed || cloud_allowed;
    let hint = if allowed { "" } else { "Ajoutez ce deviceId dans app_settings (clé debug_devices) via Supabase" };

    Ok(json!({
        "deviceId": device_id,
        "debugAllowed": allowed,
        "localList": local_allowed,
        "cloudList": cloud_allowed,
        "cloudReachable": cloud_ok,
        "hint": hint,
    }))
}

/// Identité de l'appareil (variante sans feature sync, build desktop léger)
#[cfg(not(feature = "supabase-sync"))]
#[tauri::command]
pub fn device_debug_info(state: State<'_, AppState>) -> ApiResult<Value> {
    let device_id = state.db.device_id().unwrap_or_default();
    let local_allowed = state.db.get_setting("debug_devices").ok().flatten()
        .map(|list| debug_list_contains(&list, &device_id))
        .unwrap_or(false);
    Ok(json!({
        "deviceId": device_id,
        "debugAllowed": local_allowed,
        "localList": local_allowed,
        "cloudList": false,
        "cloudReachable": false,
        "hint": ""
    }))
}

/// True si la liste JSON contient le device_id (formats acceptés : ["ULID", ...]).
fn debug_list_contains(list_json: &str, device_id: &str) -> bool {
    if list_json.trim().is_empty() || device_id.is_empty() {
        return false;
    }
    match serde_json::from_str::<Value>(list_json) {
        Ok(Value::Array(items)) => items.iter().any(|v| v.as_str() == Some(device_id)),
        _ => false,
    }
}

/// Liste debug_devices depuis Supabase (None si offline/absent).
#[cfg(feature = "supabase-sync")]
async fn cloud_debug_devices(state: &State<'_, AppState>) -> Option<String> {
    let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
    let pool = pool_opt?;
    let rows = crate::supabase::supabase_query(
        &pool,
        "SELECT value FROM app_settings WHERE key = 'debug_devices' LIMIT 1",
        &[],
    ).await.ok()?;
    rows.first().and_then(|r| crate::supabase::pg_col_to_string_pub(r, 0))
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

/// Teste la connexion Supabase et retourne le diagnostic précis où ça bloque
#[tauri::command(async)]
pub async fn test_supabase_connection(app: tauri::AppHandle, state: State<'_, AppState>) -> ApiResult<Value> {
    crate::logger::log("supabase", "test_supabase_connection demandé");
    #[cfg(feature = "supabase-sync")]
    {
        let url = std::env::var("DATABASE_URL").ok().filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("SUPABASE_DATABASE_URL").ok().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| crate::supabase::FALLBACK_URL.to_string());
        crate::logger::log("supabase", &format!("URL len {} chars, host {}", url.len(), url.split('@').last().unwrap_or("").split('/').next().unwrap_or("")));
        // Teste pool existant (ping avec timeout court : 6s max, jamais de hang de plusieurs minutes)
        let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
        if let Some(pool) = pool_opt {
            crate::logger::log("supabase", "pool déjà disponible, test ping SELECT 1 (timeout 6s)");
            match crate::supabase::ping(&pool).await {
                Ok(_) => {
                    crate::logger::log("supabase", "SELECT 1 OK - Supabase joignable");
                    return Ok(json!({"success": true, "message": "Supabase joignable (pool existant)", "url_host": url.split('@').last().unwrap_or("").split('/').next().unwrap_or("").to_string()}));
                }
                Err(e) => {
                    crate::logger::log_cloud_error("SELECT 1 pool existant", &e);
                    // Connexion morte (idle fermée par Supabase) -> reconnexion en arrière-plan
                    crate::supabase::schedule_reconnect(&app);
                    return Ok(json!({"success": false, "message": format!("Pool existant mais connexion morte ({}). Reconnexion en arrière-plan lancée, réessayez dans 10s.", e), "error": e}));
                }
            }
        }
        // Tente nouvelle connexion
        crate::logger::log("supabase", "pool non disponible, tentative init_supabase_pool (timeout 8s)");
        match crate::supabase::init_supabase_pool().await {
            Some(pool) => {
                crate::logger::log("supabase", "init_supabase_pool OK, test query");
                match crate::supabase::ping(&pool).await {
                    Ok(_) => Ok(json!({"success": true, "message": "Supabase connecté avec succès (nouveau pool)", "url_host": url.split('@').last().unwrap_or("").split('/').next().unwrap_or("").to_string()})),
                    Err(e) => {
                        crate::logger::log_cloud_error("SELECT 1 nouveau pool", &e);
                        Ok(json!({"success": false, "message": format!("Nouveau pool mais query failed: {}", e), "error": e}))
                    }
                }
            }
            None => {
                crate::logger::log("supabase", "init_supabase_pool returned None (offline)");
                Ok(json!({"success": false, "message": "Supabase pool non disponible (offline) - vérifiez DATABASE_URL et internet", "url_host": url.split('@').last().unwrap_or("").split('/').next().unwrap_or("").to_string()}))
            }
        }
    }
    #[cfg(not(feature = "supabase-sync"))]
    {
        let _ = (&app, &state);
        Ok(json!({"success": false, "message": "Feature supabase-sync désactivée"}))
    }
}
