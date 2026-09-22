// /api/users — GESTION DES COMPTES 100 % EN LIGNE.
//
// Règle absolue de cette version : l'appareil ne stocke JAMAIS de compte, ni de
// mot de passe (même haché). Le cloud (Supabase) est la seule source de vérité :
//   * lire la liste / un compte  -> requête cloud directe ;
//   * créer / modifier / archiver / supprimer -> écriture cloud directe ;
//   * sans réseau -> erreur 503 explicite, AUCUNE écriture locale en attente.
//
// La seule trace locale est un annuaire d'affichage (id, email, nom, rôle) écrit
// par `cache_identite_utilisateur` : il permet d'afficher « session démarrée par
// X » hors ligne, ne contient aucun secret et ne remonte jamais vers le cloud.

use serde_json::{Value, json};
use tauri::State;

use crate::commands::{claims, db};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

/// Connexion cloud obligatoire : la gestion des comptes n'a pas de mode hors ligne.
async fn pool_obligatoire(state: &State<'_, AppState>) -> ApiResult<crate::supabase::CloudPool> {
    crate::supabase::get_supabase_pool(state).await.map_err(|_| {
        ApiError::service_unavailable(
            "Les comptes sont gérés en ligne : connectez l'appareil à Internet pour continuer.",
        )
    })
}

/// Rafraîchit l'annuaire local (aucun secret) après une lecture/écriture cloud.
fn rafraichir_annuaire(base: &crate::db::Db, compte: &Value) {
    let id = compte.get("id").and_then(Value::as_i64).unwrap_or(0);
    if id == 0 {
        return;
    }
    let email = compte.get("email").and_then(Value::as_str).unwrap_or_default();
    let nom = compte.get("nom").and_then(Value::as_str).unwrap_or_default();
    let role = compte.get("role").and_then(Value::as_str).unwrap_or("employe");
    let archive = compte.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1;
    let _ = base.cache_identite_utilisateur(id, email, nom, role, archive);
}

/// GET /api/users — liste lue EN LIGNE (jamais depuis SQLite).
#[tauri::command]
pub async fn users_list(
    state: State<'_, AppState>,
    token: Option<String>,
    include_deleted: Option<bool>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    let pool = pool_obligatoire(&state).await?;
    let mut liste = crate::supabase::cloud_users_list(&pool, include_deleted.unwrap_or(false)).await?;
    liste.sort_by_key(|r| r.get("id").and_then(Value::as_i64).unwrap_or(0));
    // L'annuaire d'affichage suit la liste en ligne : les noms restent lisibles
    // dans l'historique des sessions même quand le réseau tombe.
    let base = db(&state);
    for compte in &liste {
        rafraichir_annuaire(base, compte);
    }
    Ok(Value::Array(liste))
}

/// GET /api/users/:id — détail lu EN LIGNE.
#[tauri::command]
pub async fn users_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let pool = pool_obligatoire(&state).await?;
    let compte = crate::supabase::cloud_user_get(&pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Utilisateur non trouvé"))?;
    rafraichir_annuaire(db(&state), &compte);
    Ok(compte)
}

/// Hachage du mot de passe : calculé sur l'appareil (thread dédié, repli
/// scrypt/bcrypt) puis ENVOYÉ au cloud. Il n'est jamais écrit en SQLite.
fn hash_or_error(password: &str) -> ApiResult<(String, &'static str)> {
    crate::auth::hash_password_resilient(password)
}

/// POST /api/users — création directement dans le cloud.
#[tauri::command]
pub async fn users_create(
    state: State<'_, AppState>, token: Option<String>, email: String,
    password: String, role: String, nom: String,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    let email = email.trim().to_lowercase();
    let nom = nom.trim().to_string();
    let role = role.trim().to_string();
    if email.is_empty() || password.is_empty() || role.is_empty() || nom.is_empty() {
        return Err(ApiError::bad_request("Nom, email, mot de passe et rôle requis"));
    }
    if !validators::is_valid_nom(&nom) { return Err(ApiError::bad_request("Nom invalide (2-50 caractères)")); }
    if !validators::is_valid_email(&email) { return Err(ApiError::bad_request("Email invalide")); }
    if !validators::is_valid_password(&password) { return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)")); }
    if !validators::is_valid_role(&role) { return Err(ApiError::bad_request("Rôle invalide (admin ou employe)")); }

    let pool = pool_obligatoire(&state).await?;
    // Le hachage part sur un thread dédié : Argon2 sur le thread principal
    // Android gelait l'interface (et fermait parfois l'application).
    let (hash, algo) = tokio::task::spawn_blocking(move || hash_or_error(&password))
        .await
        .map_err(|e| ApiError::internal(format!("Tâche de hachage interrompue : {e}")))??;

    let compte = crate::supabase::cloud_user_create(&pool, &email, &hash, &nom, &role).await?;
    rafraichir_annuaire(db(&state), &compte);
    Ok(json!({
        "id": compte.get("id"),
        "email": compte.get("email"),
        "role": compte.get("role"),
        "nom": compte.get("nom"),
        "algo": algo,
        "online": true,
    }))
}

