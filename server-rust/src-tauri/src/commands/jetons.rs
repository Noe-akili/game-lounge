// /api/jetons - transactions de jetons de fidélité.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap, row_id, sort_desc_by_created_at};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

/// GET /api/jetons (?joueur_id=)
#[tauri::command]
pub fn jetons_list(
    state: State<'_, AppState>,
    token: Option<String>,
    joueur_id: Option<i64>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let mut transactions: Vec<Value> = db.query_all("jetons_transactions")?;
    if let Some(j) = joueur_id {
        transactions.retain(|t| t.get("joueur_id").and_then(Value::as_i64) == Some(j));
    }
    sort_desc_by_created_at(&mut transactions);
    let joueurs = db.query_all("joueurs")?;
    Ok(Value::Array(
        transactions
            .iter()
            .map(|t| {
                let joueur = t
                    .get("joueur_id")
                    .and_then(Value::as_i64)
                    .and_then(|jid| joueurs.iter().find(|j| row_id(j) == Some(jid)));
                let mut o = t.clone();
                if let Value::Object(map) = &mut o {
                    map.insert(
                        "joueur_nom".into(),
                        joueur
                            .and_then(|j| j.get("nom"))
                            .cloned()
                            .unwrap_or(Value::Null),
                    );
                }
                o
            })
            .collect(),
    ))
}

/// GET /api/jetons/:id
#[tauri::command]
pub fn jetons_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "jetons_transactions", id, "Transaction non trouvée")
}

/// POST /api/jetons (met à jour le solde du joueur)
#[tauri::command]
pub fn jetons_create(
    state: State<'_, AppState>,
    token: Option<String>,
    joueur_id: i64,
    r#type: String,
    quantite: i64,
    raison: Option<String>,
    session_id: Option<i64>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(joueur_id) {
        return Err(ApiError::bad_request("Joueur ID invalide"));
    }
    if !validators::is_valid_jeton_type(&r#type) {
        return Err(ApiError::bad_request("Type invalide"));
    }
    if !validators::is_valid_quantite(quantite) {
        return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
    }
    if let Some(s) = session_id {
        if !validators::is_valid_id(s) {
            return Err(ApiError::bad_request("Session ID invalide"));
        }
    }
    let db = db(&state);
    let joueur = get_by_id(db, "joueurs", joueur_id, "Joueur non trouvé")?;
    let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);

    let mut row = jmap();
    row.insert("joueur_id".into(), json!(joueur_id));
    row.insert("type".into(), json!(r#type.clone()));
    row.insert("quantite".into(), json!(quantite));
    row.insert(
        "raison".into(),
        json!(raison
            .map(|r| validators::sanitize_input(&r, 500))
            .unwrap_or_default()),
    );
    row.insert("session_id".into(), json!(session_id));
    row.insert("created_at".into(), json!(now_iso()));
    let transaction = db.insert("jetons_transactions", &row)?;

    let mut upd = jmap();
    let new_solde = match r#type.as_str() {
        "gain" | "bonus" => solde + quantite,
        "depense" => (solde - quantite).max(0),
        _ => solde,
    };
    upd.insert("jetons_solde".into(), json!(new_solde));
    db.update("joueurs", joueur_id, &upd)?;
    Ok(transaction)
}

/// PUT /api/jetons/:id
#[tauri::command]
pub fn jetons_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    r#type: Option<String>,
    quantite: Option<i64>,
    raison: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "jetons_transactions", id, "Transaction non trouvée")?;
    let mut updates = jmap();
    if let Some(t) = r#type {
        if !validators::is_valid_jeton_type(&t) {
            return Err(ApiError::bad_request("Type invalide"));
        }
        updates.insert("type".into(), json!(t));
    }
    if let Some(q) = quantite {
        if !validators::is_valid_quantite(q) {
            return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
        }
        updates.insert("quantite".into(), json!(q));
    }
    if let Some(r) = raison {
        updates.insert("raison".into(), json!(validators::sanitize_input(&r, 500)));
    }
    db(&state).update("jetons_transactions", id, &updates)
}

/// DELETE /api/jetons/:id
#[tauri::command]
pub fn jetons_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "jetons_transactions", id, "Transaction non trouvée")?;
    db(&state).remove("jetons_transactions", id)?;
    Ok(json!({ "success": true }))
}