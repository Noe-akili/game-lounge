#!/usr/bin/env -S cargo run --features http-server --bin gl-server --quiet
// Serveur HTTP de débogage : expose exactement le même backend Rust que les
// commandes Tauri, pour pouvoir utiliser le frontend dans un navigateur et
// vérifier si le crash vient de la logique Rust ou de la couche WebView.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{Map, Value, json};

use game_lounge_rust_lib::auth::{self, Claims};
use game_lounge_rust_lib::commands::{
    admin_only as _admin_only, get_by_id as _get_by_id, jmap, row_id, sort_desc_by_created_at,
    user_public,
};
use game_lounge_rust_lib::commands::compute_montant;
use game_lounge_rust_lib::db::{Db, now_iso, today_str};
use game_lounge_rust_lib::error::{ApiError, ApiResult};
use game_lounge_rust_lib::validators;
use game_lounge_rust_lib::AppState;

type SharedState = Arc<AppState>;

// ===== Helpers (équivalent des helpers Tauri, mais sans tauri::State) =====

fn claims(state: &AppState, token: &Option<String>) -> ApiResult<Claims> {
    auth::require_auth(token.as_deref(), &state.jwt_secret)
}

fn db(state: &AppState) -> &Db {
    &state.db
}

fn get_by_id(state: &AppState, table: &str, id: i64, not_found: &str) -> ApiResult<Value> {
    _get_by_id(db(state), table, id, not_found)
}

fn admin_only(user: &Claims) -> ApiResult<()> {
    _admin_only(user)
}

fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// ===== Réponse JSON unifiée =====

fn ok(v: Value) -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(v))
}

fn reply(r: ApiResult<Value>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => ok(v),
        Err(e) => {
            let status = StatusCode::from_u16(e.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(json!({ "message": e.message, "status": e.status })))
        }
    }
}

// ===== Health =====

async fn health() -> (StatusCode, Json<Value>) {
    reply(Ok(json!({
        "status": "ok",
        "timestamp": now_iso(),
        "mode": "http-debug"
    })))
}

// ===== Auth (login email/mot de passe : SQLite locale, fallback Supabase) =====

const LOGIN_WINDOW_MS: i64 = 15 * 60 * 1000;
const LOGIN_MAX: usize = 20;

fn too_many_attempts(state: &AppState, key: &str) -> bool {
    let now = chrono::Utc::now().timestamp_millis();
    let mut map = match state.login_attempts.lock() {
        Ok(m) => m,
        Err(_) => return true,
    };
    let entries = map.entry(key.to_string()).or_default();
    entries.retain(|t| now - t < LOGIN_WINDOW_MS);
    if entries.len() >= LOGIN_MAX {
        true
    } else {
        entries.push(now);
        false
    }
}

async fn auth_login(
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let email = body.get("email").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();
    let password = body.get("password").and_then(Value::as_str).unwrap_or("").to_string();

    if email.is_empty() || password.is_empty() {
        return reply(Err(ApiError::bad_request("Email et mot de passe requis")));
    }
    if !validators::is_valid_email(&email) {
        return reply(Err(ApiError::bad_request("Email invalide")));
    }
    if !validators::is_valid_password(&password) {
        return reply(Err(ApiError::bad_request(
            "Mot de passe invalide (min 6 caractères, au moins une lettre)",
        )));
    }
    if too_many_attempts(&state, &format!("email:{}", email)) {
        return reply(Err(ApiError::new(429, "Trop de tentatives, réessayez plus tard")));
    }

    let user = match db(&state).find_one("users", |r| {
        r.get("email")
            .and_then(Value::as_str)
            .is_some_and(|stored| stored.eq_ignore_ascii_case(&email))
    }) {
        Ok(Some(u)) => u,
        Ok(None) => return reply(Err(ApiError::unauthorized("Identifiants incorrects"))),
        Err(e) => return reply(Err(e)),
    };

    let stored = user
        .get("password_hash")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::unauthorized("Identifiants incorrects"));

    let stored = match stored {
        Ok(s) => s,
        Err(e) => return reply(Err(e)),
    };

    if !auth::compare_password(&password, &stored) {
        return reply(Err(ApiError::unauthorized("Identifiants incorrects")));
    }

    // Mise à niveau des anciens hashs bcrypt vers scrypt au premier login réussi.
    if !auth::is_scrypt_hash(&stored) {
        if let Ok(upgraded) = auth::hash_password(&password) {
            let mut upd = jmap();
            upd.insert("password_hash".into(), json!(upgraded));
            if let Some(id) = user.get("id").and_then(Value::as_i64) {
                let _ = db(&state).update("users", id, &upd);
            }
        }
    }

    let c = Claims {
        id: user.get("id").and_then(Value::as_i64).unwrap_or(0),
        email: user.get("email").and_then(Value::as_str).unwrap_or("").to_string(),
        role: user.get("role").and_then(Value::as_str).unwrap_or("employe").to_string(),
        nom: user.get("nom").and_then(Value::as_str).unwrap_or("").to_string(),
        iat: 0,
        exp: 0,
    };
    let (token, _refresh) = match auth::generate_token_pair(&c, &state.jwt_secret) {
        Ok(t) => t,
        Err(e) => return reply(Err(e)),
    };

    reply(Ok(json!({ "token": token, "user": user_public(&user), "source": "local" })))
}

async fn auth_logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    reply(claims(&state, &token).map(|_| json!({ "success": true })))
}

async fn auth_me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let c = claims(&state, &token)?;
        let user = db(&state)
            .find_one("users", |r| r.get("id").and_then(Value::as_i64) == Some(c.id))?;
        match user {
            Some(u) => Ok(json!({ "user": user_public(&u) })),
            None => Err(ApiError::not_found("Utilisateur non trouvé")),
        }
    })();
    reply(result)
}

// ===== Consoles =====

fn enrich_consoles(state: &AppState) -> ApiResult<Vec<Value>> {
    let d = db(state);
    let consoles = d.query_all("consoles")?;
    let sessions: Vec<Value> = d
        .query_all("sessions_jeu")?
        .into_iter()
        .filter(|s| {
            let st = s.get("statut").and_then(Value::as_str).unwrap_or("");
            st == "en_cours" || st == "pause"
        })
        .collect();
    let joueurs = d.query_all("joueurs")?;
    let jeux = d.query_all("jeux")?;
    let mut result: Vec<Value> = consoles
        .iter()
        .map(|c| {
            let session = sessions
                .iter()
                .find(|s| s.get("console_id").and_then(Value::as_i64) == c.get("id").and_then(Value::as_i64));
            let joueur_id = session.and_then(|s| s.get("joueur_id").and_then(Value::as_i64));
            let jeu_id = session.and_then(|s| s.get("jeu_id").and_then(Value::as_i64));
            json!({
                "id": c["id"],
                "nom": c["nom"],
                "type": c["type"],
                "etat": c["etat"],
                "poste_numero": c["poste_numero"],
                "date_ajout": c["date_ajout"],
                "session_id": session.and_then(|s| s.get("id")).cloned().unwrap_or(Value::Null),
                "session_statut": session.and_then(|s| s.get("statut")).cloned().unwrap_or(Value::Null),
                "session_debut": session.and_then(|s| s.get("debut")).cloned().unwrap_or(Value::Null),
                // Secondes exactes déjà jouées + allocation (chrono temps réel).
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
        .collect();
    result.sort_by(|a, b| {
        a.get("poste_numero").and_then(Value::as_i64).unwrap_or(0)
            .cmp(&b.get("poste_numero").and_then(Value::as_i64).unwrap_or(0))
    });
    Ok(result)
}

async fn consoles_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        Ok(Value::Array(enrich_consoles(&state)?))
    })();
    reply(result)
}

async fn consoles_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "consoles", id, "Console non trouvée")
    })();
    reply(result)
}

async fn consoles_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let nom = body.get("nom").and_then(Value::as_str).unwrap_or("");
        let r#type = body.get("type").and_then(Value::as_str).unwrap_or("");
        let poste_numero = body.get("poste_numero").and_then(Value::as_i64).unwrap_or(0);
        let etat = body.get("etat").and_then(Value::as_str).map(|s| s.to_string());
        if nom.is_empty() || r#type.is_empty() {
            return Err(ApiError::bad_request("Champs requis manquants"));
        }
        if !validators::is_valid_nom(nom) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        if !validators::is_valid_console_type(r#type) {
            return Err(ApiError::bad_request("Type de console invalide"));
        }
        if !validators::is_valid_poste_numero(poste_numero) {
            return Err(ApiError::bad_request("Numéro de poste invalide (1-100)"));
        }
        let mut row = jmap();
        row.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
        row.insert("type".into(), json!(r#type));
        row.insert("poste_numero".into(), json!(poste_numero));
        row.insert("etat".into(), json!(etat.unwrap_or_else(|| "disponible".into())));
        // Image de couverture (URL) — visuel principal des cartes (item 6).
        row.insert(
            "image_url".into(),
            json!(body.get("image_url").and_then(Value::as_str)
                .map(|u| validators::sanitize_input(u, 500))
                .filter(|u| !u.is_empty())),
        );
        row.insert("created_at".into(), json!(now_iso()));
        db(&state).insert("consoles", &row)
    })();
    reply(result)
}

async fn consoles_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "consoles", id, "Console non trouvée")?;
        let mut updates = jmap();
        if let Some(nom) = body.get("nom").and_then(Value::as_str) {
            if !validators::is_valid_nom(nom) {
                return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
            }
            updates.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
        }
        if let Some(r#type) = body.get("type").and_then(Value::as_str) {
            if !validators::is_valid_console_type(r#type) {
                return Err(ApiError::bad_request("Type de console invalide"));
            }
            updates.insert("type".into(), json!(r#type));
        }
        if let Some(n) = body.get("poste_numero").and_then(Value::as_i64) {
            if !validators::is_valid_poste_numero(n) {
                return Err(ApiError::bad_request("Numéro de poste invalide (1-100)"));
            }
            updates.insert("poste_numero".into(), json!(n));
        }
        if let Some(etat) = body.get("etat").and_then(Value::as_str) {
            updates.insert("etat".into(), json!(validators::sanitize_input(etat, 50)));
        }
        if let Some(u) = body.get("image_url").and_then(Value::as_str) {
            updates.insert("image_url".into(), if u.is_empty() { json!(Value::Null) } else { json!(validators::sanitize_input(u, 500)) });
        }
        db(&state).update("consoles", id, &updates)
    })();
    reply(result)
}

