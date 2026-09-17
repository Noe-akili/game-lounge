// /api/users - Gestion des utilisateurs 100% sur Supabase.
// Aucun utilisateur ni mot de passe n'est stocké en local dans SQLite.
// Toute action (list, get, create, update, delete, restore, permanent_delete) nécessite une connexion Internet.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::claims;
use crate::error::{ApiError, ApiResult};
use crate::supabase::{escape_sql, supabase_query, supabase_batch_execute, SupabasePool, init_supabase_pool};
use crate::validators;
use crate::AppState;

/// Récupère le pool Supabase ou le reconnecte si le socket a été fermé par PgBouncer
async fn get_supabase(state: &AppState) -> ApiResult<SupabasePool> {
    if let Ok(guard) = state.supabase_pool.lock() {
        if let Some(ref pool) = *guard {
            return Ok(pool.clone());
        }
    }
    // Tentative de connexion immédiate si pas encore initialisé
    if let Some(pool) = init_supabase_pool().await {
        if let Ok(mut guard) = state.supabase_pool.lock() {
            *guard = Some(pool.clone());
        }
        return Ok(pool);
    }
    Err(ApiError::new(503, "Connexion à Supabase impossible. Vérifiez votre connexion Internet."))
}

/// Exécute une requête avec un retry automatique si le pool contenait une connexion fermée
async fn query_with_retry(state: &AppState, sql: &str) -> ApiResult<Vec<tokio_postgres::Row>> {
    let pool = get_supabase(state).await?;
    match supabase_query(&pool, sql, &[]).await {
        Ok(rows) => Ok(rows),
        Err(e) => {
            eprintln!("[users] première tentative échouée ({e}), reconnexion au pool Supabase...");
            if let Ok(mut guard) = state.supabase_pool.lock() {
                *guard = None;
            }
            if let Some(new_pool) = init_supabase_pool().await {
                if let Ok(mut guard) = state.supabase_pool.lock() {
                    *guard = Some(new_pool.clone());
                }
                supabase_query(&new_pool, sql, &[])
                    .await
                    .map_err(|e2| ApiError::new(503, &format!("Erreur Supabase: {}", e2)))
            } else {
                Err(ApiError::new(503, &format!("Erreur Supabase: {}", e)))
            }
        }
    }
}

/// Exécute un batch avec retry automatique si connexion fermée
async fn execute_with_retry(state: &AppState, sql: &str) -> ApiResult<()> {
    let pool = get_supabase(state).await?;
    match supabase_batch_execute(&pool, sql).await {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("[users] premier execute échoué ({e}), reconnexion...");
            if let Ok(mut guard) = state.supabase_pool.lock() {
                *guard = None;
            }
            if let Some(new_pool) = init_supabase_pool().await {
                if let Ok(mut guard) = state.supabase_pool.lock() {
                    *guard = Some(new_pool.clone());
                }
                supabase_batch_execute(&new_pool, sql)
                    .await
                    .map_err(|e2| ApiError::new(503, &format!("Erreur Supabase: {}", e2)))
            } else {
                Err(ApiError::new(503, &format!("Erreur Supabase: {}", e)))
            }
        }
    }
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

fn pg_col_id(row: &tokio_postgres::Row, idx: usize) -> i64 {
    if let Ok(id) = row.try_get::<_, i64>(idx) {
        return id;
    }
    if let Ok(id) = row.try_get::<_, i32>(idx) {
        return id as i64;
    }
    0
}

