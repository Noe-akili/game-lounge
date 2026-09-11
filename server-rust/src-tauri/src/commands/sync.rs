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
        let enabled = std::env::var("DATABASE_URL").is_ok() || std::env::var("NEON_DATABASE_URL").is_ok();
        let available = pool_opt.is_some();
        (enabled, available)
    };
    #[cfg(not(feature = "neon-sync"))]
    let (neon_enabled, neon_available) = (false, false);

    Ok(json!({
        "neonEnabled": neon_enabled,
        "neonAvailable": neon_available,
        "hasLocalData": has_local,
        "lastSync": null,
        "mode": if neon_available { "cloud" } else { "offline" }
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
pub async fn sync_run(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
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
            // Pull users depuis Neon vers local (offline-first)
            match crate::neon::pull_users(&pool).await {
                Ok(users) => {
                    eprintln!("[sync] pull {} users depuis Neon", users.len());
                    let db = db(&state);
                    let mut pulled = 0;
                    for u in &users {
                        if let Some(obj) = u.as_object() {
                            let mut map = serde_json::Map::new();
                            for (k,v) in obj { map.insert(k.clone(), v.clone()); }
                            // Upsert local
                            let email = map.get("email").and_then(Value::as_str).unwrap_or("").to_string();
                            match db.find_one("users", |r| r.get("email").and_then(Value::as_str) == Some(email.as_str())) {
                                Ok(Some(existing)) => {
                                    if let Some(id) = existing.get("id").and_then(Value::as_i64) {
                                        let _ = db.update("users", id, &map);
                                        pulled += 1;
                                    }
                                }
                                Ok(None) => {
                                    let _ = db.insert("users", &map);
                                    pulled += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                    return Ok(json!({
                        "success": true,
                        "message": format!("Sync Neon terminée: {} users synchronisés", pulled),
                        "pulled": pulled,
                        "timestamp": now_iso()
                    }));
                }
                Err(e) => {
                    eprintln!("[sync] pull failed: {}", e.message);
                    return Ok(json!({
                        "success": false,
                        "message": format!("Neon sync échouée (offline): {}", e.message),
                        "timestamp": now_iso()
                    }));
                }
            }
        } else {
            eprintln!("[sync] Neon pool non disponible (offline)");
            return Ok(json!({
                "success": true,
                "message": "Mode offline: données locales utilisées (Neon non disponible)",
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
    // Pour l'instant, pas de changes réels, mais structure prête pour push/pull incrémental
    Ok(json!({
        "changes": {},
        "timestamp": now_iso(),
        "neon": {
            "enabled": {
                #[cfg(feature = "neon-sync")]
                { let p = state.neon_pool.lock().ok().and_then(|g| g.clone()); p.is_some() },
                #[cfg(not(feature = "neon-sync"))]
                { false }
            }
        }
    }))
}
