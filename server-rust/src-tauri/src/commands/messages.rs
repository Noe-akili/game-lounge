// /api/messages - CRUD (port sans Neon : stockage 100% local).

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{claims, db, get_by_id, jmap};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

/// GET /api/messages (triés par created_at décroissant)
#[tauri::command]
pub fn messages_list(
    state: State<'_, AppState>,
    token: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let mut messages = db.query_all("messages")?;
    messages.sort_by(|a, b| {
        let ca = a.get("created_at").and_then(Value::as_str).unwrap_or("");
        let cb = b.get("created_at").and_then(Value::as_str).unwrap_or("");
        cb.cmp(ca)
    });
    Ok(Value::Array(messages))
}

/// POST /api/messages
#[tauri::command]
pub fn messages_create(
    state: State<'_, AppState>,
    token: Option<String>,
    titre: Option<String>,
    contenu: String,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    if contenu.trim().is_empty() {
        return Err(ApiError::bad_request("Contenu requis"));
    }
    if !validators::is_valid_contenu(&contenu) {
        return Err(ApiError::bad_request("Contenu invalide (1-1000 caractères)"));
    }
    let mut row = jmap();
    if let Some(t) = titre {
        row.insert("titre".into(), json!(validators::sanitize_input(&t, 100)));
    } else {
        row.insert("titre".into(), json!(Value::Null));
    }
    let contenu = contenu.trim().chars().take(1000).collect::<String>();
    row.insert("contenu".into(), json!(contenu));
    row.insert("auteur".into(), json!(user.nom));
    row.insert("created_at".into(), json!(now_iso()));
    db(&state).insert("messages", &row)
}

/// PUT /api/messages/:id
#[tauri::command]
pub fn messages_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    titre: Option<String>,
    contenu: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "messages", id, "Message non trouvé")?;
    let mut updates = jmap();
    if let Some(t) = titre {
        updates.insert("titre".into(), json!(validators::sanitize_input(&t, 100)));
    }
    if let Some(c) = contenu {
        updates.insert(
            "contenu".into(),
            json!(c.trim().chars().take(1000).collect::<String>()),
        );
    }
    db(&state).update("messages", id, &updates)
}

/// DELETE /api/messages/:id
#[tauri::command]
pub fn messages_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).remove("messages", id)?;
    Ok(json!({ "success": true }))
}