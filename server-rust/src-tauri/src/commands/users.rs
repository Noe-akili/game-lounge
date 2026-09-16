// /api/users - Gestion des utilisateurs 100% sur Supabase.
// Aucun utilisateur ni mot de passe n'est stocké en local dans SQLite.
// Toute action (list, get, create, update, delete) nécessite une connexion Internet.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::claims;
use crate::error::{ApiError, ApiResult};
use crate::supabase::{escape_sql, supabase_query, supabase_batch_execute, SupabasePool, init_supabase_pool};
use crate::validators;
use crate::AppState;

/// Récupère ou réinitialise le pool Supabase pour garantir l'accès réseau
async fn get_supabase(state: &AppState) -> ApiResult<SupabasePool> {
    if let Ok(guard) = state.supabase_pool.lock() {
        if let Some(ref pool) = *guard {
            return Ok(pool.clone());
        }
    }
    // Tentative de connexion immédiate
    if let Some(pool) = init_supabase_pool().await {
        if let Ok(mut guard) = state.supabase_pool.lock() {
            *guard = Some(pool.clone());
        }
        return Ok(pool);
    }
    Err(ApiError::new(503, "Connexion Internet requise pour gérer les utilisateurs"))
}

fn pg_col_str(row: &tokio_postgres::Row, idx: usize) -> Option<String> {
    if let Ok(s) = row.try_get::<_, &str>(idx) {
        return Some(s.to_string());
    }
    if let Ok(s) = row.try_get::<_, String>(idx) {
        return Some(s);
    }
    None
}

/// GET /api/users - Liste des utilisateurs directement depuis Supabase
#[tauri::command]
pub async fn users_list(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;

    let pool = get_supabase(&state).await?;
    let sql = "SELECT id, email, role, nom, created_at FROM users WHERE deleted = 0 ORDER BY id ASC";
    let rows = supabase_query(&pool, sql, &[])
        .await
        .map_err(|e| ApiError::new(503, &format!("Erreur Supabase: {}", e)))?;

    let mut list = Vec::new();
    for r in rows {
        let id = r.try_get::<_, i64>(0).unwrap_or(0);
        let email = pg_col_str(&r, 1).unwrap_or_default();
        let role = pg_col_str(&r, 2).unwrap_or_else(|| "employe".to_string());
        let nom = pg_col_str(&r, 3).unwrap_or_default();
        let created_at = pg_col_str(&r, 4).unwrap_or_default();

        list.push(json!({
            "id": id,
            "email": email,
            "role": role,
            "nom": nom,
            "created_at": created_at,
        }));
    }

    Ok(Value::Array(list))
}

/// GET /api/users/:id - Détail d'un utilisateur depuis Supabase
#[tauri::command]
pub async fn users_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }

    let pool = get_supabase(&state).await?;
    let sql = format!("SELECT id, email, role, nom, created_at FROM users WHERE id = {} AND deleted = 0 LIMIT 1", id);
    let rows = supabase_query(&pool, &sql, &[])
        .await
        .map_err(|e| ApiError::new(503, &format!("Erreur Supabase: {}", e)))?;

    if rows.is_empty() {
        return Err(ApiError::not_found("Utilisateur non trouvé"));
    }

    let r = &rows[0];
    let uid = r.try_get::<_, i64>(0).unwrap_or(0);
    let email = pg_col_str(r, 1).unwrap_or_default();
    let role = pg_col_str(r, 2).unwrap_or_else(|| "employe".to_string());
    let nom = pg_col_str(r, 3).unwrap_or_default();
    let created_at = pg_col_str(r, 4).unwrap_or_default();

    Ok(json!({
        "id": uid,
        "email": email,
        "role": role,
        "nom": nom,
        "created_at": created_at,
    }))
}

