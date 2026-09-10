// /api/sync - statut de synchronisation.
// Version Rust : 100% local (Neon/SQL cloud non supporté pour garder l'app légère).

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db};
use crate::db::now_iso;
use crate::error::ApiResult;
use crate::AppState;

fn local_status(state: &State<'_, AppState>) -> ApiResult<Value> {
    let has_local = !db(state).query_all("users")?.is_empty();
    Ok(json!({
        "neonEnabled": false,
        "neonAvailable": false,
        "hasLocalData": has_local,
    }))
}

/// GET /api/sync/status
#[tauri::command]
pub fn sync_status(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    local_status(&state)
}

/// POST /api/sync/toggle (accepté pour compatibilité, sans effet)
#[tauri::command]
pub fn sync_toggle(
    state: State<'_, AppState>,
    token: Option<String>,
    enabled: bool,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let base = local_status(&state)?;
    let mut o = base;
    if let Value::Object(map) = &mut o {
        map.insert("enabled".into(), json!(enabled));
    }
    Ok(o)
}

/// POST /api/sync/run (synchro locale uniquement)
#[tauri::command]
pub fn sync_run(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let _ = db(&state).query_all("users")?;
    Ok(json!({
        "success": true,
        "message": "Synchronisation locale terminée (Neon non configuré)"
    }))
}

/// GET /api/sync/poll
#[tauri::command]
pub fn sync_poll(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    Ok(json!({
        "changes": {},
        "timestamp": now_iso(),
    }))
}