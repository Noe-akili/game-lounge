// /api/tarifs - CRUD.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn is_deleted(v: Option<&Value>) -> bool {
    v.map(|x| x.as_bool().unwrap_or(false) || x.as_i64().unwrap_or(0) == 1).unwrap_or(false)
}

/// GET /api/tarifs
#[tauri::command]
pub fn tarifs_list(
    state: State<'_, AppState>,
    token: Option<String>,
    include_deleted: Option<bool>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    // include_deleted=true doit REELLEMENT remonter les archives : query_all()
    // applique deja "WHERE deleted = 0" en SQL, il faut donc query_all_all().
    let mut rows = if include_deleted.unwrap_or(false) {
        db(&state).query_all_all("tarifs")?
    } else {
        db(&state).query_all("tarifs")?
    };
    if !include_deleted.unwrap_or(false) {
        rows.retain(|r| !is_deleted(r.get("deleted")));
    }
    Ok(Value::Array(rows))
}

/// GET /api/tarifs/:id
#[tauri::command]
pub fn tarifs_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "tarifs", id, "Tarif non trouvé")
}

/// POST /api/tarifs
#[tauri::command]
pub fn tarifs_create(
    state: State<'_, AppState>,
    token: Option<String>,
    r#type: String,
    duree_minutes: i64,
    prix: f64,
    description: Option<String>,
    console_type: Option<String>,
    jeu: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if r#type.is_empty() || duree_minutes <= 0 || prix <= 0.0 {
        return Err(ApiError::bad_request("Champs requis manquants"));
    }
    if !validators::is_valid_tarif_type(&r#type) {
        return Err(ApiError::bad_request("Type de tarif invalide"));
    }
    if !validators::is_valid_duree(duree_minutes) {
        return Err(ApiError::bad_request("Durée invalide (1-1000)"));
    }
    if !validators::is_valid_prix(prix) {
        return Err(ApiError::bad_request("Prix invalide (1-1000000)"));
    }
    let mut row = jmap();
    row.insert("type".into(), json!(r#type));
    row.insert("duree_minutes".into(), json!(duree_minutes));
    row.insert("prix".into(), json!(prix));
    row.insert(
        "description".into(),
        json!(description
            .map(|d| validators::sanitize_input(&d, 500))
            .unwrap_or_default()),
    );
    row.insert("actif".into(), json!(1));
    row.insert("console_type".into(), json!(console_type));
    row.insert("jeu".into(), json!(jeu));
    row.insert("created_at".into(), json!(now_iso()));
    db(&state).insert("tarifs", &row)
}

/// PUT /api/tarifs/:id
#[tauri::command]
pub fn tarifs_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    r#type: Option<String>,
    duree_minutes: Option<i64>,
    prix: Option<f64>,
    description: Option<String>,
    actif: Option<bool>,
    console_type: Option<String>,
    jeu: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "tarifs", id, "Tarif non trouvé")?;
    let mut updates = jmap();
    if let Some(t) = r#type {
        if !validators::is_valid_tarif_type(&t) {
            return Err(ApiError::bad_request("Type de tarif invalide"));
        }
        updates.insert("type".into(), json!(t));
    }
    if let Some(d) = duree_minutes {
        if !validators::is_valid_duree(d) {
            return Err(ApiError::bad_request("Durée invalide (1-1000)"));
        }
        updates.insert("duree_minutes".into(), json!(d));
    }
    if let Some(p) = prix {
        if !validators::is_valid_prix(p) {
            return Err(ApiError::bad_request("Prix invalide (1-1000000)"));
        }
        updates.insert("prix".into(), json!(p));
    }
    if let Some(d) = description {
        updates.insert(
            "description".into(),
            json!(if d.is_empty() {
                String::new()
            } else {
                validators::sanitize_input(&d, 500)
            }),
        );
    }
    if let Some(a) = actif {
        updates.insert("actif".into(), json!(if a { 1 } else { 0 }));
    }
    if let Some(ct) = console_type {
        updates.insert("console_type".into(), json!(ct));
    }
    if let Some(j) = jeu {
        updates.insert("jeu".into(), json!(j));
    }
    db(&state).update("tarifs", id, &updates)
}

/// DELETE /api/tarifs/:id
#[tauri::command]
pub fn tarifs_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).remove("tarifs", id)?;
    Ok(json!({ "success": true, "deleted": true }))
}

/// POST /api/tarifs/:id/restore
#[tauri::command]
pub fn tarifs_restore(
    state: State<'_, AppState>, token: Option<String>, id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    let row = db(&state).restore("tarifs", id)?;
    Ok(json!({ "id": id, "restored": true, "tarif": row }))
}

/// DELETE /api/tarifs/:id/permanent
#[tauri::command]
pub fn tarifs_permanent_delete(
    state: State<'_, AppState>, token: Option<String>, id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    db(&state).permanent_delete("tarifs", id)?;
    Ok(json!({ "id": id, "permanently_deleted": true }))
}