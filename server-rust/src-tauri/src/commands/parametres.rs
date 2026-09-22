// /api/parametres/fidelite - règles de fidélité.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn default_rule() -> Value {
    json!({
        "id": 0,
        "regle_type": "temps",
        "seuil": 60,
        "jetons_attribues": 1,
        "valeur_jeton": 100,
        "actif": true,
    })
}

fn is_active_flag(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_i64)
        .map(|v| v == 1)
        .or_else(|| value.and_then(Value::as_bool))
        .unwrap_or(false)
}

/// GET /api/parametres/fidelite (règle active ou valeur par défaut)
#[tauri::command]
pub fn fidelite_get(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    let rule = db(&state)
        .find_one("parametres_fidelite", |r| is_active_flag(r.get("actif")))?
        .unwrap_or_else(default_rule);
    Ok(rule)
}

/// PUT /api/parametres/fidelite (upsert de la règle)
#[tauri::command]
pub fn fidelite_put(
    state: State<'_, AppState>,
    token: Option<String>,
    regle_type: Option<String>,
    seuil: Option<i64>,
    jetons_attribues: Option<i64>,
    valeur_jeton: Option<i64>,
    actif: Option<bool>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if let Some(rt) = regle_type.as_deref() {
        if !validators::is_valid_regle_type(rt) {
            return Err(ApiError::bad_request("Type de règle invalide"));
        }
    }
    if let Some(s) = seuil {
        if !validators::is_valid_seuil(s) {
            return Err(ApiError::bad_request("Seuil invalide (1-10000)"));
        }
    }
    if let Some(j) = jetons_attribues {
        if !validators::is_valid_jetons_attribues(j) {
            return Err(ApiError::bad_request("Jetons attribués invalides (1-1000)"));
        }
    }
    if let Some(v) = valeur_jeton {
        if !(1..=1_000_000).contains(&v) {
            return Err(ApiError::bad_request("Valeur d'un jeton invalide (1-1000000 FC)"));
        }
    }
    let db = db(&state);
    let existing = db.find_one("parametres_fidelite", |_| true)?;
    match existing {
        Some(existing_row) => {
            let id = existing_row
                .get("id")
                .and_then(Value::as_i64)
                .ok_or_else(|| ApiError::internal("ID fidélité manquant"))?;
            let mut updates = jmap();
            if let Some(rt) = regle_type {
                updates.insert("regle_type".into(), json!(rt));
            }
            if let Some(s) = seuil {
                updates.insert("seuil".into(), json!(s));
            }
            if let Some(j) = jetons_attribues {
                updates.insert("jetons_attribues".into(), json!(j));
            }
            if let Some(v) = valeur_jeton {
                updates.insert("valeur_jeton".into(), json!(v));
            }
            if let Some(a) = actif {
                updates.insert("actif".into(), json!(if a { 1 } else { 0 }));
            }
            db.update("parametres_fidelite", id, &updates)
        }
        None => {
            if regle_type.is_none() || seuil.is_none() || jetons_attribues.is_none() || valeur_jeton.is_none() {
                return Err(ApiError::bad_request("Champs requis manquants"));
            }
            let mut row = jmap();
            row.insert("regle_type".into(), json!(regle_type.unwrap()));
            row.insert("seuil".into(), json!(seuil.unwrap()));
            row.insert("jetons_attribues".into(), json!(jetons_attribues.unwrap()));
            row.insert("valeur_jeton".into(), json!(valeur_jeton.unwrap()));
            row.insert("actif".into(), json!(if actif.unwrap_or(true) { 1 } else { 0 }));
            db.insert("parametres_fidelite", &row)
        }
    }
}

/// GET /api/parametres/fidelite/:id
#[tauri::command]
pub fn fidelite_get_by_id(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "parametres_fidelite", id, "Paramètre non trouvé")
}

