// /api/lignes_facture - CRUD des lignes de facture.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{claims, db, get_by_id, jmap};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

/// GET /api/lignes_facture (?facture_id=)
#[tauri::command]
pub fn lignes_list(
    state: State<'_, AppState>,
    token: Option<String>,
    facture_id: Option<i64>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let mut lignes = db(&state).query_all("lignes_facture")?;
    if let Some(fid) = facture_id {
        lignes.retain(|l| l.get("facture_id").and_then(Value::as_i64) == Some(fid));
    }
    Ok(Value::Array(lignes))
}

/// GET /api/lignes_facture/:id
#[tauri::command]
pub fn lignes_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "lignes_facture", id, "Ligne non trouvée")
}

/// POST /api/lignes_facture
#[tauri::command]
pub fn lignes_create(
    state: State<'_, AppState>,
    token: Option<String>,
    facture_id: i64,
    description: String,
    quantite: i64,
    prix_unitaire: f64,
    total_ligne: Option<f64>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(facture_id) {
        return Err(ApiError::bad_request("Facture ID invalide"));
    }
    if !validators::is_valid_description(&description) {
        return Err(ApiError::bad_request("Description invalide (2-500 caractères)"));
    }
    if !validators::is_valid_quantite(quantite) {
        return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
    }
    if !validators::is_valid_prix(prix_unitaire) {
        return Err(ApiError::bad_request("Prix unitaire invalide (1-1000000)"));
    }
    if let Some(t) = total_ligne {
        if !validators::is_valid_prix(t) {
            return Err(ApiError::bad_request("Total invalide"));
        }
    }
    let mut row = jmap();
    row.insert("facture_id".into(), json!(facture_id));
    row.insert("description".into(), json!(validators::sanitize_input(&description, 500)));
    row.insert("quantite".into(), json!(quantite));
    row.insert("prix_unitaire".into(), json!(prix_unitaire));
    row.insert(
        "total_ligne".into(),
        json!(total_ligne.unwrap_or(prix_unitaire * quantite as f64)),
    );
    db(&state).insert("lignes_facture", &row)
}

/// PUT /api/lignes_facture/:id
#[tauri::command]
pub fn lignes_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    description: Option<String>,
    quantite: Option<i64>,
    prix_unitaire: Option<f64>,
    total_ligne: Option<f64>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "lignes_facture", id, "Ligne non trouvée")?;
    let mut updates = jmap();
    if let Some(d) = description {
        if !validators::is_valid_description(&d) {
            return Err(ApiError::bad_request("Description invalide (2-500 caractères)"));
        }
        updates.insert("description".into(), json!(validators::sanitize_input(&d, 500)));
    }
    if let Some(q) = quantite {
        if !validators::is_valid_quantite(q) {
            return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
        }
        updates.insert("quantite".into(), json!(q));
    }
    if let Some(p) = prix_unitaire {
        if !validators::is_valid_prix(p) {
            return Err(ApiError::bad_request("Prix unitaire invalide (1-1000000)"));
        }
        updates.insert("prix_unitaire".into(), json!(p));
    }
    if let Some(t) = total_ligne {
        if !validators::is_valid_prix(t) {
            return Err(ApiError::bad_request("Total invalide"));
        }
        updates.insert("total_ligne".into(), json!(t));
    }
    db(&state).update("lignes_facture", id, &updates)
}

/// DELETE /api/lignes_facture/:id
#[tauri::command]
pub fn lignes_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "lignes_facture", id, "Ligne non trouvée")?;
    db(&state).remove("lignes_facture", id)?;
    Ok(json!({ "success": true }))
}