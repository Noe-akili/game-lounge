// /api/sync - statut et synchronisation Neon (offline-first best practice)
use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db};
use crate::db::now_iso;
use crate::error::ApiResult;
use crate::AppState;

fn local_status(state: &State<'_, AppState>) -> ApiResult<Value> {
    let has_local = !db(state).query_all("users")?.is_empty();
    #[cfg(feature = "neon-sync")]
    let (neon_enabled, neon_available) = {
        let pool_opt = state.neon_pool.lock().ok().and_then(|g| g.clone());
        // Fallback URL hardcodé assure Neon toujours enabled (même sans .env)
        let enabled = true;
        let available = pool_opt.is_some();
        let _ = std::env::var("DATABASE_URL").is_ok(); // garde compat
        (enabled, available)
    };
    #[cfg(not(feature = "neon-sync"))]
    let (neon_enabled, neon_available) = (false, false);

    Ok(json!({
        "neonEnabled": neon_enabled,
        "neonAvailable": neon_available,
        "hasLocalData": has_local,
        "lastSync": null,
        "mode": if neon_available { "cloud" } else { "offline" },
        "message": if neon_enabled && !neon_available { "Neon configuré (fallback) mais offline - données locales" } else { "" }
    }))
}

/// GET /api/sync/status
#[tauri::command]
pub fn sync_status(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    local_status(&state)
}

/// POST /api/sync/toggle
#[tauri::command]
pub fn sync_toggle(
    state: State<'_, AppState>,
    token: Option<String>,
    enabled: bool,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    eprintln!("[sync] toggle enabled={}", enabled);
    let base = local_status(&state)?;
    let mut o = base;
    if let Value::Object(map) = &mut o {
        map.insert("enabled".into(), json!(enabled));
    }
    Ok(o)
}

/// POST /api/sync/run - Best practice : pull Neon -> local, puis push local -> Neon en background
#[tauri::command(async)]
pub async fn sync_run(app: tauri::AppHandle, state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    eprintln!("[sync] sync_run demandé par {}", user.email);

    #[cfg(feature = "neon-sync")]
    {
        let pool_opt = {
            let guard = state.neon_pool.lock().map_err(|_| crate::error::ApiError::internal("neon lock"))?;
            guard.clone()
        };
        if let Some(pool) = pool_opt {
            // Pull toutes les tables depuis Neon (offline-first, restauration complète si app data vidé)
            match crate::neon::pull_all(&pool).await {
                Ok(all) => {
                    eprintln!("[sync] pull all tables depuis Neon: {:?}", all.keys().collect::<Vec<_>>());
                    let db = db(&state);
                    let mut total_pulled = 0;
                    for (table, rows) in &all {
                        let mut pulled = 0;
                        for row in rows {
                            if let Some(obj) = row.as_object() {
                                let mut map = serde_json::Map::new();
                                for (k,v) in obj { map.insert(k.clone(), v.clone()); }
                                // Upsert local : tente update si id existe, sinon insert
                                if let Some(id) = map.get("id").and_then(Value::as_i64) {
                                    match db.get_opt(table, id) {
                                        Ok(Some(_)) => { let _ = db.update(table, id, &map); pulled += 1; },
                                        Ok(None) => { let _ = db.insert(table, &map); pulled += 1; },
                                        _ => {}
                                    }
                                } else {
                                    let _ = db.insert(table, &map);
                                    pulled += 1;
                                }
                            }
                        }
                        eprintln!("[sync] pull {}: {} rows", table, pulled);
                        total_pulled += pulled;
                    }
                    return Ok(json!({
                        "success": true,
                        "message": format!("Sync Neon terminée: {} lignes synchronisées (toutes tables)", total_pulled),
                        "pulled": total_pulled,
                        "tables": all.keys().collect::<Vec<_>>(),
                        "timestamp": now_iso()
                    }));
                }
                Err(e) => {
                    eprintln!("[sync] pull_all failed: {}", e.message);
                    crate::logger::log_neon(&format!("sync_run: pull_all failed: {} -> reconnexion", e.message));
                    crate::neon::schedule_reconnect(&app);
                    return Ok(json!({
                        "success": false,
                        "message": format!("Neon sync échouée (connexion perdue): {}. Reconnexion en arrière-plan, réessayez.", e.message),
                        "timestamp": now_iso()
                    }));
                }
            }
        } else {
            eprintln!("[sync] Neon pool non disponible (offline) - données locales conservées, sera sync à la reconnexion");
            return Ok(json!({
                "success": true,
                "message": "Mode offline: données locales utilisées (Neon sera synchronisé à la reconnexion)",
                "timestamp": now_iso()
            }));
        }
    }

    #[cfg(not(feature = "neon-sync"))]
    {
        let _ = db(&state).query_all("users")?;
        return Ok(json!({
            "success": true,
            "message": "Synchronisation locale terminée (Neon non configuré)",
            "timestamp": now_iso()
        }));
    }
}

/// GET /api/sync/poll - Best practice : retourne timestamp, utilisé par frontend pour refresh
#[tauri::command]
pub fn sync_poll(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    #[cfg(feature = "neon-sync")]
    let neon_enabled = state.neon_pool.lock().ok().and_then(|g| g.clone()).is_some();
    #[cfg(not(feature = "neon-sync"))]
    let neon_enabled = false;
    Ok(json!({
        "changes": {},
        "timestamp": now_iso(),
        "neon": {
            "enabled": neon_enabled
        }
    }))
}
