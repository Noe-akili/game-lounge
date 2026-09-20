// /api/users - Gestion locale immédiate + synchronisation Supabase en arrière-plan.
// Les utilisateurs et leurs hash sont conservés dans SQLite pour éviter les latences
// PostgreSQL sur mobile. Le sync_outbox pousse les changements vers Supabase dès que possible.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{claims, db};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn is_deleted(v: Option<&Value>) -> bool {
    v.map(|x| x.as_bool().unwrap_or(false) || x.as_i64().unwrap_or(0) == 1).unwrap_or(false)
}

/// GET /api/users - Liste locale immédiate (sync Supabase en arrière-plan)
#[tauri::command]
pub async fn users_list(state: State<'_, AppState>, token: Option<String>, include_deleted: Option<bool>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;

    // Lecture locale immédiate : aucune attente PostgreSQL sur le chemin IPC Android.
    // query_all() applique déjà "WHERE deleted = 0" : pour voir les archives il
    // faut query_all_all(), sinon la case "Afficher archivés" ne remonte rien.
    let mut list = if include_deleted.unwrap_or(false) {
        db(&state).query_all_all("users")?
    } else {
        db(&state).query_all("users")?
    };
    if !include_deleted.unwrap_or(false) {
        list.retain(|r| !is_deleted(r.get("deleted")));
    }
    list.sort_by_key(|r| r.get("id").and_then(Value::as_i64).unwrap_or(0));
    for row in &mut list {
        if let Some(obj) = row.as_object_mut() { obj.remove("password_hash"); }
    }
    Ok(Value::Array(list))
}

/// GET /api/users/:id - Détail local immédiat
#[tauri::command]
pub async fn users_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }

    let mut row = db(&state).get_opt("users", id)?
        .ok_or_else(|| ApiError::not_found("Utilisateur non trouvé"))?;
    if let Some(obj) = row.as_object_mut() { obj.remove("password_hash"); }
    Ok(row)
}

/// POST /api/users - Création locale immédiate + synchronisation différée
#[tauri::command]
pub async fn users_create(
    state: State<'_, AppState>, token: Option<String>, email: String,
    password: String, role: String, nom: String,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    let email = email.trim();
    let nom = nom.trim();
    if email.is_empty() || password.is_empty() || role.is_empty() || nom.is_empty() {
        return Err(ApiError::bad_request("Nom, email, mot de passe et rôle requis"));
    }
    if !validators::is_valid_nom(nom) { return Err(ApiError::bad_request("Nom invalide (2-50 caractères)")); }
    if !validators::is_valid_email(email) { return Err(ApiError::bad_request("Email invalide")); }
    if !validators::is_valid_password(&password) { return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)")); }
    if !validators::is_valid_role(&role) { return Err(ApiError::bad_request("Rôle invalide (admin ou employe)")); }

    // IMPORTANT : query_all_all() — la colonne email est UNIQUE en SQLite. Un
    // compte ARCHIVÉ portant le même email est invisible pour query_all(), donc
    // l'INSERT partait quand même et échouait sur la contrainte UNIQUE
    // ("erreur de base de données"). On le retrouve ici pour le réactiver.
    let email_lc = email.to_lowercase();
    let existing = db(&state).query_all_all("users")?.into_iter().find(|r| {
        r.get("email").and_then(Value::as_str).map(|e| e.trim().eq_ignore_ascii_case(&email_lc)).unwrap_or(false)
    });
    let existing_id = existing.as_ref().and_then(|r| r.get("id").and_then(Value::as_i64));
    let existing_deleted = existing.as_ref().map(|r| is_deleted(r.get("deleted"))).unwrap_or(false);
    if existing_id.is_some() && !existing_deleted {
        return Err(ApiError::new(409, "Cet email est déjà utilisé par un compte local actif"));
    }

    let hash_start = std::time::Instant::now();
    let pwd_clone = password.clone();
    let hash = tokio::task::spawn_blocking(move || {
        let argon2_start = std::time::Instant::now();
        let result = std::panic::catch_unwind(|| crate::auth::hash_password(&pwd_clone))
            .map_err(|_| ApiError::internal("Erreur interne lors du hachage du mot de passe"));
        eprintln!("[users_create][PERF] Argon2/hash_password = {:?}", argon2_start.elapsed());
        result
    }).await
      .map_err(|e| ApiError::internal(format!("spawn_blocking: {}", e)))??
      .map_err(|e| ApiError::bad_request(e.to_string()))?;
    eprintln!("[users_create][PERF] attente spawn_blocking + hash = {:?}", hash_start.elapsed());

    let mut row = crate::commands::jmap();
    row.insert("email".into(), json!(email));
    row.insert("password_hash".into(), json!(hash));
    row.insert("role".into(), json!(role));
    row.insert("nom".into(), json!(nom));
    row.insert("created_at".into(), json!(crate::db::now_iso()));
    row.insert("deleted".into(), json!(0));
    let db_start = std::time::Instant::now();
    let (id, reactivated) = if let Some(existing_id) = existing_id {
        row.remove("created_at");
        let updated = db(&state).update("users", existing_id, &row)?;
        (updated.get("id").and_then(Value::as_i64).unwrap_or(existing_id), true)
    } else {
        // Filet de sécurité : la contrainte UNIQUE(email) doit produire un 409
        // explicite, jamais un message technique "erreur de base de données".
        let created = db(&state).insert("users", &row).map_err(|e| {
            if e.message.to_lowercase().contains("unique") {
                ApiError::new(409, "Cet email est déjà pris par un autre compte (peut-être archivé) : activez « Afficher archivés » pour le restaurer")
            } else { e }
        })?;
        (created.get("id").and_then(Value::as_i64).unwrap_or(0), false)
    };
    eprintln!("[users_create][PERF] SQLite + outbox = {:?}", db_start.elapsed());

    Ok(json!({"id": id, "email": email, "role": role, "nom": nom,
        "reactivated": reactivated, "syncPending": true}))
}

