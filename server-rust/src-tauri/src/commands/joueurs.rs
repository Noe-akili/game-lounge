// /api/joueurs - CRUD + historique.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{claims, db, get_by_id, jmap, row_id, sort_desc_by_created_at};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

/// GET /api/joueurs (?search=)
#[tauri::command]
pub fn joueurs_list(
    state: State<'_, AppState>,
    token: Option<String>,
    search: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let mut joueurs = db(&state).query_all("joueurs")?;
    if let Some(q) = search {
        let q = q.to_lowercase();
        joueurs.retain(|j| {
            let nom = j.get("nom").and_then(Value::as_str).unwrap_or("").to_lowercase();
            let tel = j.get("telephone").and_then(Value::as_str).unwrap_or("");
            let email = j.get("email").and_then(Value::as_str).unwrap_or("").to_lowercase();
            nom.contains(&q) || tel.contains(&q) || email.contains(&q)
        });
    }
    Ok(Value::Array(joueurs))
}

/// GET /api/joueurs/:id
#[tauri::command]
pub fn joueurs_get(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "joueurs", id, "Joueur non trouvé")
}

/// POST /api/joueurs
#[tauri::command]
pub fn joueurs_create(
    state: State<'_, AppState>,
    token: Option<String>,
    nom: String,
    telephone: Option<String>,
    email: Option<String>,
    sticker: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if nom.is_empty() {
        return Err(ApiError::bad_request("Nom requis"));
    }
    if !validators::is_valid_nom(&nom) {
        return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
    }
    if let Some(t) = telephone.as_deref() {
        if !t.is_empty() && !validators::is_valid_phone(t) {
            return Err(ApiError::bad_request(
                "Téléphone invalide (8-15 chiffres, ex: +243...)",
            ));
        }
    }
    if let Some(e) = email.as_deref() {
        if !e.is_empty() && !validators::is_valid_email(e) {
            return Err(ApiError::bad_request("Email invalide"));
        }
    }
    let mut row = jmap();
    row.insert("nom".into(), json!(validators::sanitize_input(&nom, 50)));
    row.insert(
        "telephone".into(),
        json!(telephone
            .map(|t| t.replace([' ', '-'], ""))
            .unwrap_or_default()),
    );
    row.insert(
        "email".into(),
        json!(email.map(|e| validators::sanitize_input(&e, 100)).unwrap_or_default()),
    );
    row.insert("jetons_solde".into(), json!(0));
    row.insert("date_inscription".into(), json!(now_iso()));
    row.insert("derniere_visite".into(), json!(Value::Null));
    // Sticker/icône du joueur (emoji ou URL courte) — affiché sur les cartes.
    row.insert(
        "sticker".into(),
        json!(sticker.map(|s| validators::sanitize_input(&s, 16)).filter(|s| !s.is_empty())),
    );
    db(&state).insert("joueurs", &row)
}

/// PUT /api/joueurs/:id
#[tauri::command]
pub fn joueurs_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    nom: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    jetons_solde: Option<Option<i64>>,
    sticker: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "joueurs", id, "Joueur non trouvé")?;
    let mut updates = jmap();
    if let Some(n) = nom {
        if !validators::is_valid_nom(&n) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        updates.insert("nom".into(), json!(validators::sanitize_input(&n, 50)));
    }
    if let Some(t) = telephone {
        if !t.is_empty() && !validators::is_valid_phone(&t) {
            return Err(ApiError::bad_request(
                "Téléphone invalide (8-15 chiffres, ex: +243...)",
            ));
        }
        updates.insert(
            "telephone".into(),
            json!(t.replace([' ', '-'], "")),
        );
    }
    if let Some(e) = email {
        if !e.is_empty() && !validators::is_valid_email(&e) {
            return Err(ApiError::bad_request("Email invalide"));
        }
        updates.insert(
            "email".into(),
            json!(if e.is_empty() {
                String::new()
            } else {
                validators::sanitize_input(&e, 100)
            }),
        );
    }
    if let Some(js) = jetons_solde {
        if let Some(js) = js {
            if js < 0 {
                return Err(ApiError::bad_request("Jetons invalides"));
            }
            updates.insert("jetons_solde".into(), json!(js));
        }
    }
    // Sticker : chaîne vide = retirer le visuel.
    if let Some(s) = sticker {
        updates.insert(
            "sticker".into(),
            json!(if s.is_empty() {
                Value::Null
            } else {
                json!(validators::sanitize_input(&s, 16))
            }),
        );
    }
    db(&state).update("joueurs", id, &updates)
}

/// GET /api/joueurs/:id/historique
#[tauri::command]
pub fn joueurs_historique(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let joueur = get_by_id(db, "joueurs", id, "Joueur non trouvé")?;

    let mut sessions: Vec<Value> = db
        .query_all("sessions_jeu")?
        .into_iter()
        .filter(|s| s.get("joueur_id").and_then(Value::as_i64) == Some(id))
        .collect();
    sort_desc_by_created_at(&mut sessions);

    let mut transactions: Vec<Value> = db
        .query_all("jetons_transactions")?
        .into_iter()
        .filter(|t| t.get("joueur_id").and_then(Value::as_i64) == Some(id))
        .collect();
    sort_desc_by_created_at(&mut transactions);

    let mut factures: Vec<Value> = db
        .query_all("factures")?
        .into_iter()
        .filter(|f| f.get("joueur_id").and_then(Value::as_i64) == Some(id))
        .collect();
    sort_desc_by_created_at(&mut factures);

    let consoles = db.query_all("consoles")?;
    let jeux = db.query_all("jeux")?;

    let enriched_sessions: Vec<Value> = sessions
        .iter()
        .map(|s| {
            let console_id = s.get("console_id").and_then(Value::as_i64);
            let jeu_id = s.get("jeu_id").and_then(Value::as_i64);
            let mut o = s.clone();
            if let Value::Object(map) = &mut o {
                match consoles.iter().find(|c| row_id(c) == console_id) {
                    Some(c) => {
                        map.insert("console_nom".into(), c.get("nom").cloned().unwrap_or(Value::Null))
                    }
                    None => map.insert("console_nom".into(), Value::Null),
                };
                match jeux.iter().find(|j| row_id(j) == jeu_id) {
                    Some(j) => {
                        map.insert("jeu_nom".into(), j.get("titre").cloned().unwrap_or(Value::Null))
                    }
                    None => map.insert("jeu_nom".into(), Value::Null),
                };
            }
            o
        })
        .collect();

    Ok(json!({
        "joueur": joueur,
        "sessions": enriched_sessions,
        "transactions": transactions,
        "factures": factures,
    }))
}

/// DELETE /api/joueurs/:id
#[tauri::command]
pub fn joueurs_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "joueurs", id, "Joueur non trouvé")?;
    db(&state).remove("joueurs", id)?;
    Ok(json!({ "success": true }))
}