async fn consoles_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        db(&state).remove("consoles", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Jeux =====

async fn jeux_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let mut jeux = db(&state).query_all("jeux")?;
        jeux.retain(|j| j.get("actif").map(|a| !matches!(a, Value::Null)).unwrap_or(false));
        if let Some(cid) = q.get("console_id").and_then(|s| s.parse::<i64>().ok()) {
            jeux.retain(|j| j.get("console_id").and_then(Value::as_i64) == Some(cid));
        }
        Ok(Value::Array(jeux))
    })();
    reply(result)
}

async fn jeux_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "jeux", id, "Jeu non trouvé")
    })();
    reply(result)
}

async fn jeux_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let titre = body.get("titre").and_then(Value::as_str).unwrap_or("");
        let genre = body.get("genre").and_then(Value::as_str).map(|s| s.to_string());
        let console_id = body.get("console_id").and_then(Value::as_i64);
        let jaquette_url = body.get("jaquette_url").and_then(Value::as_str).map(|s| s.to_string());
        if titre.is_empty() {
            return Err(ApiError::bad_request("Titre requis"));
        }
        if !validators::is_valid_titre(titre) {
            return Err(ApiError::bad_request("Titre invalide (2-100 caractères)"));
        }
        if let Some(g) = genre.as_deref() {
            if !validators::is_valid_genre(Some(g)) {
                return Err(ApiError::bad_request("Genre invalide (2-50 caractères)"));
            }
        }
        if let Some(cid) = console_id {
            if !validators::is_valid_id(cid) {
                return Err(ApiError::bad_request("Console ID invalide"));
            }
        }
        let mut row = jmap();
        row.insert("titre".into(), json!(validators::sanitize_input(titre, 100)));
        row.insert("genre".into(), json!(genre.as_deref().map(|g| validators::sanitize_input(g, 50))));
        row.insert("console_id".into(), json!(console_id));
        row.insert("jaquette_url".into(), json!(jaquette_url.as_deref().map(|u| validators::sanitize_input(u, 500))));
        // image_url unifiée (item 6) : accepte image_url ou jaquette_url.
        row.insert(
            "image_url".into(),
            json!(body.get("image_url").or_else(|| body.get("jaquette_url"))
                .and_then(Value::as_str)
                .map(|u| validators::sanitize_input(u, 500))
                .filter(|u| !u.is_empty())),
        );
        row.insert("actif".into(), json!(1));
        row.insert("created_at".into(), json!(now_iso()));
        db(&state).insert("jeux", &row)
    })();
    reply(result)
}

async fn jeux_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "jeux", id, "Jeu non trouvé")?;
        let mut updates = jmap();
        if let Some(t) = body.get("titre").and_then(Value::as_str) {
            if !validators::is_valid_titre(t) {
                return Err(ApiError::bad_request("Titre invalide (2-100 caractères)"));
            }
            updates.insert("titre".into(), json!(validators::sanitize_input(t, 100)));
        }
        if let Some(g) = body.get("genre").and_then(Value::as_str) {
            if !g.is_empty() && !validators::is_valid_genre(Some(g)) {
                return Err(ApiError::bad_request("Genre invalide"));
            }
            updates.insert("genre".into(), if g.is_empty() { json!(Value::Null) } else { json!(validators::sanitize_input(g, 50)) });
        }
        if let Some(cid) = body.get("console_id") {
            if let Some(c) = cid.as_i64() {
                if !validators::is_valid_id(c) {
                    return Err(ApiError::bad_request("Console ID invalide"));
                }
            }
            updates.insert("console_id".into(), cid.clone());
        }
        if let Some(u) = body.get("jaquette_url").and_then(Value::as_str) {
            updates.insert("jaquette_url".into(), if u.is_empty() { json!(Value::Null) } else { json!(validators::sanitize_input(u, 500)) });
        }
        // image_url unifiée (item 6) : accepte image_url ou jaquette_url.
        if let Some(u) = body.get("image_url").or_else(|| body.get("jaquette_url")).and_then(Value::as_str) {
            updates.insert("image_url".into(), if u.is_empty() { json!(Value::Null) } else { json!(validators::sanitize_input(u, 500)) });
        }
        db(&state).update("jeux", id, &updates)
    })();
    reply(result)
}

async fn jeux_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        db(&state).remove("jeux", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Joueurs =====

async fn joueurs_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let mut joueurs = db(&state).query_all("joueurs")?;
        if let Some(qs) = q.get("search") {
            let qs = qs.to_lowercase();
            joueurs.retain(|j| {
                let nom = j.get("nom").and_then(Value::as_str).unwrap_or("").to_lowercase();
                let tel = j.get("telephone").and_then(Value::as_str).unwrap_or("");
                let email = j.get("email").and_then(Value::as_str).unwrap_or("").to_lowercase();
                nom.contains(&qs) || tel.contains(&qs) || email.contains(&qs)
            });
        }
        Ok(Value::Array(joueurs))
    })();
    reply(result)
}

async fn joueurs_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "joueurs", id, "Joueur non trouvé")
    })();
    reply(result)
}

async fn joueurs_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let nom = body.get("nom").and_then(Value::as_str).unwrap_or("");
        let telephone = body.get("telephone").and_then(Value::as_str).map(|s| s.to_string());
        let email = body.get("email").and_then(Value::as_str).map(|s| s.to_string());
        if nom.is_empty() {
            return Err(ApiError::bad_request("Nom requis"));
        }
        if !validators::is_valid_nom(nom) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        if let Some(t) = telephone.as_deref() {
            if !t.is_empty() && !validators::is_valid_phone(t) {
                return Err(ApiError::bad_request("Téléphone invalide (8-15 chiffres, ex: +243...)"));
            }
        }
        if let Some(e) = email.as_deref() {
            if !e.is_empty() && !validators::is_valid_email(e) {
                return Err(ApiError::bad_request("Email invalide"));
            }
        }
        let mut row = jmap();
        row.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
        row.insert("telephone".into(), json!(telephone.map(|t| t.replace([' ', '-'], "")).unwrap_or_default()));
        row.insert("email".into(), json!(email.map(|e| validators::sanitize_input(&e, 100)).unwrap_or_default()));
        row.insert("jetons_solde".into(), json!(0));
        row.insert("date_inscription".into(), json!(now_iso()));
        row.insert("derniere_visite".into(), json!(Value::Null));
        // Sticker/icône du joueur (emoji ou URL courte) — item 6.
        row.insert(
            "sticker".into(),
            json!(body.get("sticker").and_then(Value::as_str)
                .map(|s| validators::sanitize_input(s, 16))
                .filter(|s| !s.is_empty())),
        );
        db(&state).insert("joueurs", &row)
    })();
    reply(result)
}

async fn joueurs_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "joueurs", id, "Joueur non trouvé")?;
        let mut updates = jmap();
        if let Some(n) = body.get("nom").and_then(Value::as_str) {
            if !validators::is_valid_nom(n) {
                return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
            }
            updates.insert("nom".into(), json!(validators::sanitize_input(n, 50)));
        }
        if let Some(t) = body.get("telephone").and_then(Value::as_str) {
            if !t.is_empty() && !validators::is_valid_phone(t) {
                return Err(ApiError::bad_request("Téléphone invalide (8-15 chiffres, ex: +243...)"));
            }
            updates.insert("telephone".into(), json!(t.replace([' ', '-'], "")));
        }
        if let Some(e) = body.get("email").and_then(Value::as_str) {
            if !e.is_empty() && !validators::is_valid_email(e) {
                return Err(ApiError::bad_request("Email invalide"));
            }
            updates.insert("email".into(), json!(if e.is_empty() { String::new() } else { validators::sanitize_input(e, 100) }));
        }
        if let Some(js) = body.get("jetons_solde").and_then(Value::as_i64) {
            if js < 0 {
                return Err(ApiError::bad_request("Jetons invalides"));
            }
            updates.insert("jetons_solde".into(), json!(js));
        }
        if let Some(s) = body.get("sticker").and_then(Value::as_str) {
            updates.insert("sticker".into(), if s.is_empty() { json!(Value::Null) } else { json!(validators::sanitize_input(s, 16)) });
        }
        db(&state).update("joueurs", id, &updates)
    })();
    reply(result)
}

async fn joueurs_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "joueurs", id, "Joueur non trouvé")?;
        db(&state).remove("joueurs", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

async fn joueurs_historique(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let joueur = get_by_id(&state, "joueurs", id, "Joueur non trouvé")?;
        let mut sessions: Vec<Value> = d.query_all("sessions_jeu")?.into_iter()
            .filter(|s| s.get("joueur_id").and_then(Value::as_i64) == Some(id)).collect();
        sort_desc_by_created_at(&mut sessions);
        let mut transactions: Vec<Value> = d.query_all("jetons_transactions")?.into_iter()
            .filter(|t| t.get("joueur_id").and_then(Value::as_i64) == Some(id)).collect();
        sort_desc_by_created_at(&mut transactions);
        let mut factures: Vec<Value> = d.query_all("factures")?.into_iter()
            .filter(|f| f.get("joueur_id").and_then(Value::as_i64) == Some(id)).collect();
        sort_desc_by_created_at(&mut factures);
        let consoles = d.query_all("consoles")?;
        let jeux = d.query_all("jeux")?;
        let enriched_sessions: Vec<Value> = sessions.iter().map(|s| {
            let console_id = s.get("console_id").and_then(Value::as_i64);
            let jeu_id = s.get("jeu_id").and_then(Value::as_i64);
            let mut o = s.clone();
            if let Value::Object(map) = &mut o {
                map.insert("console_nom".into(), consoles.iter().find(|c| row_id(c) == console_id).and_then(|c| c.get("nom")).cloned().unwrap_or(Value::Null));
                map.insert("jeu_nom".into(), jeux.iter().find(|j| row_id(j) == jeu_id).and_then(|j| j.get("titre")).cloned().unwrap_or(Value::Null));
            }
            o
        }).collect();
        Ok(json!({
            "joueur": joueur,
            "sessions": enriched_sessions,
            "transactions": transactions,
            "factures": factures,
        }))
    })();
    reply(result)
}