/// POST /api/users - Création d'un utilisateur DIRECTEMENT sur Supabase
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

    let pool = get_supabase(&state).await?;

    // Vérifier l'unicité sur Supabase
    let check_sql = format!(
        "SELECT id, deleted FROM users WHERE LOWER(email) = LOWER('{}') LIMIT 1",
        escape_sql(email)
    );
    let existing = supabase_query(&pool, &check_sql, &[])
        .await
        .map_err(|e| ApiError::new(503, &format!("Erreur Supabase: {}", e)))?;

    // Hachage sécurisé bcrypt (léger, ne crashe pas sur Android)
    let hash = bcrypt::hash(&password, 8)
        .map_err(|e| ApiError::internal(format!("Bcrypt: {}", e)))?;

    if !existing.is_empty() {
        let r = &existing[0];
        let ex_id = r.try_get::<_, i64>(0).unwrap_or(0);
        let deleted = r.try_get::<_, i32>(1).unwrap_or(0);
        if deleted == 1 {
            // Réactivation du compte soft-deleted sur Supabase
            let upd_sql = format!(
                "UPDATE users SET nom = '{}', role = '{}', password_hash = '{}', deleted = 0 WHERE id = {}",
                escape_sql(nom), escape_sql(&role), escape_sql(&hash), ex_id
            );
            supabase_batch_execute(&pool, &upd_sql)
                .await
                .map_err(|e| ApiError::new(503, &format!("Erreur réactivation Supabase: {}", e)))?;

            return Ok(json!({
                "id": ex_id,
                "email": email,
                "role": role,
                "nom": nom,
            }));
        } else {
            return Err(ApiError::new(409, "Cet email est déjà utilisé sur Supabase"));
        }
    }

    // Insertion sur Supabase
    let insert_sql = format!(
        "INSERT INTO users (email, password_hash, role, nom, created_at, deleted) VALUES ('{}', '{}', '{}', '{}', NOW(), 0) RETURNING id",
        escape_sql(email), escape_sql(&hash), escape_sql(&role), escape_sql(nom)
    );

    let rows = supabase_query(&pool, &insert_sql, &[])
        .await
        .map_err(|e| ApiError::new(503, &format!("Erreur insertion Supabase: {}", e)))?;

    let new_id = if !rows.is_empty() {
        rows[0].try_get::<_, i64>(0).unwrap_or(0)
    } else {
        0
    };

    Ok(json!({
        "id": new_id,
        "email": email,
        "role": role,
        "nom": nom,
    }))
}

/// PUT /api/users/:id - Mise à jour d'un utilisateur DIRECTEMENT sur Supabase
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
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }

    let pool = get_supabase(&state).await?;

    let mut sets = Vec::new();

    if let Some(ref em) = email {
        let em = em.trim();
        if !validators::is_valid_email(em) {
            return Err(ApiError::bad_request("Email invalide"));
        }
        // Vérifier conflit avec un autre utilisateur sur Supabase
        let check_sql = format!(
            "SELECT id FROM users WHERE LOWER(email) = LOWER('{}') AND id <> {} AND deleted = 0 LIMIT 1",
            escape_sql(em), id
        );
        let clash = supabase_query(&pool, &check_sql, &[])
            .await
            .map_err(|e| ApiError::new(503, &format!("Erreur Supabase: {}", e)))?;
        if !clash.is_empty() {
            return Err(ApiError::new(409, "Email déjà utilisé par un autre utilisateur sur Supabase"));
        }
        sets.push(format!("email = '{}'", escape_sql(em)));
    }

    if let Some(ref n) = nom {
        let n = n.trim();
        if !validators::is_valid_nom(n) {
            return Err(ApiError::bad_request("Nom invalide"));
        }
        sets.push(format!("nom = '{}'", escape_sql(n)));
    }

    if let Some(ref r) = role {
        if !validators::is_valid_role(r) {
            return Err(ApiError::bad_request("Rôle invalide"));
        }
        sets.push(format!("role = '{}'", escape_sql(r)));
    }

    if let Some(ref pwd) = password {
        let pwd = pwd.trim();
        if !pwd.is_empty() {
            if !validators::is_valid_password(pwd) {
                return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)"));
            }
            let hash = bcrypt::hash(pwd, 8)
                .map_err(|e| ApiError::internal(format!("Bcrypt: {}", e)))?;
            sets.push(format!("password_hash = '{}'", escape_sql(&hash)));
        }
    }

    if sets.is_empty() {
        return Err(ApiError::bad_request("Aucun champ à modifier"));
    }

    let update_sql = format!(
        "UPDATE users SET {} WHERE id = {} AND deleted = 0",
        sets.join(", "), id
    );

    supabase_batch_execute(&pool, &update_sql)
        .await
        .map_err(|e| ApiError::new(503, &format!("Erreur mise à jour Supabase: {}", e)))?;

    Ok(json!({
        "id": id,
        "email": email,
        "role": role,
        "nom": nom,
    }))
}

/// DELETE /api/users/:id - Suppression (soft-delete) DIRECTEMENT sur Supabase
#[tauri::command]
pub async fn users_delete(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    if user.id == id {
        return Err(ApiError::bad_request("Impossible de supprimer son propre compte"));
    }

    let pool = get_supabase(&state).await?;
    let sql = format!("UPDATE users SET deleted = 1 WHERE id = {}", id);
    supabase_batch_execute(&pool, &sql)
        .await
        .map_err(|e| ApiError::new(503, &format!("Erreur suppression Supabase: {}", e)))?;

    Ok(json!({ "id": id, "deleted": true }))
}
