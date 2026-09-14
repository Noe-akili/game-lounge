// Regroupement des commandes Tauri (équivalent des routes Express).

pub mod auth;
pub mod consoles;
pub mod factures;
pub mod jetons;
pub mod jeux;
pub mod joueurs;
pub mod lignes;
pub mod messages;
pub mod parametres;
pub mod rapports;
pub mod sessions;
pub mod sync;
pub mod tarifs;
pub mod users;

pub use auth::*;
pub use consoles::*;
pub use factures::*;
pub use jetons::*;
pub use jeux::*;
pub use joueurs::*;
pub use lignes::*;
pub use messages::*;
pub use rapports::*;
pub use sessions::*;
pub use sync::*;
pub use tarifs::*;

/// GET /api/health
#[tauri::command]
pub fn health() -> ApiResult<Value> {
    Ok(json!({
        "status": "ok",
        "timestamp": crate::db::now_iso(),
    }))
}

/// Identité de l'appareil (mission §3) : device_id stable, nom, installation_id,
/// created_at, statut d'activation. Persisté en SQLite : stable entre
/// redémarrages, régénéré uniquement si la base disparaît (désinstallation).
#[tauri::command]
pub fn device_info(state: State<'_, AppState>) -> ApiResult<Value> {
    db(&state).device_identity()
}

use serde_json::{Map, Value, json};

use crate::auth::{Claims, require_admin as _require_admin, require_auth as _require_auth};
use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

use tauri::State;

/// Vérifie le token JWT et renvoie les claims de l'utilisateur connecté.
pub fn claims(state: &State<'_, AppState>, token: &Option<String>) -> ApiResult<Claims> {
    _require_auth(token.as_deref(), &state.jwt_secret)
}

pub fn admin_only(user: &Claims) -> ApiResult<()> {
    _require_admin(user)
}

pub fn db<'a>(state: &'a State<'_, AppState>) -> &'a Db {
    &state.db
}

#[inline]
pub fn jmap() -> Map<String, Value> {
    Map::new()
}

/// Sérialise un utilisateur (sans hash de mot de passe).
pub fn user_public(row: &Value) -> Value {
    json!({
        "id": row["id"],
        "email": row["email"],
        "role": row["role"],
        "nom": row["nom"],
    })
}

pub fn get_by_id(db: &Db, table: &str, id: i64, not_found: &str) -> ApiResult<Value> {
    db.get_opt(table, id)?.ok_or_else(|| ApiError::not_found(not_found))
}

pub fn row_id(row: &Value) -> Option<i64> {
    row.get("id").and_then(Value::as_i64)
}

/// Recherche la valeur d'une clé dans une liste de lignes JSON.
pub fn lookup<'a>(rows: &'a [Value], id: Option<i64>, key: &str) -> Option<&'a Value> {
    match id {
        Some(id) => rows.iter().find(|r| row_id(r) == Some(id)).map(|r| &r[key]),
        None => None,
    }
}

pub fn str_field(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

pub fn int_field(row: &Value, key: &str) -> Option<i64> {
    row.get(key).and_then(Value::as_i64)
}

pub fn float_field(row: &Value, key: &str) -> f64 {
    row.get(key)
        .and_then(Value::as_f64)
        .or_else(|| row.get(key).and_then(Value::as_i64).map(|i| i as f64))
        .unwrap_or(0.0)
}

/// Trie des sessions/factures par `created_at` décroissant (chaînes ISO comparables).
pub fn sort_desc_by_created_at(rows: &mut [Value]) {
    rows.sort_by(|a, b| {
        let ca = a.get("created_at").and_then(Value::as_str).unwrap_or("");
        let cb = b.get("created_at").and_then(Value::as_str).unwrap_or("");
        cb.cmp(ca)
    });
}