// ===== Sessions =====

fn parse_iso_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp_millis())
}

fn enrich_session(s: &Value, state: &AppState) -> ApiResult<Value> {
    let d = db(state);
    let consoles = d.query_all("consoles")?;
    let joueurs = d.query_all("joueurs")?;
    let jeux = d.query_all("jeux")?;
    let users = d.query_all("users")?;
    let console_id = s.get("console_id").and_then(Value::as_i64);
    let joueur_id = s.get("joueur_id").and_then(Value::as_i64);
    let jeu_id = s.get("jeu_id").and_then(Value::as_i64);
    let employe_id = s.get("employe_id").and_then(Value::as_i64);
    let mut o = s.clone();
    if let Value::Object(map) = &mut o {
        map.insert("console_nom".into(), consoles.iter().find(|c| row_id(c) == console_id).and_then(|c| c.get("nom")).cloned().unwrap_or(Value::Null));
        map.insert("console_type".into(), consoles.iter().find(|c| row_id(c) == console_id).and_then(|c| c.get("type")).cloned().unwrap_or(Value::Null));
        map.insert("poste_numero".into(), consoles.iter().find(|c| row_id(c) == console_id).and_then(|c| c.get("poste_numero")).cloned().unwrap_or(Value::Null));
        map.insert("joueur_nom".into(), joueurs.iter().find(|j| row_id(j) == joueur_id).and_then(|j| j.get("nom")).cloned().unwrap_or(Value::Null));
        map.insert("jeu_nom".into(), jeux.iter().find(|j| row_id(j) == jeu_id).and_then(|j| j.get("titre")).cloned().unwrap_or(Value::Null));
        map.insert("employe_nom".into(), users.iter().find(|u| row_id(u) == employe_id).and_then(|u| u.get("nom")).cloned().unwrap_or(Value::Null));
    }
    Ok(o)
}

async fn sessions_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let mut sessions: Vec<Value> = db(&state).query_all("sessions_jeu")?;
        if let Some(s) = q.get("statut") {
            sessions.retain(|x| x.get("statut").and_then(Value::as_str) == Some(s.as_str()));
        }
        sort_desc_by_created_at(&mut sessions);
        let mut out = Vec::with_capacity(sessions.len());
        for s in &sessions {
            out.push(enrich_session(s, &state)?);
        }
        Ok(Value::Array(out))
    })();
    reply(result)
}

async fn sessions_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let s = get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        let statut = s.get("statut").and_then(Value::as_str).unwrap_or("");
        let debut = s.get("debut").and_then(Value::as_str).unwrap_or("");
        let fin = s.get("fin").and_then(Value::as_str).unwrap_or("");
        let duree_minutes = s.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
        let now_ms = chrono::Utc::now().timestamp_millis();
        // Précision seconde : duree_secondes si présent, sinon fallback minutes*60.
        let accum_secondes = s.get("duree_secondes").and_then(Value::as_i64).unwrap_or(duree_minutes * 60);
        let duree_secondes = if statut == "en_cours" {
            // Temps accumulé (secondes exactes) + temps depuis la (re)prise.
            accum_secondes
                + now_ms.checked_sub(parse_iso_ms(debut).unwrap_or(now_ms)).unwrap_or(0) / 1000
        } else if statut == "pause" {
            accum_secondes
        } else {
            parse_iso_ms(fin).and_then(|f| parse_iso_ms(debut).map(|d| (f - d) / 1000)).unwrap_or(0)
        };
        let console_id = s.get("console_id").and_then(Value::as_i64);
        let joueur_id = s.get("joueur_id").and_then(Value::as_i64);
        let jeu_id = s.get("jeu_id").and_then(Value::as_i64);
        let console_nom = d.find_one("consoles", |c| row_id(c) == console_id)?.and_then(|c| c.get("nom").cloned());
        let joueur = d.find_one("joueurs", |j| row_id(j) == joueur_id)?;
        let jeu_nom = d.find_one("jeux", |j| row_id(j) == jeu_id)?.and_then(|j| j.get("titre").cloned());
        let mut o = s.clone();
        if let Value::Object(map) = &mut o {
            map.insert("console_nom".into(), console_nom.unwrap_or(Value::Null));
            map.insert("joueur_nom".into(), joueur.as_ref().and_then(|j| j.get("nom")).cloned().unwrap_or(Value::Null));
            map.insert("joueur_telephone".into(), joueur.as_ref().and_then(|j| j.get("telephone")).cloned().unwrap_or(Value::Null));
            map.insert("jeu_nom".into(), jeu_nom.unwrap_or(Value::Null));
            map.insert("duree_secondes".into(), json!(duree_secondes.max(0)));
        }
        Ok(o)
    })();
    reply(result)
}

async fn sessions_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        let console_id = body.get("console_id").and_then(Value::as_i64).unwrap_or(0);
        let joueur_id = body.get("joueur_id").and_then(Value::as_i64).unwrap_or(0);
        let jeu_id = body.get("jeu_id").and_then(Value::as_i64).unwrap_or(0);
        let tarif_id = body.get("tarif_id").and_then(Value::as_i64);
        if !validators::is_valid_id(console_id) {
            return Err(ApiError::bad_request("Console ID invalide"));
        }
        if !validators::is_valid_id(joueur_id) {
            return Err(ApiError::bad_request("Joueur ID invalide"));
        }
        if !validators::is_valid_id(jeu_id) {
            return Err(ApiError::bad_request("Jeu ID invalide"));
        }
        let d = db(&state);
        let existing = d.find_one("sessions_jeu", |s| {
            s.get("console_id").and_then(Value::as_i64) == Some(console_id)
                && matches!(s.get("statut").and_then(Value::as_str), Some("en_cours") | Some("pause"))
        })?;
        if existing.is_some() {
            return Err(ApiError::bad_request("Cette console a déjà une session en cours"));
        }
        let mut tarif_prix: i64 = 2000;
        let mut duree_minutes: i64 = 60;
        let mut tarif_id_val: Option<i64> = None;
        if let Some(tid) = tarif_id {
            if let Some(tarif) = d.find_one("tarifs", |t| row_id(t) == Some(tid))? {
                tarif_prix = tarif.get("prix").and_then(Value::as_i64).unwrap_or(2000);
                duree_minutes = tarif.get("duree_minutes").and_then(Value::as_i64).unwrap_or(60);
                tarif_id_val = Some(tid);
            }
        }
        let mut row = jmap();
        row.insert("console_id".into(), json!(console_id));
        row.insert("joueur_id".into(), json!(joueur_id));
        row.insert("jeu_id".into(), json!(jeu_id));
        row.insert("employe_id".into(), json!(user.id));
        row.insert("tarif_id".into(), json!(tarif_id_val));
        row.insert("debut".into(), json!(now_iso()));
        row.insert("fin".into(), json!(Value::Null));
        // Même modèle que l'app Tauri : duree_minutes = temps DÉJÀ JOUÉ (0 au
        // démarrage), la durée du tarif va dans duree_allouee.
        row.insert("duree_minutes".into(), json!(0));
        row.insert("duree_secondes".into(), json!(0));
        row.insert("duree_allouee".into(), json!(duree_minutes));
        row.insert("montant".into(), json!(tarif_prix));
        row.insert("tarif_prix".into(), json!(tarif_prix));
        row.insert("jetons_gagnes".into(), json!(0));
        row.insert("statut".into(), json!("en_cours"));
        row.insert("created_at".into(), json!(now_iso()));
        let session = d.insert("sessions_jeu", &row)?;
        let mut upd_console = jmap();
        upd_console.insert("etat".into(), json!("occupee"));
        d.update("consoles", console_id, &upd_console)?;
        let mut upd_joueur = jmap();
        upd_joueur.insert("derniere_visite".into(), json!(now_iso()));
        d.update("joueurs", joueur_id, &upd_joueur)?;
        enrich_session(&session, &state)
    })();
    reply(result)
}

async fn sessions_pause(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let s = get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        if s.get("statut").and_then(Value::as_str) != Some("en_cours") {
            return Err(ApiError::bad_request("Session non en cours"));
        }
        let debut = s.get("debut").and_then(Value::as_str).unwrap_or("");
        // Accumulation en SECONDES : ne plus perdre les secondes à chaque pause.
        let duree_minutes = s.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
        let accum_secondes = s.get("duree_secondes").and_then(Value::as_i64).unwrap_or(duree_minutes * 60);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let elapsed = now_ms.checked_sub(parse_iso_ms(debut).unwrap_or(now_ms)).unwrap_or(0) / 1000;
        let total_secondes = accum_secondes + elapsed;
        let console_id = s.get("console_id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("Console ID manquant"))?;
        let mut upd = jmap();
        upd.insert("statut".into(), json!("pause"));
        upd.insert("duree_minutes".into(), json!(total_secondes / 60));
        upd.insert("duree_secondes".into(), json!(total_secondes));
        d.update("sessions_jeu", id, &upd)?;
        let mut upd_console = jmap();
        upd_console.insert("etat".into(), json!("pause"));
        d.update("consoles", console_id, &upd_console)?;
        get_by_id(&state, "sessions_jeu", id, "Session non trouvée")
    })();
    reply(result)
}