/// PUT /api/users/:id - Mise à jour locale immédiate + synchronisation différée
#[tauri::command]
pub async fn users_update(
    state: State<'_, AppState>, token: Option<String>, id: i64,
    email: Option<String>, role: Option<String>, nom: Option<String>, password: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    if user.id != id { crate::commands::admin_only(&user)?; }
    if !user.is_admin() && role.is_some() { return Err(ApiError::forbidden("Seul un administrateur peut modifier les rôles")); }
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    let mut updates = crate::commands::jmap();
    if let Some(ref em) = email {
        let em=em.trim();
        if !validators::is_valid_email(em) { return Err(ApiError::bad_request("Email invalide")); }
        let em_lc=em.to_lowercase();
        // La contrainte UNIQUE(email) en SQLite vise AUSSI les comptes archivés :
        // on les inclut pour renvoyer un 409 lisible plutôt qu'une erreur SQL.
        let clash = db(&state).query_all_all("users")?.into_iter().find(|r| r.get("id").and_then(Value::as_i64)!=Some(id)
            && r.get("email").and_then(Value::as_str).map(|e| e.trim().eq_ignore_ascii_case(&em_lc)).unwrap_or(false));
        if let Some(other) = clash {
            return Err(if is_deleted(other.get("deleted")) {
                ApiError::new(409, "Email déjà utilisé par un compte archivé : restaurez-le ou supprimez-le définitivement")
            } else {
                ApiError::new(409, "Email déjà utilisé par un autre compte local actif")
            });
        }
        updates.insert("email".into(),json!(em));
    }
    if let Some(ref n)=nom { let n=n.trim(); if !validators::is_valid_nom(n){return Err(ApiError::bad_request("Nom invalide"));} updates.insert("nom".into(),json!(n)); }
    if let Some(ref r)=role { let r=r.trim(); if !validators::is_valid_role(r){return Err(ApiError::bad_request("Rôle invalide (admin ou employe)"));} updates.insert("role".into(),json!(r)); }
    if let Some(ref pwd)=password {
        let pwd=pwd.trim();
        if !pwd.is_empty() {
            if !validators::is_valid_password(pwd){return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)"));}
            let pwd_clone=pwd.to_string(); let hash_start=std::time::Instant::now();
            let hash=tokio::task::spawn_blocking(move || {
                let argon2_start=std::time::Instant::now();
                let result=std::panic::catch_unwind(|| crate::auth::hash_password(&pwd_clone))
                    .map_err(|_| ApiError::internal("Erreur interne lors du hachage du mot de passe"));
                eprintln!("[users_update][PERF] Argon2/hash_password = {:?}",argon2_start.elapsed()); result
            }).await.map_err(|e|ApiError::internal(format!("spawn_blocking: {}",e)))??
              .map_err(|e|ApiError::bad_request(e.to_string()))?;
            eprintln!("[users_update][PERF] attente spawn_blocking + hash = {:?}",hash_start.elapsed());
            updates.insert("password_hash".into(),json!(hash));
        }
    }
    if updates.is_empty(){return Err(ApiError::bad_request("Aucun champ à modifier"));}
    let db_start=std::time::Instant::now(); let updated=db(&state).update("users",id,&updates)?;
    eprintln!("[users_update][PERF] SQLite + outbox = {:?}",db_start.elapsed());
    Ok(json!({"id":id,"email":updated.get("email"),"role":updated.get("role"),"nom":updated.get("nom"),"syncPending":true}))
}

/// DELETE /api/users/:id - Archivage local immédiat + sync différée
#[tauri::command]
pub fn users_delete(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    if user.id == id { return Err(ApiError::bad_request("Impossible de supprimer son propre compte")); }
    db(&state).remove("users", id)?;
    Ok(json!({ "id": id, "deleted": true, "syncPending": true }))
}

/// POST /api/users/:id/restore - Restauration locale immédiate + sync différée
#[tauri::command]
pub fn users_restore(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    db(&state).restore("users", id)?;
    Ok(json!({ "id": id, "restored": true, "syncPending": true }))
}

/// DELETE /api/users/:id/permanent - Suppression locale immédiate + sync différée
#[tauri::command]
pub fn users_permanent_delete(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    if user.id == id { return Err(ApiError::bad_request("Impossible de supprimer définitivement son propre compte")); }
    db(&state).permanent_delete("users", id)?;
    Ok(json!({ "id": id, "permanently_deleted": true, "syncPending": true }))
}