/// GET /api/users - Liste des utilisateurs directement depuis Supabase
#[tauri::command]
pub async fn users_list(state: State<'_, AppState>, token: Option<String>, include_deleted: Option<bool>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;

    let sql = if include_deleted.unwrap_or(false) {
        "SELECT id, email, role, nom, created_at, COALESCE(deleted::text, '0') as deleted FROM users ORDER BY id ASC".to_string()
    } else {
        "SELECT id, email, role, nom, created_at, COALESCE(deleted::text, '0') as deleted FROM users WHERE COALESCE(deleted::text, '0') NOT IN ('1', 'true') ORDER BY id ASC".to_string()
    };
    let rows = query_with_retry(&state, &sql).await?;

    let mut list = Vec::new();
    for r in rows {
        let id = pg_col_id(&r, 0);
        let email = pg_col_str(&r, 1).unwrap_or_default();
        let role = pg_col_str(&r, 2).unwrap_or_else(|| "employe".to_string());
        let nom = pg_col_str(&r, 3).unwrap_or_default();
        let created_at = pg_col_str(&r, 4).unwrap_or_default();
        let del_str = pg_col_str(&r, 5).unwrap_or_default();
        let is_deleted = del_str == "1" || del_str == "true";

        list.push(json!({
            "id": id,
            "email": email,
            "role": role,
            "nom": nom,
            "created_at": created_at,
            "deleted": is_deleted,
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

    let sql = format!("SELECT id, email, role, nom, created_at, COALESCE(deleted::text, '0') FROM users WHERE id = {} LIMIT 1", id);
    let rows = query_with_retry(&state, &sql).await?;

    if rows.is_empty() {
        return Err(ApiError::not_found("Utilisateur non trouvé"));
    }

    let r = &rows[0];
    let uid = pg_col_id(r, 0);
    let email = pg_col_str(r, 1).unwrap_or_default();
    let role = pg_col_str(r, 2).unwrap_or_else(|| "employe".to_string());
    let nom = pg_col_str(r, 3).unwrap_or_default();
    let created_at = pg_col_str(r, 4).unwrap_or_default();
    let del_str = pg_col_str(r, 5).unwrap_or_default();

    Ok(json!({
        "id": uid,
        "email": email,
        "role": role,
        "nom": nom,
        "created_at": created_at,
        "deleted": del_str == "1" || del_str == "true",
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
        return Err(ApiError::bad_request("Rôle invalide (admin ou employe)"));
    }

    // Vérifier l'existence sur Supabase (actif ou archivé)
    let check_sql = format!(
        "SELECT id, COALESCE(deleted::text, '0') FROM users WHERE LOWER(email) = LOWER('{}') LIMIT 1",
        escape_sql(email)
    );
    let existing = query_with_retry(&state, &check_sql).await?;

    // Hachage sécurisé bcrypt (coût 8)
    let hash = bcrypt::hash(&password, 8)
        .map_err(|e| ApiError::internal(format!("Bcrypt: {}", e)))?;

    if !existing.is_empty() {
        let r = &existing[0];
        let ex_id = pg_col_id(r, 0);
        let del_str = pg_col_str(r, 1).unwrap_or_default();
        let is_deleted = del_str == "1" || del_str == "true";

        if is_deleted {
            // Réactivation automatique du compte précédemment archivé
            let upd_sql = format!(
                "UPDATE users SET nom = '{}', role = '{}', password_hash = '{}', deleted = 0 WHERE id = {}",
                escape_sql(nom), escape_sql(&role), escape_sql(&hash), ex_id
            );
            execute_with_retry(&state, &upd_sql).await?;

            return Ok(json!({
                "id": ex_id,
                "email": email,
                "role": role,
                "nom": nom,
                "reactivated": true
            }));
        } else {
            return Err(ApiError::new(409, "Cet email est déjà utilisé par un compte actif"));
        }
    }

    let now_str = crate::db::now_iso();

    // Insertion sur Supabase
    let insert_sql = format!(
        "INSERT INTO users (email, password_hash, role, nom, created_at, deleted) VALUES ('{}', '{}', '{}', '{}', '{}', 0) RETURNING id",
        escape_sql(email), escape_sql(&hash), escape_sql(&role), escape_sql(nom), now_str
    );

    let rows = query_with_retry(&state, &insert_sql).await?;
    let new_id = if !rows.is_empty() {
        pg_col_id(&rows[0], 0)
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

    let mut sets = Vec::new();

    if let Some(ref em) = email {
        let em = em.trim();
        if !validators::is_valid_email(em) {
            return Err(ApiError::bad_request("Email invalide"));
        }
        // Vérifier conflit avec un autre utilisateur
        let check_sql = format!(
            "SELECT id FROM users WHERE LOWER(email) = LOWER('{}') AND id <> {} AND COALESCE(deleted::text, '0') NOT IN ('1', 'true') LIMIT 1",
            escape_sql(em), id
        );
        let clash = query_with_retry(&state, &check_sql).await?;
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
        let r = r.trim();
        if !validators::is_valid_role(r) {
            return Err(ApiError::bad_request("Rôle invalide (admin ou employe)"));
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
        "UPDATE users SET {} WHERE id = {}",
        sets.join(", "), id
    );

    execute_with_retry(&state, &update_sql).await?;

    Ok(json!({
        "id": id,
        "email": email,
        "role": role,
        "nom": nom,
    }))
}

/// DELETE /api/users/:id - Archivage (soft-delete) DIRECTEMENT sur Supabase
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

    let sql = format!("UPDATE users SET deleted = 1 WHERE id = {}", id);
    execute_with_retry(&state, &sql).await?;

    Ok(json!({ "id": id, "deleted": true }))
}

/// POST /api/users/:id/restore - Restauration d'un utilisateur archivé sur Supabase
#[tauri::command]
pub async fn users_restore(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }

    let sql = format!("UPDATE users SET deleted = 0 WHERE id = {}", id);
    execute_with_retry(&state, &sql).await?;

    Ok(json!({ "id": id, "restored": true }))
}

/// DELETE /api/users/:id/permanent - Suppression DÉFINITIVE sur Supabase
#[tauri::command]
pub async fn users_permanent_delete(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    if user.id == id {
        return Err(ApiError::bad_request("Impossible de supprimer définitivement son propre compte"));
    }

    let sql = format!("DELETE FROM users WHERE id = {}", id);
    execute_with_retry(&state, &sql).await?;

    Ok(json!({ "id": id, "permanently_deleted": true }))
}