async fn sessions_reprendre(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let s = get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        if s.get("statut").and_then(Value::as_str) != Some("pause") {
            return Err(ApiError::bad_request("Session non en pause"));
        }
        let console_id = s.get("console_id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("Console ID manquant"))?;
        let mut upd = jmap();
        upd.insert("statut".into(), json!("en_cours"));
        upd.insert("debut".into(), json!(now_iso()));
        d.update("sessions_jeu", id, &upd)?;
        let mut upd_console = jmap();
        upd_console.insert("etat".into(), json!("occupee"));
        d.update("consoles", console_id, &upd_console)?;
        get_by_id(&state, "sessions_jeu", id, "Session non trouvée")
    })();
    reply(result)
}

async fn sessions_terminer(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let s = get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        let statut = s.get("statut").and_then(Value::as_str).unwrap_or("");
        let debut = s.get("debut").and_then(Value::as_str).unwrap_or("");
        let duree_minutes = s.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
        // Précision seconde : duree_secondes si présent, sinon fallback minutes*60.
        let accum_secondes = s.get("duree_secondes").and_then(Value::as_i64).unwrap_or(duree_minutes * 60);
        let tarif_prix = s.get("tarif_prix").and_then(Value::as_i64).unwrap_or(2000);
        let console_id = s.get("console_id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("Console ID manquant"))?;
        let joueur_id = s.get("joueur_id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("Joueur ID manquant"))?;
        let session_id = s.get("id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("Session ID manquant"))?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let elapsed = if statut == "en_cours" {
            now_ms.checked_sub(parse_iso_ms(debut).unwrap_or(now_ms)).unwrap_or(0) / 1000
        } else { 0 };
        let total_secondes = accum_secondes + elapsed;
        // Durée facturée à la seconde (plus d'arrondi minutes) ; duree_minutes
        // reste une compat en minutes (ceil).
        let duree_secondes_final = total_secondes.max(1);
        let duree_minutes_final = (duree_secondes_final + 59) / 60;
        // Forfait choisi + dépassement prorata À LA SECONDE (même règle que l'app).
        let allouee = s.get("duree_allouee").and_then(Value::as_i64).unwrap_or(0);
        let montant = compute_montant(tarif_prix, allouee, duree_secondes_final);

        let mut upd_session = jmap();
        upd_session.insert("statut".into(), json!("terminee"));
        upd_session.insert("fin".into(), json!(now_iso()));
        upd_session.insert("duree_minutes".into(), json!(duree_minutes_final));
        upd_session.insert("duree_secondes".into(), json!(duree_secondes_final));
        upd_session.insert("montant".into(), json!(montant));
        d.update("sessions_jeu", id, &upd_session)?;

        let mut upd_console = jmap();
        upd_console.insert("etat".into(), json!("disponible"));
        d.update("consoles", console_id, &upd_console)?;

        let random: u32 = rand::random::<u32>() % 10000;
        let numero_facture = format!("FAC-{}-{:04}", today_str(), random);
        let montant_ht = ((montant as f64) / 1.2).round() as i64;
        let taux_tva: i64 = 20;
        let montant_tva = montant - montant_ht;
        let now = now_iso();

        let mut fac = jmap();
        fac.insert("numero_facture".into(), json!(numero_facture));
        fac.insert("session_id".into(), json!(session_id));
        fac.insert("joueur_id".into(), json!(joueur_id));
        fac.insert("montant_ht".into(), json!(montant_ht));
        fac.insert("taux_tva".into(), json!(taux_tva));
        fac.insert("montant_tva".into(), json!(montant_tva));
        fac.insert("montant_ttc".into(), json!(montant));
        fac.insert("mode_paiement".into(), json!("especes"));
        fac.insert("statut".into(), json!("payee"));
        fac.insert("date_paiement".into(), json!(now.clone()));
        fac.insert("created_at".into(), json!(now.clone()));
        let facture = d.insert("factures", &fac)?;
        let facture_id = facture.get("id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("Facture ID manquant"))?;

        let console_nom = d.find_one("consoles", |c| row_id(c) == Some(console_id))?.and_then(|c| c.get("nom").cloned());
        let jeu_nom = d.find_one("jeux", |j| row_id(j) == s.get("jeu_id").and_then(Value::as_i64))?.and_then(|j| j.get("titre").cloned());
        let description = format!(
            "Session {} - {} - {}min",
            console_nom.as_ref().map(|v| v.as_str().unwrap_or("").to_string()).unwrap_or_default(),
            jeu_nom.as_ref().map(|v| v.as_str().unwrap_or("").to_string()).unwrap_or_default(),
            duree_minutes_final
        );
        let description = validators::sanitize_input(&description, 500);
        let mut ligne = jmap();
        ligne.insert("facture_id".into(), json!(facture_id));
        ligne.insert("description".into(), json!(description));
        ligne.insert("quantite".into(), json!(1));
        ligne.insert("prix_unitaire".into(), json!(montant));
        ligne.insert("total_ligne".into(), json!(montant));
        d.insert("lignes_facture", &ligne)?;

        let mut jetons_gagnes: i64 = 0;
        if let Some(regle) = d.find_one("parametres_fidelite", |r| {
            let a = r.get("actif").map(|v| !matches!(v, Value::Null)).unwrap_or(false);
            a
        })? {
            let jetons = regle.get("jetons_attribues").and_then(Value::as_i64).unwrap_or(1);
            match regle.get("regle_type").and_then(Value::as_str) {
                Some("temps") => {
                    let seuil = regle.get("seuil").and_then(Value::as_i64).unwrap_or(60);
                    jetons_gagnes = (duree_minutes_final / seuil) * jetons;
                }
                // Règle 'montant' : bonus selon le montant de la session
                // (seuil = montant en FC, ex. tous les 5000 FC -> +N jetons).
                Some("montant") => {
                    let seuil = regle.get("seuil").and_then(Value::as_i64).unwrap_or(0);
                    if seuil > 0 && montant >= seuil {
                        jetons_gagnes = (montant / seuil) * jetons;
                    }
                }
                _ => {}
            }
        }
        if jetons_gagnes > 0 {
            let joueur = get_by_id(&state, "joueurs", joueur_id, "Joueur non trouvé")?;
            let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);
            let mut upd_j = jmap();
            upd_j.insert("jetons_solde".into(), json!(solde + jetons_gagnes));
            d.update("joueurs", joueur_id, &upd_j)?;
            let raison = format!("Session {}min - {}", duree_minutes_final, {
                console_nom.as_ref().map(|v| v.as_str().unwrap_or("").to_string()).unwrap_or_default()
            });
            let raison = validators::sanitize_input(&raison, 500);
            let mut jtx = jmap();
            jtx.insert("joueur_id".into(), json!(joueur_id));
            jtx.insert("type".into(), json!("gain"));
            jtx.insert("quantite".into(), json!(jetons_gagnes));
            jtx.insert("raison".into(), json!(raison));
            jtx.insert("session_id".into(), json!(session_id));
            jtx.insert("created_at".into(), json!(now.clone()));
            d.insert("jetons_transactions", &jtx)?;
        }

        let joueur = get_by_id(&state, "joueurs", joueur_id, "Joueur non trouvé")?;
        let lignes: Vec<Value> = d.query_all("lignes_facture")?.into_iter()
            .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(facture_id)).collect();
        let mut enriched_facture = facture.clone();
        if let Value::Object(map) = &mut enriched_facture {
            map.insert("joueur_nom".into(), joueur.get("nom").cloned().unwrap_or(Value::Null));
            map.insert("lignes".into(), json!(lignes));
        }
        let session_final = get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        Ok(json!({
            "session": session_final,
            "facture": enriched_facture,
            "montant": montant,
            "jetonsGagnes": jetons_gagnes,
            "dureeMinutes": duree_minutes_final,
        }))
    })();
    reply(result)
}

async fn sessions_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        let mut updates: Map<String, Value> = jmap();
        if let Some(c) = body.get("console_id").and_then(Value::as_i64) {
            if !validators::is_valid_id(c) {
                return Err(ApiError::bad_request("Console ID invalide"));
            }
            updates.insert("console_id".into(), json!(c));
        }
        if let Some(j) = body.get("joueur_id").and_then(Value::as_i64) {
            if !validators::is_valid_id(j) {
                return Err(ApiError::bad_request("Joueur ID invalide"));
            }
            updates.insert("joueur_id".into(), json!(j));
        }
        if let Some(j) = body.get("jeu_id").and_then(Value::as_i64) {
            if !validators::is_valid_id(j) {
                return Err(ApiError::bad_request("Jeu ID invalide"));
            }
            updates.insert("jeu_id".into(), json!(j));
        }
        if let Some(s) = body.get("statut").and_then(Value::as_str) {
            if !validators::is_valid_session_statut(s) {
                return Err(ApiError::bad_request("Statut invalide"));
            }
            updates.insert("statut".into(), json!(s));
        }
        if let Some(d) = body.get("duree_minutes").and_then(Value::as_i64) {
            if !validators::is_valid_duree(d) {
                return Err(ApiError::bad_request("Durée invalide (1-1000)"));
            }
            // Session ACTIVE : la durée éditable = temps ALLOUÉ (comme l'app).
            // Terminée : durée facturée (compat minutes + secondes).
            let statut = get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?
                .get("statut").and_then(Value::as_str).unwrap_or("").to_string();
            if statut == "en_cours" || statut == "pause" {
                updates.insert("duree_allouee".into(), json!(d));
            } else {
                updates.insert("duree_minutes".into(), json!(d));
                updates.insert("duree_secondes".into(), json!(d * 60));
                updates.insert("duree_allouee".into(), json!(d));
            }
        }
        if let Some(m) = body.get("montant").and_then(Value::as_f64) {
            if !validators::is_valid_prix(m) {
                return Err(ApiError::bad_request("Montant invalide (1-1000000)"));
            }
            updates.insert("montant".into(), json!(m));
        }
        db(&state).update("sessions_jeu", id, &updates)
    })();
    reply(result)
}

