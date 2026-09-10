// /api/factures - liste, détail, création, modification, annulation, PDF.

use base64::Engine;
use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap, row_id, sort_desc_by_created_at};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn enrich_facture(f: &Value, joueur: Option<&Value>) -> Value {
    let mut o = f.clone();
    if let Value::Object(map) = &mut o {
        map.insert(
            "joueur_nom".into(),
            joueur
                .and_then(|j| j.get("nom"))
                .cloned()
                .unwrap_or_else(|| json!("N/A")),
        );
    }
    o
}

/// GET /api/factures (?statut=&joueur_id=&date_start=&date_end=)
#[tauri::command]
pub fn factures_list(
    state: State<'_, AppState>,
    token: Option<String>,
    statut: Option<String>,
    joueur_id: Option<i64>,
    date_start: Option<String>,
    date_end: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let mut factures: Vec<Value> = db.query_all("factures")?;
    if let Some(s) = statut {
        factures.retain(|f| f.get("statut").and_then(Value::as_str) == Some(s.as_str()));
    }
    if let Some(j) = joueur_id {
        factures.retain(|f| f.get("joueur_id").and_then(Value::as_i64) == Some(j));
    }
    if let Some(ds) = date_start {
        factures.retain(|f| {
            f.get("created_at")
                .and_then(Value::as_str)
                .map(|c| c >= ds.as_str())
                .unwrap_or(false)
        });
    }
    if let Some(de) = date_end {
        let end = format!("{}T23:59:59", de);
        factures.retain(|f| {
            f.get("created_at")
                .and_then(Value::as_str)
                .map(|c| c <= end.as_str())
                .unwrap_or(false)
        });
    }
    sort_desc_by_created_at(&mut factures);

    let joueurs = db.query_all("joueurs")?;
    Ok(Value::Array(
        factures
            .iter()
            .map(|f| {
                let joueur = f
                    .get("joueur_id")
                    .and_then(Value::as_i64)
                    .and_then(|jid| joueurs.iter().find(|j| row_id(j) == Some(jid)));
                enrich_facture(f, joueur)
            })
            .collect(),
    ))
}

/// GET /api/factures/:id
#[tauri::command]
pub fn factures_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let f = get_by_id(db, "factures", id, "Facture non trouvée")?;
    let joueur = f
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());

    let lignes: Vec<Value> = db
        .query_all("lignes_facture")?
        .into_iter()
        .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id))
        .collect();

    let mut o = enrich_facture(&f, joueur.as_ref());
    if let Value::Object(map) = &mut o {
        map.insert(
            "joueur_telephone".into(),
            joueur
                .as_ref()
                .and_then(|j| j.get("telephone"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert("lignes".into(), json!(lignes));
    }
    Ok(o)
}

/// GET /api/factures/:id/pdf (renvoie le PDF en base64)
#[tauri::command]
pub fn factures_pdf(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let f = get_by_id(db, "factures", id, "Facture non trouvée")?;
    let joueur = f
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
    let lignes: Vec<Value> = db
        .query_all("lignes_facture")?
        .into_iter()
        .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id))
        .collect();

    let pdf = crate::pdf::facture_pdf(&f, joueur.as_ref(), &lignes)
        .map_err(|e| ApiError::internal(format!("Erreur génération PDF: {e}")))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(pdf);
    Ok(json!({ "pdf_base64": b64 }))
}

/// PUT /api/factures/:id/annuler
#[tauri::command]
pub fn factures_annuler(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    get_by_id(db, "factures", id, "Facture non trouvée")?;
    let mut upd = jmap();
    upd.insert("statut".into(), json!("annulee"));
    let updated = db.update("factures", id, &upd)?;
    let joueur = updated
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
    Ok(enrich_facture(&updated, joueur.as_ref()))
}

/// POST /api/factures
#[tauri::command]
pub fn factures_create(
    state: State<'_, AppState>,
    token: Option<String>,
    session_id: i64,
    joueur_id: i64,
    montant_ht: Option<f64>,
    taux_tva: Option<f64>,
    montant_tva: Option<f64>,
    montant_ttc: f64,
    mode_paiement: Option<String>,
    statut: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(session_id) {
        return Err(ApiError::bad_request("Session ID invalide"));
    }
    if !validators::is_valid_id(joueur_id) {
        return Err(ApiError::bad_request("Joueur ID invalide"));
    }
    if !validators::is_valid_prix(montant_ttc) {
        return Err(ApiError::bad_request("Montant TTC invalide (1-1000000)"));
    }
    if let Some(ht) = montant_ht {
        if ht != 0.0 && !validators::is_valid_prix(ht) {
            return Err(ApiError::bad_request("Montant HT invalide"));
        }
    }
    if let Some(s) = statut.as_deref() {
        if !validators::is_valid_facture_statut(s) {
            return Err(ApiError::bad_request("Statut invalide"));
        }
    }
    if let Some(m) = mode_paiement.as_deref() {
        if !validators::is_valid_mode_paiement(m) {
            return Err(ApiError::bad_request("Mode paiement invalide"));
        }
    }
    let random: u32 = rand::random::<u32>() % 10000;
    let now = crate::db::now_iso();
    let mut row = jmap();
    row.insert(
        "numero_facture".into(),
        json!(format!("FAC-{}-{:04}", crate::db::today_str(), random)),
    );
    row.insert("session_id".into(), json!(session_id));
    row.insert("joueur_id".into(), json!(joueur_id));
    row.insert("montant_ht".into(), json!(montant_ht.unwrap_or(0.0)));
    row.insert("taux_tva".into(), json!(taux_tva.unwrap_or(20.0)));
    row.insert("montant_tva".into(), json!(montant_tva.unwrap_or(0.0)));
    row.insert("montant_ttc".into(), json!(montant_ttc));
    row.insert(
        "mode_paiement".into(),
        json!(mode_paiement.unwrap_or_else(|| "especes".into())),
    );
    row.insert("statut".into(), json!(statut.unwrap_or_else(|| "payee".into())));
    row.insert("date_paiement".into(), json!(now.clone()));
    row.insert("created_at".into(), json!(now));
    db(&state).insert("factures", &row)
}

/// PUT /api/factures/:id
#[tauri::command]
pub fn factures_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    statut: Option<String>,
    mode_paiement: Option<String>,
    montant_ttc: Option<f64>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "factures", id, "Facture non trouvée")?;
    let mut updates = jmap();
    if let Some(s) = statut {
        if !validators::is_valid_facture_statut(&s) {
            return Err(ApiError::bad_request("Statut invalide"));
        }
        updates.insert("statut".into(), json!(s));
    }
    if let Some(m) = mode_paiement {
        if !validators::is_valid_mode_paiement(&m) {
            return Err(ApiError::bad_request("Mode paiement invalide"));
        }
        updates.insert("mode_paiement".into(), json!(m));
    }
    if let Some(m) = montant_ttc {
        if !validators::is_valid_prix(m) {
            return Err(ApiError::bad_request("Montant invalide (1-1000000)"));
        }
        updates.insert("montant_ttc".into(), json!(m));
    }
    db(&state).update("factures", id, &updates)
}

/// DELETE /api/factures/:id
#[tauri::command]
pub fn factures_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "factures", id, "Facture non trouvée")?;
    db(&state).remove("factures", id)?;
    Ok(json!({ "success": true }))
}