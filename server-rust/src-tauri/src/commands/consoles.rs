// /api/consoles - CRUD + liste enrichie avec la session en cours.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap, row_id};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn is_del(v: Option<&Value>) -> bool {
    v.map(|val| val.as_bool().unwrap_or(false) || val.as_i64().unwrap_or(0) == 1).unwrap_or(false)
}

fn enrich(
    consoles: &[Value],
    sessions: &[Value],
    joueurs: &[Value],
    jeux: &[Value],
) -> Vec<Value> {
    consoles
        .iter()
        .map(|c| {
            let session = sessions
                .iter()
                .find(|s| s.get("console_id").and_then(Value::as_i64) == c.get("id").and_then(Value::as_i64));
            let joueur_id = session.and_then(|s| s.get("joueur_id").and_then(Value::as_i64));
            let jeu_id = session.and_then(|s| s.get("jeu_id").and_then(Value::as_i64));
            let deleted = is_del(c.get("deleted"));
            json!({
                "id": c["id"],
                "nom": c["nom"],
                "type": c["type"],
                "etat": c["etat"],
                "poste_numero": c["poste_numero"],
                "date_ajout": c["date_ajout"],
                "image_url": c.get("image_url").cloned().unwrap_or(Value::Null),
                "deleted": deleted,
                "session_id": session.and_then(|s| s.get("id")).cloned().unwrap_or(Value::Null),
                "session_statut": session.and_then(|s| s.get("statut")).cloned().unwrap_or(Value::Null),
                "session_debut": session.and_then(|s| s.get("debut")).cloned().unwrap_or(Value::Null),
                "duree_allouee": session.and_then(|s| s.get("duree_allouee")).cloned().unwrap_or(Value::Null),
                "duree_minutes": session.and_then(|s| s.get("duree_minutes")).cloned().unwrap_or(Value::Null),
                "duree_secondes": session.and_then(|s| s.get("duree_secondes")).cloned().unwrap_or(Value::Null),
                "joueur_id": joueur_id.map(Value::from).unwrap_or(Value::Null),
                "jeu_id": jeu_id.map(Value::from).unwrap_or(Value::Null),
                "tarif_prix": session.and_then(|s| s.get("tarif_prix")).cloned().unwrap_or(Value::Null),
                "joueur_nom": joueurs.iter().find(|j| row_id(j) == joueur_id).and_then(|j| j.get("nom")).cloned().unwrap_or(Value::Null),
                "jeu_nom": jeux.iter().find(|j| row_id(j) == jeu_id).and_then(|j| j.get("titre")).cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

/// GET /api/consoles
#[tauri::command]
pub async fn consoles_list(state: State<'_, AppState>, token: Option<String>, include_deleted: Option<bool>) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let include_del = include_deleted.unwrap_or(false);
    let consoles = if include_del {
        // Exiger obligatoirement la connexion Internet et Supabase
        let pool = crate::supabase::get_supabase_pool(&state).await
            .map_err(|_| ApiError::network("Connexion Internet requise : les archives sont stockées exclusivement sur Supabase."))?;
        crate::supabase::pull_table_all(&pool, "consoles").await
            .map_err(|e| ApiError::network(format!("Connexion Internet requise pour charger les archives : {e}")))?
    } else {
        db.query_all("consoles")?
    };
    let sessions: Vec<Value> = db
        .query_all("sessions_jeu")?
        .into_iter()
        .filter(|s| {
            let st = s.get("statut").and_then(Value::as_str).unwrap_or("");
            st == "en_cours" || st == "pause"
        })
        .collect();
    let joueurs = db.query_all("joueurs")?;
    let jeux = db.query_all("jeux")?;

    let mut result = enrich(&consoles, &sessions, &joueurs, &jeux);
    result.sort_by(|a, b| {
        a.get("poste_numero")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .cmp(&b.get("poste_numero").and_then(Value::as_i64).unwrap_or(0))
    });
    Ok(Value::Array(result))
}

/// GET /api/consoles/:id
#[tauri::command]
pub fn consoles_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "consoles", id, "Console non trouvée")
}

/// POST /api/consoles
#[tauri::command]
pub fn consoles_create(
    state: State<'_, AppState>,
    token: Option<String>,
    nom: String,
    r#type: String,
    poste_numero: i64,
    etat: Option<String>,
    image_url: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if nom.is_empty() || r#type.is_empty() {
        return Err(ApiError::bad_request("Champs requis manquants"));
    }
    if !validators::is_valid_nom(&nom) {
        return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
    }
    if !validators::is_valid_console_type(&r#type) {
        return Err(ApiError::bad_request("Type de console invalide"));
    }
    if !validators::is_valid_poste_numero(poste_numero) {
        return Err(ApiError::bad_request("Numéro de poste invalide (1-100)"));
    }
    let mut row = jmap();
    row.insert("nom".into(), json!(validators::sanitize_input(&nom, 50)));
    row.insert("type".into(), json!(r#type));
    row.insert("poste_numero".into(), json!(poste_numero));
    row.insert(
        "etat".into(),
        json!(etat.unwrap_or_else(|| "disponible".into())),
    );
    row.insert(
        "image_url".into(),
        json!(image_url
            .map(|u| validators::sanitize_input(&u, 500))
            .filter(|u| !u.is_empty())),
    );
    let now = now_iso();
    row.insert("created_at".into(), json!(now.clone()));
    row.insert("date_ajout".into(), json!(now));
    db(&state).insert("consoles", &row)
}

/// PUT /api/consoles/:id
#[tauri::command]
pub fn consoles_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    nom: Option<String>,
    r#type: Option<String>,
    poste_numero: Option<i64>,
    etat: Option<String>,
    image_url: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "consoles", id, "Console non trouvée")?;
    let mut updates = jmap();
    if let Some(nom) = nom {
        if !validators::is_valid_nom(&nom) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        updates.insert("nom".into(), json!(validators::sanitize_input(&nom, 50)));
    }
    if let Some(r#type) = r#type {
        if !validators::is_valid_console_type(&r#type) {
            return Err(ApiError::bad_request("Type de console invalide"));
        }
        updates.insert("type".into(), json!(r#type));
    }
    if let Some(n) = poste_numero {
        if !validators::is_valid_poste_numero(n) {
            return Err(ApiError::bad_request("Numéro de poste invalide (1-100)"));
        }
        updates.insert("poste_numero".into(), json!(n));
    }
    if let Some(etat) = etat {
        updates.insert("etat".into(), json!(validators::sanitize_input(&etat, 50)));
    }
    if let Some(u) = image_url {
        updates.insert(
            "image_url".into(),
            json!(if u.is_empty() {
                Value::Null
            } else {
                json!(validators::sanitize_input(&u, 500))
            }),
        );
    }
    db(&state).update("consoles", id, &updates)
}

/// DELETE /api/consoles/:id
#[tauri::command]
pub fn consoles_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).remove("consoles", id)?;
    Ok(json!({ "success": true }))
}

/// POST /api/consoles/:id/restore
#[tauri::command]
pub async fn consoles_restore(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let mut cloud_row: Option<Value> = None;
    if let Ok(pool) = crate::supabase::get_supabase_pool(&state).await {
        let sql_update = format!("UPDATE consoles SET deleted = 0 WHERE id = {};", id);
        let _ = crate::supabase::supabase_batch_execute(&pool, &sql_update).await;
        let sql_select = format!("SELECT COALESCE(json_agg(t)::text, '[]') FROM (SELECT * FROM consoles WHERE id = {} LIMIT 1) t;", id);
        if let Ok(jrows) = crate::supabase::supabase_query(&pool, &sql_select, &[]).await {
            if !jrows.is_empty() {
                if let Ok(s) = jrows[0].try_get::<_, String>(0) {
                    if let Ok(v) = serde_json::from_str::<Value>(&s) {
                        if let Some(arr) = v.as_array() {
                            cloud_row = arr.first().cloned();
                        }
                    }
                }
            }
        }
    }
    db(&state).restore_with_row("consoles", id, cloud_row.as_ref())
}

/// DELETE /api/consoles/:id/permanent
#[tauri::command]
pub async fn consoles_permanent_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    if let Ok(pool) = crate::supabase::get_supabase_pool(&state).await {
        let sql = format!("DELETE FROM consoles WHERE id = {};", id);
        let _ = crate::supabase::supabase_batch_execute(&pool, &sql).await;
    }
    db(&state).permanent_delete("consoles", id)?;
    Ok(json!({ "success": true, "permanently_deleted": true }))
}
