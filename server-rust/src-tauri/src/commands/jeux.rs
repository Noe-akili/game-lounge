// /api/jeux - CRUD.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

/// GET /api/jeux (?console_id=)
#[tauri::command]
pub fn jeux_list(
    state: State<'_, AppState>,
    token: Option<String>,
    console_id: Option<i64>,
    include_deleted: Option<bool>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let mut jeux = if include_deleted.unwrap_or(false) {
        db(&state).query_all_all("jeux")?
    } else {
        db(&state).query_all("jeux")?
    };
    jeux.retain(|j| j.get("actif").map(|a| !matches!(a, Value::Null)).unwrap_or(false));
    if let Some(cid) = console_id {
        jeux.retain(|j| j.get("console_id").and_then(Value::as_i64) == Some(cid));
    }
    Ok(Value::Array(jeux))
}

/// GET /api/jeux/:id
#[tauri::command]
pub fn jeux_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "jeux", id, "Jeu non trouvé")
}

/// POST /api/jeux
#[tauri::command]
pub fn jeux_create(
    state: State<'_, AppState>,
    token: Option<String>,
    titre: String,
    genre: Option<String>,
    console_id: Option<i64>,
    jaquette_url: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if titre.is_empty() {
        return Err(ApiError::bad_request("Titre requis"));
    }
    if !validators::is_valid_titre(&titre) {
        return Err(ApiError::bad_request("Titre invalide (2-100 caractères)"));
    }
    if let Some(g) = genre.as_deref() {
        if !validators::is_valid_genre(Some(g)) {
            return Err(ApiError::bad_request("Genre invalide (2-50 caractères)"));
        }
    }
    if let Some(cid) = console_id {
        if !validators::is_valid_id(cid) {
            return Err(ApiError::bad_request("Console ID invalide"));
        }
    }
    let mut row = jmap();
    row.insert("titre".into(), json!(validators::sanitize_input(&titre, 100)));
    row.insert(
        "genre".into(),
        json!(genre.as_deref().map(|g| validators::sanitize_input(g, 50))),
    );
    row.insert("console_id".into(), json!(console_id));
    row.insert(
        "jaquette_url".into(),
        json!(jaquette_url
            .as_deref()
            .map(|u| validators::sanitize_input(u, 500))),
    );
    // image_url unifiée (item 6) : même valeur que la jaquette par défaut.
    row.insert(
        "image_url".into(),
        json!(jaquette_url
            .as_deref()
            .map(|u| validators::sanitize_input(u, 500))
            .filter(|u| !u.is_empty())),
    );
    row.insert("actif".into(), json!(1));
    row.insert("created_at".into(), json!(now_iso()));
    db(&state).insert("jeux", &row)
}

/// PUT /api/jeux/:id
#[tauri::command]
pub fn jeux_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    titre: Option<String>,
    genre: Option<String>,
    console_id: Option<Option<i64>>,
    jaquette_url: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "jeux", id, "Jeu non trouvé")?;
    let mut updates = jmap();
    if let Some(t) = titre {
        if !validators::is_valid_titre(&t) {
            return Err(ApiError::bad_request("Titre invalide (2-100 caractères)"));
        }
        updates.insert("titre".into(), json!(validators::sanitize_input(&t, 100)));
    }
    if let Some(g) = genre {
        if !g.is_empty() && !validators::is_valid_genre(Some(&g)) {
            return Err(ApiError::bad_request("Genre invalide"));
        }
        updates.insert(
            "genre".into(),
            if g.is_empty() {
                json!(Value::Null)
            } else {
                json!(validators::sanitize_input(&g, 50))
            },
        );
    }
    if let Some(cid) = console_id {
        if let Some(c) = cid {
            if !validators::is_valid_id(c) {
                return Err(ApiError::bad_request("Console ID invalide"));
            }
        }
        updates.insert("console_id".into(), json!(cid));
    }
    if let Some(u) = jaquette_url {
        updates.insert(
            "jaquette_url".into(),
            if u.is_empty() {
                json!(Value::Null)
            } else {
                json!(validators::sanitize_input(&u, 500))
            },
        );
        // image_url suit la jaquette quand elle n'est pas fournie explicitement.
        if !updates.contains_key("image_url") {
            updates.insert(
                "image_url".into(),
                if u.is_empty() {
                    json!(Value::Null)
                } else {
                    json!(validators::sanitize_input(&u, 500))
                },
            );
        }
    }
    db(&state).update("jeux", id, &updates)
}

/// DELETE /api/jeux/:id
#[tauri::command]
pub fn jeux_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).remove("jeux", id)?;
    Ok(json!({ "success": true }))
}

/// POST /api/jeux/:id/restore
#[tauri::command]
pub fn jeux_restore(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).restore("jeux", id)
}

/// DELETE /api/jeux/:id/permanent
#[tauri::command]
pub fn jeux_permanent_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).permanent_delete("jeux", id)?;
    Ok(json!({ "success": true, "permanently_deleted": true }))
}