/// PUT /api/users/:id — modification directement dans le cloud
/// (y compris le changement de mot de passe : seul le hash est transmis).
#[tauri::command]
pub async fn users_update(
    state: State<'_, AppState>, token: Option<String>, id: i64,
    email: Option<String>, role: Option<String>, nom: Option<String>, password: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    if user.id != id { crate::commands::admin_only(&user)?; }
    if !user.is_admin() && role.is_some() { return Err(ApiError::forbidden("Seul un administrateur peut modifier les rôles")); }
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }

    let mut champs: Vec<(&str, String)> = Vec::new();
    if let Some(ref em) = email {
        let em = em.trim().to_lowercase();
        if !validators::is_valid_email(&em) { return Err(ApiError::bad_request("Email invalide")); }
        champs.push(("email", em));
    }
    if let Some(ref n) = nom {
        let n = n.trim();
        if !validators::is_valid_nom(n) { return Err(ApiError::bad_request("Nom invalide")); }
        champs.push(("nom", n.to_string()));
    }
    if let Some(ref r) = role {
        let r = r.trim();
        if !validators::is_valid_role(r) { return Err(ApiError::bad_request("Rôle invalide (admin ou employe)")); }
        champs.push(("role", r.to_string()));
    }

    let mut mot_de_passe_change = false;
    let mut algo_utilise: Option<&'static str> = None;
    if let Some(pwd) = password {
        let pwd = pwd.trim().to_string();
        if !pwd.is_empty() {
            if !validators::is_valid_password(&pwd) {
                return Err(ApiError::bad_request(
                    "Mot de passe invalide (min 6 caractères, au moins une lettre)",
                ));
            }
            let (hash, algo) = tokio::task::spawn_blocking(move || hash_or_error(&pwd))
                .await
                .map_err(|e| ApiError::internal(format!("Tâche de hachage interrompue : {e}")))??;
            champs.push(("password_hash", hash));
            algo_utilise = Some(algo);
            mot_de_passe_change = true;
        }
    }
    if champs.is_empty() {
        return Err(ApiError::bad_request("Aucun champ à modifier"));
    }

    let pool = pool_obligatoire(&state).await?;
    let compte = crate::supabase::cloud_user_update(&pool, id, &champs).await?;
    rafraichir_annuaire(db(&state), &compte);
    Ok(json!({
        "id": compte.get("id"),
        "email": compte.get("email"),
        "role": compte.get("role"),
        "nom": compte.get("nom"),
        "password_change": mot_de_passe_change,
        "algo": algo_utilise,
        "online": true,
    }))
}

/// DELETE /api/users/:id — archivage EN LIGNE. L'appareil du compte concerné le
/// découvre en moins d'une minute (account_watcher), efface toutes ses données
/// et revient sur l'écran de connexion.
#[tauri::command]
pub async fn users_delete(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    if user.id == id { return Err(ApiError::bad_request("Impossible de supprimer son propre compte")); }
    let pool = pool_obligatoire(&state).await?;
    crate::supabase::cloud_user_set_deleted(&pool, id, true).await?;
    // L'annuaire local oublie immédiatement ce compte : plus aucune trace.
    let _ = db(&state).oublier_identite_utilisateur(id);
    Ok(json!({ "id": id, "deleted": true, "online": true }))
}

/// POST /api/users/:id/restore — restauration EN LIGNE.
#[tauri::command]
pub async fn users_restore(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    let pool = pool_obligatoire(&state).await?;
    crate::supabase::cloud_user_set_deleted(&pool, id, false).await?;
    if let Some(compte) = crate::supabase::cloud_user_get(&pool, id).await? {
        rafraichir_annuaire(db(&state), &compte);
    }
    Ok(json!({ "id": id, "restored": true, "online": true }))
}

/// DELETE /api/users/:id/permanent — suppression DÉFINITIVE en ligne.
#[tauri::command]
pub async fn users_permanent_delete(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    crate::commands::admin_only(&user)?;
    if !validators::is_valid_id(id) { return Err(ApiError::bad_request("ID invalide")); }
    if user.id == id { return Err(ApiError::bad_request("Impossible de supprimer définitivement son propre compte")); }
    let pool = pool_obligatoire(&state).await?;
    crate::supabase::cloud_user_permanent_delete(&pool, id).await?;
    let _ = db(&state).oublier_identite_utilisateur(id);
    Ok(json!({ "id": id, "permanently_deleted": true, "online": true }))
}