async fn sessions_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "sessions_jeu", id, "Session non trouvée")?;
        db(&state).remove("sessions_jeu", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Factures =====

fn enrich_facture(f: &Value, joueur: Option<&Value>) -> Value {
    let mut o = f.clone();
    if let Value::Object(map) = &mut o {
        map.insert("joueur_nom".into(), joueur.and_then(|j| j.get("nom")).cloned().unwrap_or_else(|| json!("N/A")));
    }
    o
}

async fn factures_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let d = db(&state);
        let mut factures: Vec<Value> = d.query_all("factures")?;
        if let Some(s) = q.get("statut") {
            factures.retain(|f| f.get("statut").and_then(Value::as_str) == Some(s.as_str()));
        }
        if let Some(j) = q.get("joueur_id").and_then(|s| s.parse::<i64>().ok()) {
            factures.retain(|f| f.get("joueur_id").and_then(Value::as_i64) == Some(j));
        }
        if let Some(ds) = q.get("date_start") {
            factures.retain(|f| f.get("created_at").and_then(Value::as_str).map(|c| c >= ds.as_str()).unwrap_or(false));
        }
        if let Some(de) = q.get("date_end") {
            let end = format!("{}T23:59:59", de);
            factures.retain(|f| f.get("created_at").and_then(Value::as_str).map(|c| c <= end.as_str()).unwrap_or(false));
        }
        sort_desc_by_created_at(&mut factures);
        let joueurs = d.query_all("joueurs")?;
        Ok(Value::Array(factures.iter().map(|f| {
            let joueur = f.get("joueur_id").and_then(Value::as_i64).and_then(|jid| joueurs.iter().find(|j| row_id(j) == Some(jid)));
            enrich_facture(f, joueur)
        }).collect()))
    })();
    reply(result)
}

async fn factures_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let f = get_by_id(&state, "factures", id, "Facture non trouvée")?;
        let joueur = f.get("joueur_id").and_then(Value::as_i64).and_then(|jid| d.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
        let lignes: Vec<Value> = d.query_all("lignes_facture")?.into_iter().filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id)).collect();
        let mut o = enrich_facture(&f, joueur.as_ref());
        if let Value::Object(map) = &mut o {
            map.insert("joueur_telephone".into(), joueur.as_ref().and_then(|j| j.get("telephone")).cloned().unwrap_or(Value::Null));
            map.insert("lignes".into(), json!(lignes));
        }
        Ok(o)
    })();
    reply(result)
}

async fn factures_pdf(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        let f = get_by_id(&state, "factures", id, "Facture non trouvée")?;
        let joueur = f.get("joueur_id").and_then(Value::as_i64).and_then(|jid| d.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
        let lignes: Vec<Value> = d.query_all("lignes_facture")?.into_iter().filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id)).collect();
        let pdf = game_lounge_rust_lib::pdf::facture_pdf(&f, joueur.as_ref(), &lignes)
            .map_err(|e| ApiError::internal(format!("Erreur génération PDF: {e}")))?;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, pdf);
        Ok(json!({ "pdf_base64": b64 }))
    })();
    reply(result)
}

async fn factures_annuler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        get_by_id(&state, "factures", id, "Facture non trouvée")?;
        let mut upd = jmap();
        upd.insert("statut".into(), json!("annulee"));
        let updated = d.update("factures", id, &upd)?;
        let joueur = updated.get("joueur_id").and_then(Value::as_i64).and_then(|jid| d.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
        Ok(enrich_facture(&updated, joueur.as_ref()))
    })();
    reply(result)
}

async fn factures_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let session_id = body.get("session_id").and_then(Value::as_i64).unwrap_or(0);
        let joueur_id = body.get("joueur_id").and_then(Value::as_i64).unwrap_or(0);
        let montant_ht = body.get("montant_ht").and_then(Value::as_f64);
        let taux_tva = body.get("taux_tva").and_then(Value::as_f64);
        let montant_tva = body.get("montant_tva").and_then(Value::as_f64);
        let montant_ttc = body.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0);
        let mode_paiement = body.get("mode_paiement").and_then(Value::as_str).map(|s| s.to_string());
        let statut = body.get("statut").and_then(Value::as_str).map(|s| s.to_string());
        if !validators::is_valid_id(session_id) {
            return Err(ApiError::bad_request("Session ID invalide"));
        }
        if !validators::is_valid_id(joueur_id) {
            return Err(ApiError::bad_request("Joueur ID invalide"));
        }
        if !validators::is_valid_prix(montant_ttc) {
            return Err(ApiError::bad_request("Montant TTC invalide (1-1000000)"));
        }
        if let Some(ht) = montant_ht {
            if ht != 0.0 && !validators::is_valid_prix(ht) {
                return Err(ApiError::bad_request("Montant HT invalide"));
            }
        }
        if let Some(s) = statut.as_deref() {
            if !validators::is_valid_facture_statut(s) {
                return Err(ApiError::bad_request("Statut invalide"));
            }
        }
        if let Some(m) = mode_paiement.as_deref() {
            if !validators::is_valid_mode_paiement(m) {
                return Err(ApiError::bad_request("Mode paiement invalide"));
            }
        }
        let random: u32 = rand::random::<u32>() % 10000;
        let now = now_iso();
        let mut row = jmap();
        row.insert("numero_facture".into(), json!(format!("FAC-{}-{:04}", today_str(), random)));
        row.insert("session_id".into(), json!(session_id));
        row.insert("joueur_id".into(), json!(joueur_id));
        row.insert("montant_ht".into(), json!(montant_ht.unwrap_or(0.0)));
        row.insert("taux_tva".into(), json!(taux_tva.unwrap_or(20.0)));
        row.insert("montant_tva".into(), json!(montant_tva.unwrap_or(0.0)));
        row.insert("montant_ttc".into(), json!(montant_ttc));
        row.insert("mode_paiement".into(), json!(mode_paiement.unwrap_or_else(|| "especes".into())));
        row.insert("statut".into(), json!(statut.unwrap_or_else(|| "payee".into())));
        row.insert("date_paiement".into(), json!(now.clone()));
        row.insert("created_at".into(), json!(now));
        db(&state).insert("factures", &row)
    })();
    reply(result)
}

async fn factures_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        let is_admin = user.is_admin();
        if !is_admin {
            if user.role != "employe" {
                return Err(ApiError::forbidden("Accès réservé aux administrateurs et employés"));
            }
            if body.get("statut").is_some() || body.get("montant_ttc").is_some() || body.get("mode_paiement").and_then(Value::as_str).is_none() {
                return Err(ApiError::forbidden("Un employé peut uniquement modifier le mode de paiement"));
            }
        }
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "factures", id, "Facture non trouvée")?;
        let mut updates = jmap();
        if is_admin {
            if let Some(s) = body.get("statut").and_then(Value::as_str) {
                if !validators::is_valid_facture_statut(s) {
                    return Err(ApiError::bad_request("Statut invalide"));
                }
                updates.insert("statut".into(), json!(s));
            }
        }
        if let Some(m) = body.get("mode_paiement").and_then(Value::as_str) {
            if !validators::is_valid_mode_paiement(m) {
                return Err(ApiError::bad_request("Mode paiement invalide"));
            }
            updates.insert("mode_paiement".into(), json!(m));
        }
        if is_admin {
            if let Some(m) = body.get("montant_ttc").and_then(Value::as_f64) {
                if !validators::is_valid_prix(m) {
                    return Err(ApiError::bad_request("Montant invalide (1-1000000)"));
                }
                updates.insert("montant_ttc".into(), json!(m));
            }
        }
        db(&state).update("factures", id, &updates)
    })();
    reply(result)
}

async fn factures_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "factures", id, "Facture non trouvée")?;
        db(&state).remove("factures", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Lignes facture =====

async fn lignes_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let mut lignes = db(&state).query_all("lignes_facture")?;
        if let Some(fid) = q.get("facture_id").and_then(|s| s.parse::<i64>().ok()) {
            lignes.retain(|l| l.get("facture_id").and_then(Value::as_i64) == Some(fid));
        }
        Ok(Value::Array(lignes))
    })();
    reply(result)
}

async fn lignes_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "lignes_facture", id, "Ligne non trouvée")
    })();
    reply(result)
}

async fn lignes_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let facture_id = body.get("facture_id").and_then(Value::as_i64).unwrap_or(0);
        let description = body.get("description").and_then(Value::as_str).unwrap_or("");
        let quantite = body.get("quantite").and_then(Value::as_i64).unwrap_or(0);
        let prix_unitaire = body.get("prix_unitaire").and_then(Value::as_f64).unwrap_or(0.0);
        let total_ligne = body.get("total_ligne").and_then(Value::as_f64);
        if !validators::is_valid_id(facture_id) {
            return Err(ApiError::bad_request("Facture ID invalide"));
        }
        if !validators::is_valid_description(description) {
            return Err(ApiError::bad_request("Description invalide (2-500 caractères)"));
        }
        if !validators::is_valid_quantite(quantite) {
            return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
        }
        if !validators::is_valid_prix(prix_unitaire) {
            return Err(ApiError::bad_request("Prix unitaire invalide (1-1000000)"));
        }
        if let Some(t) = total_ligne {
            if !validators::is_valid_prix(t) {
                return Err(ApiError::bad_request("Total invalide"));
            }
        }
        let mut row = jmap();
        row.insert("facture_id".into(), json!(facture_id));
        row.insert("description".into(), json!(validators::sanitize_input(description, 500)));
        row.insert("quantite".into(), json!(quantite));
        row.insert("prix_unitaire".into(), json!(prix_unitaire));
        row.insert("total_ligne".into(), json!(total_ligne.unwrap_or(prix_unitaire * quantite as f64)));
        db(&state).insert("lignes_facture", &row)
    })();
    reply(result)
}

