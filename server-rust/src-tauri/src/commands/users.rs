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

/// POST /api/users - SYNCHRONE et direct (<30ms, évite ANR et timeout IPC Android)
#[tauri::command]
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
    let email = email.trim();
    let nom = nom.trim();
    if email.is_empty() || password.is_empty() || role.is_empty() || nom.is_empty() {
        return Err(ApiError::bad_request("Nom, email, mot de passe et rôle requis"));
    }
    if !validators::is_valid_nom(nom) {
        return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
    }
    if !validators::is_valid_email(email) {
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
    let hash = auth_core::hash_password(&password)?;

    // FIX utilisateurs fantômes : si l'utilisateur existe déjà mais est soft-deleted,
    // on le réactive avec les nouvelles valeurs
    if let Some(prev) = db.find_one_all("users", |u| {
        u.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(email))
            && u.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1
    })? {
        let id = prev.get("id").and_then(Value::as_i64).unwrap_or(0);
        if id > 0 {
            let mut upd = jmap();
            upd.insert("deleted".into(), json!(0));
            upd.insert("password_hash".into(), json!(hash));
            upd.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
            upd.insert("role".into(), json!(role));
            upd.insert("created_at".into(), json!(crate::db::now_iso()));
            let revived = db.update("users", id, &upd)?;
            return Ok(to_public(&revived));
        }
    }
    if db
        .find_one("users", |u| {
            u.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 0
                && u.get("email").and_then(Value::as_str).is_some_and(|e| e.eq_ignore_ascii_case(email))
        })?
        .is_some()
    {
        return Err(ApiError::new(409, "Email déjà utilisé"));
    }
    let mut row = jmap();
    row.insert("email".into(), json!(validators::sanitize_input(email, 100)));
    row.insert("password_hash".into(), json!(hash));
    row.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
    row.insert("role".into(), json!(role));
    row.insert("created_at".into(), json!(crate::db::now_iso()));
    let created = db.insert("users", &row)?;
    Ok(to_public(&created))
}

/// PUT /api/users/:id - SYNCHRONE et direct (<30ms, évite timeout IPC)
#[tauri::command]
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
        let e_trimmed = e.trim();
        if !validators::is_valid_email(e_trimmed) {
            return Err(ApiError::bad_request("Email invalide"));
        }
        // Vérifier si un AUTRE utilisateur actif utilise déjà cet email
        if db.find_one("users", |u| {
            let uid = u.get("id").and_then(Value::as_i64).unwrap_or(0);
            uid != id
                && u.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 0
                && u.get("email").and_then(Value::as_str).is_some_and(|existing| existing.eq_ignore_ascii_case(e_trimmed))
        })?.is_some() {
            return Err(ApiError::new(409, "Cet email est déjà utilisé par un autre utilisateur"));
        }
        updates.insert("email".into(), json!(validators::sanitize_input(e_trimmed, 100)));
    }
    if let Some(r) = role {
        if !validators::is_valid_role(&r) {
            return Err(ApiError::bad_request("Rôle invalide"));
        }
        updates.insert("role".into(), json!(r));
    }
    if let Some(n) = nom {
        let n_trimmed = n.trim();
        if !validators::is_valid_nom(n_trimmed) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        updates.insert("nom".into(), json!(validators::sanitize_input(n_trimmed, 50)));
    }
    if let Some(p) = password {
        let p_trimmed = p.trim();
        if !p_trimmed.is_empty() {
            if !validators::is_valid_password(p_trimmed) {
                return Err(ApiError::bad_request(
                    "Mot de passe invalide (min 6 caractères, au moins une lettre)",
                ));
            }
            let hash = auth_core::hash_password(p_trimmed)?;
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
