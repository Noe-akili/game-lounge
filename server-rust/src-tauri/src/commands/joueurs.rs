// /api/joueurs - CRUD + historique.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{claims, db, get_by_id, jmap, row_id, sort_desc_by_created_at};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn is_del(v: Option<&Value>) -> bool {
    v.map(|val| val.as_bool().unwrap_or(false) || val.as_i64().unwrap_or(0) == 1).unwrap_or(false)
}

fn duplicate_contact(rows: &[Value], id_to_ignore: Option<i64>, telephone: &str, email: &str) -> bool {
    let phone = telephone.replace([' ', '-'], "");
    let email = email.trim().to_ascii_lowercase();
    rows.iter().any(|row| {
        if row_id(row) == id_to_ignore {
            return false;
        }
        let same_phone = !phone.is_empty()
            && row.get("telephone").and_then(Value::as_str)
                .is_some_and(|value| value.replace([' ', '-'], "") == phone);
        let same_email = !email.is_empty()
            && row.get("email").and_then(Value::as_str)
                .is_some_and(|value| value.trim().eq_ignore_ascii_case(&email));
        same_phone || same_email
    })
}

/// GET /api/joueurs (?search=)
#[tauri::command]
pub async fn joueurs_list(
    state: State<'_, AppState>,
    token: Option<String>,
    search: Option<String>,
    include_deleted: Option<bool>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let include_del = include_deleted.unwrap_or(false);
    let mut joueurs = if include_del {
        let pool = crate::supabase::get_supabase_pool(&state).await
            .map_err(|_| ApiError::network("Connexion Internet requise : les archives sont stockées exclusivement sur Supabase."))?;
        crate::supabase::pull_table_all(&pool, "joueurs").await
            .map_err(|e| ApiError::network(format!("Connexion Internet requise pour charger les archives : {e}")))?
    } else {
        db.query_all("joueurs")?
    };
    if let Some(q) = search {
        let q = q.to_lowercase();
        joueurs.retain(|j| {
            let nom = j.get("nom").and_then(Value::as_str).unwrap_or("").to_lowercase();
            let tel = j.get("telephone").and_then(Value::as_str).unwrap_or("");
            let email = j.get("email").and_then(Value::as_str).unwrap_or("").to_lowercase();
            nom.contains(&q) || tel.contains(&q) || email.contains(&q)
        });
    }
    for j in &mut joueurs {
        let del = is_del(j.get("deleted"));
        if let Some(obj) = j.as_object_mut() {
            obj.insert("deleted".into(), json!(del));
        }
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
    let db = db(&state);
    let telephone_clean = telephone.as_deref().unwrap_or("").replace([' ', '-'], "");
    let email_clean = email.as_deref().unwrap_or("").trim().to_ascii_lowercase();
    if duplicate_contact(&db.query_all("joueurs")?, None, &telephone_clean, &email_clean) {
        return Err(ApiError::bad_request("Ce joueur existe déjà (même téléphone ou e-mail)"));
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
    row.insert(
        "sticker".into(),
        json!(sticker.map(|s| validators::sanitize_input(&s, 16)).filter(|s| !s.is_empty())),
    );
    db.insert("joueurs", &row)
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
    let database = db(&state);
    get_by_id(database, "joueurs", id, "Joueur non trouvé")?;
    let current = get_by_id(database, "joueurs", id, "Joueur non trouvé")?;
    let next_phone = telephone.as_deref().unwrap_or_else(|| current.get("telephone").and_then(Value::as_str).unwrap_or(""));
    let next_email = email.as_deref().unwrap_or_else(|| current.get("email").and_then(Value::as_str).unwrap_or(""));
    if duplicate_contact(&database.query_all("joueurs")?, Some(id), next_phone, next_email) {
        return Err(ApiError::bad_request("Ce joueur existe déjà (même téléphone ou e-mail)"));
    }
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
    database.update("joueurs", id, &updates)
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

/// POST /api/joueurs/:id/restore
#[tauri::command]
pub async fn joueurs_restore(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let mut cloud_row: Option<Value> = None;
    if let Ok(pool) = crate::supabase::get_supabase_pool(&state).await {
        let sql_update = format!("UPDATE joueurs SET deleted = 0 WHERE id = {};", id);
        let _ = crate::supabase::supabase_batch_execute(&pool, &sql_update).await;
        let sql_select = format!("SELECT COALESCE(json_agg(t)::text, '[]') FROM (SELECT * FROM joueurs WHERE id = {} LIMIT 1) t;", id);
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
    db(&state).restore_with_row("joueurs", id, cloud_row.as_ref())
}

/// DELETE /api/joueurs/:id/permanent
#[tauri::command]
pub async fn joueurs_permanent_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    if let Ok(pool) = crate::supabase::get_supabase_pool(&state).await {
        let sql = format!("DELETE FROM joueurs WHERE id = {};", id);
        let _ = crate::supabase::supabase_batch_execute(&pool, &sql).await;
    }
    db(&state).permanent_delete("joueurs", id)?;
    Ok(json!({ "success": true, "permanently_deleted": true }))
}