async fn lignes_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "lignes_facture", id, "Ligne non trouvée")?;
        let mut updates = jmap();
        if let Some(d) = body.get("description").and_then(Value::as_str) {
            if !validators::is_valid_description(d) {
                return Err(ApiError::bad_request("Description invalide (2-500 caractères)"));
            }
            updates.insert("description".into(), json!(validators::sanitize_input(d, 500)));
        }
        if let Some(q) = body.get("quantite").and_then(Value::as_i64) {
            if !validators::is_valid_quantite(q) {
                return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
            }
            updates.insert("quantite".into(), json!(q));
        }
        if let Some(p) = body.get("prix_unitaire").and_then(Value::as_f64) {
            if !validators::is_valid_prix(p) {
                return Err(ApiError::bad_request("Prix unitaire invalide (1-1000000)"));
            }
            updates.insert("prix_unitaire".into(), json!(p));
        }
        if let Some(t) = body.get("total_ligne").and_then(Value::as_f64) {
            if !validators::is_valid_prix(t) {
                return Err(ApiError::bad_request("Total invalide"));
            }
            updates.insert("total_ligne".into(), json!(t));
        }
        db(&state).update("lignes_facture", id, &updates)
    })();
    reply(result)
}

async fn lignes_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "lignes_facture", id, "Ligne non trouvée")?;
        db(&state).remove("lignes_facture", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Tarifs =====

async fn tarifs_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        Ok(Value::Array(db(&state).query_all("tarifs")?))
    })();
    reply(result)
}

async fn tarifs_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "tarifs", id, "Tarif non trouvé")
    })();
    reply(result)
}

async fn tarifs_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let r#type = body.get("type").and_then(Value::as_str).unwrap_or("");
        let duree_minutes = body.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
        let prix = body.get("prix").and_then(Value::as_f64).unwrap_or(0.0);
        let description = body.get("description").and_then(Value::as_str).map(|s| s.to_string());
        let console_type = body.get("console_type").and_then(Value::as_str).map(|s| s.to_string());
        let jeu = body.get("jeu").and_then(Value::as_str).map(|s| s.to_string());
        if r#type.is_empty() || duree_minutes <= 0 || prix <= 0.0 {
            return Err(ApiError::bad_request("Champs requis manquants"));
        }
        if !validators::is_valid_tarif_type(r#type) {
            return Err(ApiError::bad_request("Type de tarif invalide"));
        }
        if !validators::is_valid_duree(duree_minutes) {
            return Err(ApiError::bad_request("Durée invalide (1-1000)"));
        }
        if !validators::is_valid_prix(prix) {
            return Err(ApiError::bad_request("Prix invalide (1-1000000)"));
        }
        let mut row = jmap();
        row.insert("type".into(), json!(r#type));
        row.insert("duree_minutes".into(), json!(duree_minutes));
        row.insert("prix".into(), json!(prix));
        row.insert("description".into(), json!(description.map(|d| validators::sanitize_input(&d, 500)).unwrap_or_default()));
        row.insert("actif".into(), json!(1));
        row.insert("console_type".into(), json!(console_type));
        row.insert("jeu".into(), json!(jeu));
        row.insert("created_at".into(), json!(now_iso()));
        db(&state).insert("tarifs", &row)
    })();
    reply(result)
}

async fn tarifs_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "tarifs", id, "Tarif non trouvé")?;
        let mut updates = jmap();
        if let Some(t) = body.get("type").and_then(Value::as_str) {
            if !validators::is_valid_tarif_type(t) {
                return Err(ApiError::bad_request("Type de tarif invalide"));
            }
            updates.insert("type".into(), json!(t));
        }
        if let Some(d) = body.get("duree_minutes").and_then(Value::as_i64) {
            if !validators::is_valid_duree(d) {
                return Err(ApiError::bad_request("Durée invalide (1-1000)"));
            }
            updates.insert("duree_minutes".into(), json!(d));
        }
        if let Some(p) = body.get("prix").and_then(Value::as_f64) {
            if !validators::is_valid_prix(p) {
                return Err(ApiError::bad_request("Prix invalide (1-1000000)"));
            }
            updates.insert("prix".into(), json!(p));
        }
        if let Some(d) = body.get("description").and_then(Value::as_str) {
            updates.insert("description".into(), json!(if d.is_empty() { String::new() } else { validators::sanitize_input(d, 500) }));
        }
        if let Some(a) = body.get("actif").and_then(Value::as_bool) {
            updates.insert("actif".into(), json!(if a { 1 } else { 0 }));
        }
        if let Some(ct) = body.get("console_type").and_then(Value::as_str) {
            updates.insert("console_type".into(), json!(ct));
        }
        if let Some(j) = body.get("jeu").and_then(Value::as_str) {
            updates.insert("jeu".into(), json!(j));
        }
        db(&state).update("tarifs", id, &updates)
    })();
    reply(result)
}

async fn tarifs_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        db(&state).remove("tarifs", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Jetons =====

async fn jetons_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let d = db(&state);
        let mut transactions: Vec<Value> = d.query_all("jetons_transactions")?;
        if let Some(j) = q.get("joueur_id").and_then(|s| s.parse::<i64>().ok()) {
            transactions.retain(|t| t.get("joueur_id").and_then(Value::as_i64) == Some(j));
        }
        sort_desc_by_created_at(&mut transactions);
        let joueurs = d.query_all("joueurs")?;
        Ok(Value::Array(transactions.iter().map(|t| {
            let joueur = t.get("joueur_id").and_then(Value::as_i64).and_then(|jid| joueurs.iter().find(|j| row_id(j) == Some(jid)));
            let mut o = t.clone();
            if let Value::Object(map) = &mut o {
                map.insert("joueur_nom".into(), joueur.and_then(|j| j.get("nom")).cloned().unwrap_or(Value::Null));
            }
            o
        }).collect()))
    })();
    reply(result)
}

async fn jetons_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "jetons_transactions", id, "Transaction non trouvée")
    })();
    reply(result)
}

async fn jetons_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let joueur_id = body.get("joueur_id").and_then(Value::as_i64).unwrap_or(0);
        let r#type = body.get("type").and_then(Value::as_str).unwrap_or("");
        let quantite = body.get("quantite").and_then(Value::as_i64).unwrap_or(0);
        let raison = body.get("raison").and_then(Value::as_str).map(|s| s.to_string());
        let session_id = body.get("session_id").and_then(Value::as_i64);
        if !validators::is_valid_id(joueur_id) {
            return Err(ApiError::bad_request("Joueur ID invalide"));
        }
        if !validators::is_valid_jeton_type(r#type) {
            return Err(ApiError::bad_request("Type invalide"));
        }
        if !validators::is_valid_quantite(quantite) {
            return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
        }
        if let Some(s) = session_id {
            if !validators::is_valid_id(s) {
                return Err(ApiError::bad_request("Session ID invalide"));
            }
        }
        let d = db(&state);
        let joueur = get_by_id(&state, "joueurs", joueur_id, "Joueur non trouvé")?;
        let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);
        let mut row = jmap();
        row.insert("joueur_id".into(), json!(joueur_id));
        row.insert("type".into(), json!(r#type.to_string()));
        row.insert("quantite".into(), json!(quantite));
        row.insert("raison".into(), json!(raison.map(|r| validators::sanitize_input(&r, 500)).unwrap_or_default()));
        row.insert("session_id".into(), json!(session_id));
        row.insert("created_at".into(), json!(now_iso()));
        let transaction = d.insert("jetons_transactions", &row)?;
        let mut upd = jmap();
        let new_solde = match r#type {
            "gain" | "bonus" => solde + quantite,
            "depense" => (solde - quantite).max(0),
            _ => solde,
        };
        upd.insert("jetons_solde".into(), json!(new_solde));
        d.update("joueurs", joueur_id, &upd)?;
        Ok(transaction)
    })();
    reply(result)
}

async fn jetons_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "jetons_transactions", id, "Transaction non trouvée")?;
        let mut updates = jmap();
        if let Some(t) = body.get("type").and_then(Value::as_str) {
            if !validators::is_valid_jeton_type(t) {
                return Err(ApiError::bad_request("Type invalide"));
            }
            updates.insert("type".into(), json!(t));
        }
        if let Some(q) = body.get("quantite").and_then(Value::as_i64) {
            if !validators::is_valid_quantite(q) {
                return Err(ApiError::bad_request("Quantité invalide (1-10000)"));
            }
            updates.insert("quantite".into(), json!(q));
        }
        if let Some(r) = body.get("raison").and_then(Value::as_str) {
            updates.insert("raison".into(), json!(validators::sanitize_input(r, 500)));
        }
        db(&state).update("jetons_transactions", id, &updates)
    })();
    reply(result)
}

async fn jetons_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "jetons_transactions", id, "Transaction non trouvée")?;
        db(&state).remove("jetons_transactions", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Messages =====

async fn messages_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let mut messages = db(&state).query_all("messages")?;
        messages.sort_by(|a, b| {
            let ca = a.get("created_at").and_then(Value::as_str).unwrap_or("");
            let cb = b.get("created_at").and_then(Value::as_str).unwrap_or("");
            cb.cmp(ca)
        });
        Ok(Value::Array(messages))
    })();
    reply(result)
}

async fn messages_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        let titre = body.get("titre").and_then(Value::as_str).map(|s| s.to_string());
        let contenu = body.get("contenu").and_then(Value::as_str).unwrap_or("");
        if contenu.trim().is_empty() {
            return Err(ApiError::bad_request("Contenu requis"));
        }
        if !validators::is_valid_contenu(contenu) {
            return Err(ApiError::bad_request("Contenu invalide (1-1000 caractères)"));
        }
        let mut row = jmap();
        if let Some(t) = titre {
            row.insert("titre".into(), json!(validators::sanitize_input(&t, 100)));
        } else {
            row.insert("titre".into(), json!(Value::Null));
        }
        let contenu = contenu.trim().chars().take(1000).collect::<String>();
        row.insert("contenu".into(), json!(contenu));
        row.insert("auteur".into(), json!(user.nom));
        row.insert("created_at".into(), json!(now_iso()));
        db(&state).insert("messages", &row)
    })();
    reply(result)
}

