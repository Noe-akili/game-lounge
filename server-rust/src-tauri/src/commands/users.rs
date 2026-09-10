// /api/users - gestion des utilisateurs (admin, stockage local).

use serde_json::{Value, json};
use tauri::State;

use crate::auth as auth_core;
use crate::commands::{claims, db, get_by_id, jmap};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn to_public(row: &Value) -> Value {
    json!({
        "id": row["id"],
        "email": row["email"],
        "role": row["role"],
        "nom": row["nom"],
        "created_at": row["created_at"],
    })
}

/// GET /api/users
#[tauri::command]
pub fn users_list(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    let users = db(&state).query_all("users")?;
    Ok(Value::Array(users.iter().map(to_public).collect()))
}

/// GET /api/users/:id
#[tauri::command]
pub fn users_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let row = get_by_id(db(&state), "users", id, "Utilisateur non trouvé")?;
    Ok(to_public(&row))
}

/// POST /api/users - ASYNC pour éviter ANR (scrypt 1-2s sur Android low-end)
#[tauri::command(async)]
pub async fn users_create(
    state: State<'_, AppState>,
    token: Option<String>,
    email: String,
    password: String,
    role: String,
    nom: String,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if email.is_empty() || password.is_empty() || role.is_empty() || nom.is_empty() {
        return Err(ApiError::bad_request("Nom, email, mot de passe et rôle requis"));
    }
    if !validators::is_valid_nom(&nom) {
        return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
    }
    if !validators::is_valid_email(&email) {
        return Err(ApiError::bad_request("Email invalide"));
    }
    if !validators::is_valid_password(&password) {
        return Err(ApiError::bad_request(
            "Mot de passe invalide (min 6 caractères, au moins une lettre)",
        ));
    }
    if !validators::is_valid_role(&role) {
        return Err(ApiError::bad_request("Rôle invalide"));
    }
    let db = db(&state);
    if db
        .find_one("users", |u| u.get("email").and_then(Value::as_str) == Some(email.as_str()))?
        .is_some()
    {
        return Err(ApiError::new(409, "Email déjà utilisé"));
    }
    // Hash dans thread bloquant pour ne pas freezer l'UI Android
    let pwd = password.clone();
    let hash = tokio::task::spawn_blocking(move || auth_core::hash_password(&pwd))
        .await
        .map_err(|e| ApiError::internal(format!("Erreur hachage: {e}")))?
        ?;
    let mut row = jmap();
    row.insert("email".into(), json!(validators::sanitize_input(&email, 100)));
    row.insert("password_hash".into(), json!(hash));
    row.insert("nom".into(), json!(validators::sanitize_input(&nom, 50)));
    row.insert("role".into(), json!(role));
    row.insert("created_at".into(), json!(crate::db::now_iso()));
    let created = db.insert("users", &row)?;
    Ok(to_public(&created))
}

/// PUT /api/users/:id - ASYNC si password (scrypt)
#[tauri::command(async)]
pub async fn users_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    email: Option<String>,
    role: Option<String>,
    nom: Option<String>,
    password: Option<String>,
) -> ApiResult<Value> {
    let current = claims(&state, &token)?;
    crate::commands::admin_only(&current)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    get_by_id(db, "users", id, "Utilisateur non trouvé")?;
    let mut updates = jmap();
    if let Some(e) = email {
        if !validators::is_valid_email(&e) {
            return Err(ApiError::bad_request("Email invalide"));
        }
        updates.insert("email".into(), json!(validators::sanitize_input(&e, 100)));
    }
    if let Some(r) = role {
        if !validators::is_valid_role(&r) {
            return Err(ApiError::bad_request("Rôle invalide"));
        }
        updates.insert("role".into(), json!(r));
    }
    if let Some(n) = nom {
        if !validators::is_valid_nom(&n) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        updates.insert("nom".into(), json!(validators::sanitize_input(&n, 50)));
    }
    if let Some(p) = password {
        if !p.is_empty() {
            if !validators::is_valid_password(&p) {
                return Err(ApiError::bad_request(
                    "Mot de passe invalide (min 6 caractères, au moins une lettre)",
                ));
            }
            let p2 = p.clone();
            let hash = tokio::task::spawn_blocking(move || auth_core::hash_password(&p2))
                .await
                .map_err(|e| ApiError::internal(format!("Erreur hachage: {e}")))?
                ?;
            updates.insert("password_hash".into(), json!(hash));
        }
    }
    let updated = db.update("users", id, &updates)?;
    Ok(to_public(&updated))
}

/// DELETE /api/users/:id
#[tauri::command]
pub fn users_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let current = claims(&state, &token)?;
    crate::commands::admin_only(&current)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    if id == current.id {
        return Err(ApiError::bad_request("Impossible de supprimer votre propre compte"));
    }
    let db = db(&state);
    get_by_id(db, "users", id, "Utilisateur non trouvé")?;
    db.remove("users", id)?;
    Ok(json!({ "success": true }))
}