/// POST /api/parametres/fidelite
#[tauri::command]
pub fn fidelite_create(
    state: State<'_, AppState>,
    token: Option<String>,
    regle_type: String,
    seuil: i64,
    jetons_attribues: i64,
    valeur_jeton: i64,
    actif: Option<bool>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if regle_type.is_empty() || seuil <= 0 || jetons_attribues <= 0 || valeur_jeton <= 0 {
        return Err(ApiError::bad_request("Champs requis manquants"));
    }
    if !validators::is_valid_regle_type(&regle_type) {
        return Err(ApiError::bad_request("Type de règle invalide"));
    }
    if !validators::is_valid_seuil(seuil) {
        return Err(ApiError::bad_request("Seuil invalide (1-10000)"));
    }
    if !validators::is_valid_jetons_attribues(jetons_attribues) {
        return Err(ApiError::bad_request("Jetons attribués invalides (1-1000)"));
    }
    if valeur_jeton > 1_000_000 {
        return Err(ApiError::bad_request("Valeur d'un jeton invalide (1-1000000 FC)"));
    }
    let mut row = jmap();
    row.insert("regle_type".into(), json!(regle_type));
    row.insert("seuil".into(), json!(seuil));
    row.insert("jetons_attribues".into(), json!(jetons_attribues));
    row.insert("valeur_jeton".into(), json!(valeur_jeton));
    row.insert("actif".into(), json!(if actif.unwrap_or(true) { 1 } else { 0 }));
    db(&state).insert("parametres_fidelite", &row)
}

/// DELETE /api/parametres/fidelite/:id
#[tauri::command]
pub fn fidelite_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "parametres_fidelite", id, "Paramètre non trouvé")?;
    db(&state).remove("parametres_fidelite", id)?;
    Ok(json!({ "success": true }))
}
/// GET /api/parametres/app/:key or /api/parametres/app
#[tauri::command]
pub async fn app_settings_get(
    state: State<'_, AppState>,
    token: Option<String>,
    key: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let k = key.unwrap_or_else(|| "app_name".to_string());
    let mut val = db(&state).get_setting(&k).ok().and_then(|o| o);

    #[cfg(feature = "supabase-sync")]
    if val.is_none() {
        if let Some(pool) = state.supabase_pool.lock().ok().and_then(|g| g.clone()) {
            if let Ok(rows) = crate::supabase::pull_app_settings(&pool).await {
                for (sk, sv) in rows {
                    let _ = db(&state).set_setting(&sk, &sv);
                    if sk == k {
                        val = Some(sv);
                    }
                }
            }
        }
    }

    let default_val = if k == "app_name" { "Game Lounge" } else { "" };
    let final_val = val.unwrap_or_else(|| default_val.to_string());
    Ok(json!({ "key": k, "value": final_val }))
}

/// POST /api/parametres/app
#[tauri::command]
pub async fn app_settings_set(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    token: Option<String>,
    key: String,
    value: String,
) -> ApiResult<Value> {
    use tauri::Emitter;
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let clean_key = key.trim().to_string();
    if clean_key.is_empty() {
        return Err(ApiError::bad_request("Clé requise"));
    }
    let clean_val = value.trim().to_string();

    db(&state).set_setting(&clean_key, &clean_val)?;

    #[cfg(feature = "supabase-sync")]
    {
        if let Some(pool) = state.supabase_pool.lock().ok().and_then(|g| g.clone()) {
            if let Err(e) = crate::supabase::set_app_setting(&pool, &clean_key, &clean_val).await {
                eprintln!("[app_settings] Erreur sauvegarde Supabase: {}", e);
            } else {
                crate::logger::log_cloud(&format!("app_setting '{}' mis à jour sur Supabase", clean_key));
            }
        }
    }

    let _ = app.emit("app-setting-changed", json!({ "key": clean_key, "value": clean_val }));
    Ok(json!({ "success": true, "key": clean_key, "value": clean_val }))
}