async fn messages_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "messages", id, "Message non trouvé")?;
        let mut updates = jmap();
        if let Some(t) = body.get("titre").and_then(Value::as_str) {
            updates.insert("titre".into(), json!(validators::sanitize_input(t, 100)));
        }
        if let Some(c) = body.get("contenu").and_then(Value::as_str) {
            updates.insert("contenu".into(), json!(c.trim().chars().take(1000).collect::<String>()));
        }
        db(&state).update("messages", id, &updates)
    })();
    reply(result)
}

async fn messages_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        db(&state).remove("messages", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Paramètres fidélité =====

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

async fn fidelite_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let rule = db(&state)
            .find_one("parametres_fidelite", |r| is_active_flag(r.get("actif")))?
            .unwrap_or_else(default_rule);
        Ok(rule)
    })();
    reply(result)
}

async fn fidelite_get_by_id(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "parametres_fidelite", id, "Paramètre non trouvé")
    })();
    reply(result)
}

async fn fidelite_put(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let regle_type = body.get("regle_type").and_then(Value::as_str).map(|s| s.to_string());
        let seuil = body.get("seuil").and_then(Value::as_i64);
        let jetons_attribues = body.get("jetons_attribues").and_then(Value::as_i64);
        let valeur_jeton = body.get("valeur_jeton").and_then(Value::as_i64);
        let actif = body.get("actif").and_then(Value::as_bool);
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
        let d = db(&state);
        let existing = d.find_one("parametres_fidelite", |_| true)?;
        match existing {
            Some(existing_row) => {
                let id = existing_row.get("id").and_then(Value::as_i64).ok_or_else(|| ApiError::internal("ID fidélité manquant"))?;
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
                d.update("parametres_fidelite", id, &updates)
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
                d.insert("parametres_fidelite", &row)
            }
        }
    })();
    reply(result)
}

async fn fidelite_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let regle_type = body.get("regle_type").and_then(Value::as_str).unwrap_or("");
        let seuil = body.get("seuil").and_then(Value::as_i64).unwrap_or(0);
        let jetons_attribues = body.get("jetons_attribues").and_then(Value::as_i64).unwrap_or(0);
        let valeur_jeton = body.get("valeur_jeton").and_then(Value::as_i64).unwrap_or(0);
        let actif = body.get("actif").and_then(Value::as_bool);
        if regle_type.is_empty() || seuil <= 0 || jetons_attribues <= 0 || valeur_jeton <= 0 {
            return Err(ApiError::bad_request("Champs requis manquants"));
        }
        if !validators::is_valid_regle_type(regle_type) {
            return Err(ApiError::bad_request("Type de règle invalide"));
        }
        if !validators::is_valid_seuil(seuil) {
            return Err(ApiError::bad_request("Seuil invalide (1-10000)"));
        }
        if !validators::is_valid_jetons_attribues(jetons_attribues) {
            return Err(ApiError::bad_request("Jetons attribués invalides (1-1000)"));
        }
        if !(1..=1_000_000).contains(&valeur_jeton) {
            return Err(ApiError::bad_request("Valeur d'un jeton invalide (1-1000000 FC)"));
        }
        let mut row = jmap();
        row.insert("regle_type".into(), json!(regle_type));
        row.insert("seuil".into(), json!(seuil));
        row.insert("jetons_attribues".into(), json!(jetons_attribues));
        row.insert("valeur_jeton".into(), json!(valeur_jeton));
        row.insert("actif".into(), json!(if actif.unwrap_or(true) { 1 } else { 0 }));
        db(&state).insert("parametres_fidelite", &row)
    })();
    reply(result)
}

async fn fidelite_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "parametres_fidelite", id, "Paramètre non trouvé")?;
        db(&state).remove("parametres_fidelite", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Rapports =====

async fn rapports_ca(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        let d = db(&state);
        let factures: Vec<Value> = d.query_all("factures")?.into_iter()
            .filter(|f| f.get("statut").and_then(Value::as_str) == Some("payee"))
            .collect();
        let sessions = d.query_all("sessions_jeu")?;
        let joueurs = d.query_all("joueurs")?;
        let jeux = d.query_all("jeux")?;
        let consoles = d.query_all("consoles")?;
        let jetons_tx = d.query_all("jetons_transactions")?;

        let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let date_prefix = |row: &Value, key: &str| -> String {
            row.get(key).and_then(Value::as_str).map(|s| s.chars().take(10).collect()).unwrap_or_default()
        };
        let today_factures: Vec<&Value> = factures.iter().filter(|f| date_prefix(f, "date_paiement") == today).collect();
        let today_sessions: Vec<&Value> = sessions.iter().filter(|s| date_prefix(s, "created_at") == today).collect();
        let total_revenus: f64 = factures.iter().map(|f| f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0)).sum();
        let total_sessions = sessions.len();

        let mut top_jeux: Vec<Value> = jeux.iter().map(|j| {
            let jid = row_id(j);
            let count = sessions.iter().filter(|s| s.get("jeu_id").and_then(Value::as_i64) == jid).count();
            let pct = if total_sessions > 0 { ((count as f64) / (total_sessions as f64) * 100.0).round() as i64 } else { 0 };
            json!({ "titre": j["titre"], "sessions": count, "pct": pct })
        }).collect();
        top_jeux.sort_by(|a, b| {
            b.get("sessions").and_then(Value::as_i64).unwrap_or(0)
                .cmp(&a.get("sessions").and_then(Value::as_i64).unwrap_or(0))
        });
        top_jeux.truncate(5);

        let mut repartition_consoles: Vec<Value> = consoles.iter().map(|c| {
            let cid = row_id(c);
            let count = sessions.iter().filter(|s| s.get("console_id").and_then(Value::as_i64) == cid).count();
            let pct = if total_sessions > 0 { ((count as f64) / (total_sessions as f64) * 100.0).round() as i64 } else { 0 };
            json!({ "nom": c["nom"], "sessions": count, "pct": pct })
        }).collect();
        repartition_consoles.sort_by(|a, b| {
            b.get("sessions").and_then(Value::as_i64).unwrap_or(0)
                .cmp(&a.get("sessions").and_then(Value::as_i64).unwrap_or(0))
        });

        let mut sessions_history: Vec<Value> = Vec::new();
        for i in (0..7).rev() {
            let ddate = chrono::Utc::now().date_naive() - chrono::Duration::days(i);
            let ds = ddate.format("%Y-%m-%d").to_string();
            let count = sessions.iter().filter(|s| date_prefix(s, "created_at") == ds).count();
            let revenus: f64 = factures.iter()
                .filter(|f| date_prefix(f, "date_paiement") == ds)
                .map(|f| f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0))
                .sum();
            sessions_history.push(json!({
                "date": ds[5..].to_string(),
                "count": count,
                "revenus": revenus,
            }));
        }

        let jetons_attribues_auj: i64 = jetons_tx.iter()
            .filter(|t| date_prefix(t, "created_at") == today && t.get("type").and_then(Value::as_str) == Some("gain"))
            .map(|t| t.get("quantite").and_then(Value::as_i64).unwrap_or(0))
            .sum();

        let joueurs_actifs_auj = {
            let mut set: Vec<i64> = Vec::new();
            for s in &today_sessions {
                if let Some(j) = s.get("joueur_id").and_then(Value::as_i64) {
                    if !set.contains(&j) {
                        set.push(j);
                    }
                }
            }
            set.len()
        };

        Ok(json!({
            "revenus_aujourd_hui": today_factures.iter().map(|f| f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0)).sum::<f64>(),
            "sessions_aujourd_hui": today_sessions.len(),
            "joueurs_actifs": joueurs_actifs_auj,
            "jetons_attribues": jetons_attribues_auj,
            "total_revenus": total_revenus,
            "total_sessions": total_sessions,
            "total_joueurs": joueurs.len(),
            "top_jeux": top_jeux,
            "repartition_consoles": repartition_consoles,
            "sessions_history": sessions_history,
        }))
    })();
    reply(result)
}

// ===== Users (admin) =====

fn to_public(row: &Value) -> Value {
    json!({
        "id": row["id"],
        "email": row["email"],
        "role": row["role"],
        "nom": row["nom"],
        "created_at": row["created_at"],
    })
}

async fn users_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let users = db(&state).query_all("users")?;
        Ok(Value::Array(users.iter().map(to_public).collect()))
    })();
    reply(result)
}

async fn users_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let row = get_by_id(&state, "users", id, "Utilisateur non trouvé")?;
        Ok(to_public(&row))
    })();
    reply(result)
}

async fn users_create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let email = body.get("email").and_then(Value::as_str).unwrap_or("");
        let password = body.get("password").and_then(Value::as_str).unwrap_or("");
        let role = body.get("role").and_then(Value::as_str).unwrap_or("");
        let nom = body.get("nom").and_then(Value::as_str).unwrap_or("");
        if email.is_empty() || password.is_empty() || role.is_empty() || nom.is_empty() {
            return Err(ApiError::bad_request("Nom, email, mot de passe et rôle requis"));
        }
        if !validators::is_valid_nom(nom) {
            return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
        }
        if !validators::is_valid_email(email) {
            return Err(ApiError::bad_request("Email invalide"));
        }
        if !validators::is_valid_password(password) {
            return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)"));
        }
        if !validators::is_valid_role(role) {
            return Err(ApiError::bad_request("Rôle invalide"));
        }
        let d = db(&state);
        // FIX utilisateurs fantômes (comme l'app Tauri) : réactive une ligne
        // soft-deleted au lieu d'échouer sur UNIQUE(email) -> plus de batch de
        // sync perdu ni de compte fantôme côté cloud.
        if let Some(prev) = d.find_one_all("users", |u| {
            u.get("email").and_then(Value::as_str) == Some(email)
                && u.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1
        })? {
            let prev_id = prev.get("id").and_then(Value::as_i64).unwrap_or(0);
            if prev_id > 0 {
                let hash = auth::hash_password(password)?;
                let mut upd = jmap();
                upd.insert("deleted".into(), json!(0));
                upd.insert("password_hash".into(), json!(hash));
                upd.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
                upd.insert("role".into(), json!(role));
                upd.insert("created_at".into(), json!(now_iso()));
                let revived = d.update("users", prev_id, &upd)?;
                return Ok(to_public(&revived));
            }
        }
        if d.find_one("users", |u| u.get("email").and_then(Value::as_str) == Some(email))?.is_some() {
            return Err(ApiError::new(409, "Email déjà utilisé"));
        }
        let hash = auth::hash_password(password)?;
        let mut row = jmap();
        row.insert("email".into(), json!(validators::sanitize_input(email, 100)));
        row.insert("password_hash".into(), json!(hash));
        row.insert("nom".into(), json!(validators::sanitize_input(nom, 50)));
        row.insert("role".into(), json!(role));
        row.insert("created_at".into(), json!(now_iso()));
        let created = d.insert("users", &row)?;
        Ok(to_public(&created))
    })();
    reply(result)
}

async fn users_update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let current = claims(&state, &token)?;
        admin_only(&current)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        let d = db(&state);
        get_by_id(&state, "users", id, "Utilisateur non trouvé")?;
        let mut updates = jmap();
        if let Some(e) = body.get("email").and_then(Value::as_str) {
            if !validators::is_valid_email(e) {
                return Err(ApiError::bad_request("Email invalide"));
            }
            updates.insert("email".into(), json!(validators::sanitize_input(e, 100)));
        }
        if let Some(r) = body.get("role").and_then(Value::as_str) {
            if !validators::is_valid_role(r) {
                return Err(ApiError::bad_request("Rôle invalide"));
            }
            updates.insert("role".into(), json!(r));
        }
        if let Some(n) = body.get("nom").and_then(Value::as_str) {
            if !validators::is_valid_nom(n) {
                return Err(ApiError::bad_request("Nom invalide (2-50 caractères)"));
            }
            updates.insert("nom".into(), json!(validators::sanitize_input(n, 50)));
        }
        if let Some(p) = body.get("password").and_then(Value::as_str) {
            if !p.is_empty() {
                if !validators::is_valid_password(p) {
                    return Err(ApiError::bad_request("Mot de passe invalide (min 6 caractères, au moins une lettre)"));
                }
                let hash = auth::hash_password(p)?;
                updates.insert("password_hash".into(), json!(hash));
            }
        }
        let updated = d.update("users", id, &updates)?;
        Ok(to_public(&updated))
    })();
    reply(result)
}

async fn users_delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let current = claims(&state, &token)?;
        admin_only(&current)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        if id == current.id {
            return Err(ApiError::bad_request("Impossible de supprimer votre propre compte"));
        }
        let d = db(&state);
        get_by_id(&state, "users", id, "Utilisateur non trouvé")?;
        d.remove("users", id)?;
        Ok(json!({ "success": true }))
    })();
    reply(result)
}

// ===== Sync =====

fn local_status(state: &AppState) -> ApiResult<Value> {
    let has_local = !db(state).query_all("users")?.is_empty();
    Ok(json!({
        "supabaseEnabled": false,
        "supabaseAvailable": false,
        "hasLocalData": has_local,
    }))
}

async fn sync_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        local_status(&state)
    })();
    reply(result)
}

async fn sync_toggle(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        let base = local_status(&state)?;
        let mut o = base;
        if let Value::Object(map) = &mut o {
            map.insert("enabled".into(), json!(enabled));
        }
        Ok(o)
    })();
    reply(result)
}

async fn sync_run(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        let user = claims(&state, &token)?;
        admin_only(&user)?;
        let _ = db(&state).query_all("users")?;
        Ok(json!({
            "success": true,
            "message": "Synchronisation locale terminée (Supabase non configuré)"
        }))
    })();
    reply(result)
}

async fn sync_poll(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        Ok(json!({
            "changes": {},
            "timestamp": now_iso(),
        }))
    })();
    reply(result)
}

// ===== Serveur =====

fn seed_default_users(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("GL_SEED_DEMO").as_deref() != Ok("1") {
        return Ok(());
    }
    if !db(state).query_all("users")?.is_empty() {
        return Ok(());
    }
    let now = now_iso();
    let mut admin = Map::new();
    admin.insert("email".into(), json!("admin@gamelounge.com"));
    admin.insert("password_hash".into(), json!(auth::hash_password("admin123")?));
    admin.insert("role".into(), json!("admin"));
    admin.insert("nom".into(), json!("Admin"));
    admin.insert("created_at".into(), json!(now.clone()));
    db(state).insert("users", &admin)?;

    let mut emp = Map::new();
    emp.insert("email".into(), json!("john@gamelounge.com"));
    emp.insert("password_hash".into(), json!(auth::hash_password("employe123")?));
    emp.insert("role".into(), json!("employe"));
    emp.insert("nom".into(), json!("John Doe"));
    emp.insert("created_at".into(), json!(now));
    db(state).insert("users", &emp)?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let data_dir = std::env::var("DATADIR").unwrap_or_else(|_| "/tmp/gamelounge".to_string());
    std::fs::create_dir_all(&data_dir).expect("impossible de créer DATADIR");
    let db_path = std::path::Path::new(&data_dir).join("gamelounge.db");
    println!("DATADIR : {data_dir}");
    println!("Base : {db_path:?}");

    let db = Db::open(&db_path).expect("ouverture SQLite");
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "game-lounge-secret-2024".into());
    let state = Arc::new(AppState {
        db: std::sync::Arc::new(db),
        jwt_secret,
        supabase_pool: Mutex::new(None),
        supabase_reconnecting: std::sync::atomic::AtomicBool::new(false),
        sync_state: Mutex::new(None),
        login_attempts: Mutex::new(HashMap::new()),
        session_authenticated: std::sync::atomic::AtomicBool::new(true), // serveur debug : métier toujours actif
    });
    seed_default_users(&state).expect("seed des comptes par défaut");

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(auth_login))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/auth/me", get(auth_me))
        .route("/api/consoles", get(consoles_list).post(consoles_create))
        .route("/api/consoles/:id", get(consoles_get).put(consoles_update).delete(consoles_delete))
        .route("/api/jeux", get(jeux_list).post(jeux_create))
        .route("/api/jeux/:id", get(jeux_get).put(jeux_update).delete(jeux_delete))
        .route("/api/joueurs", get(joueurs_list).post(joueurs_create))
        .route("/api/joueurs/:id", get(joueurs_get).put(joueurs_update).delete(joueurs_delete))
        .route("/api/joueurs/:id/historique", get(joueurs_historique))
        .route("/api/sessions", get(sessions_list).post(sessions_create))
        .route("/api/sessions/:id", get(sessions_get).put(sessions_update).delete(sessions_delete))
        .route("/api/sessions/:id/pause", put(sessions_pause))
        .route("/api/sessions/:id/reprendre", put(sessions_reprendre))
        .route("/api/sessions/:id/terminer", put(sessions_terminer))
        .route("/api/factures", get(factures_list).post(factures_create))
        .route("/api/factures/:id", get(factures_get).put(factures_update).delete(factures_delete))
        .route("/api/factures/:id/pdf", get(factures_pdf))
        .route("/api/factures/:id/annuler", put(factures_annuler))
        .route("/api/lignes_facture", get(lignes_list).post(lignes_create))
        .route("/api/lignes_facture/:id", get(lignes_get).put(lignes_update).delete(lignes_delete))
        .route("/api/tarifs", get(tarifs_list).post(tarifs_create))
        .route("/api/tarifs/:id", get(tarifs_get).put(tarifs_update).delete(tarifs_delete))
        .route("/api/jetons", get(jetons_list).post(jetons_create))
        .route("/api/jetons/:id", get(jetons_get).put(jetons_update).delete(jetons_delete))
        .route("/api/messages", get(messages_list).post(messages_create))
        .route("/api/messages/:id", get(messages_get).put(messages_update).delete(messages_delete))
        .route("/api/parametres/fidelite", get(fidelite_get).put(fidelite_put).post(fidelite_create))
        .route("/api/parametres/fidelite/:id", get(fidelite_get_by_id).delete(fidelite_delete))
        .route("/api/rapports/ca", get(rapports_ca))
        .route("/api/users", get(users_list).post(users_create))
        .route("/api/users/:id", get(users_get).put(users_update).delete(users_delete))
        .route("/api/sync/status", get(sync_status))
        .route("/api/sync/toggle", post(sync_toggle))
        .route("/api/sync/run", post(sync_run))
        .route("/api/sync/poll", get(sync_poll))
        .with_state(state)
        .layer(axum::middleware::from_fn(cors_layer));

    let addr = std::env::var("GL_SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    println!("Serveur de debug sur http://{addr}");
    println!("Test rapide : curl http://localhost:8080/api/health");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serveur");
}

async fn cors_layer(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if req.method() == axum::http::Method::OPTIONS {
        let mut res = axum::response::Response::new(axum::body::Body::empty());
        res.headers_mut().insert("Access-Control-Allow-Origin", "*".parse().unwrap());
        res.headers_mut().insert("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
        res.headers_mut().insert("Access-Control-Allow-Headers", "Content-Type, Authorization".parse().unwrap());
        return res;
    }
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    headers.insert("Access-Control-Allow-Headers", "Content-Type, Authorization".parse().unwrap());
    res
}

// messages_get didn't exist in the original commands (only list/create/update/delete).
// Use a stub to satisfy the router registration.
async fn messages_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> (StatusCode, Json<Value>) {
    let token = token_from_headers(&headers);
    let result = (|| -> ApiResult<Value> {
        claims(&state, &token)?;
        if !validators::is_valid_id(id) {
            return Err(ApiError::bad_request("ID invalide"));
        }
        get_by_id(&state, "messages", id, "Message non trouvé")
    })();
    reply(result)
}