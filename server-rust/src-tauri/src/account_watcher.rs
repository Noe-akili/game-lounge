//! Surveillance du compte connecté (gestion des comptes 100 % en ligne).
//!
//! Le cloud est la seule source de vérité pour les comptes. Quand un
//! administrateur supprime (ou désactive) un compte, l'appareil de la personne
//! concernée doit s'en apercevoir tout seul : ce module interroge le cloud
//! toutes les minutes tant qu'une session est ouverte, et en cas de suppression
//! il EFFACE toutes les données de l'application puis renvoie le frontend sur
//! l'écran de connexion (événement `account-revoked`).
//!
//! Règle de prudence : hors ligne, ou si la requête échoue, on ne conclut RIEN.
//! Un réseau coupé n'est pas une suppression de compte.

use std::sync::atomic::Ordering;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

/// Intervalle de vérification : assez court pour que la personne supprimée soit
/// éjectée en moins d'une minute, assez léger pour ne rien coûter en batterie ni
/// en données mobiles (une requête, une ligne).
const PERIODE: std::time::Duration = std::time::Duration::from_secs(60);

/// Résultat d'une vérification.
pub struct Verification {
    /// La requête cloud a réellement pu être faite.
    pub verifie: bool,
    /// Le compte a disparu (ou est désactivé) : les données ont été effacées.
    pub revoque: bool,
    /// "supprime" | "desactive" | "" (aucune conclusion)
    pub raison: &'static str,
    pub message: String,
}

impl Verification {
    fn indetermine() -> Self {
        Verification { verifie: false, revoque: false, raison: "", message: String::new() }
    }
    fn valide() -> Self {
        Verification { verifie: true, revoque: false, raison: "", message: String::new() }
    }
}

/// Démarre la surveillance en tâche de fond (appelée une fois au boot).
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(PERIODE).await;
            let Some(state) = app.try_state::<crate::AppState>() else { continue };
            // RÈGLE DE DÉMARRAGE : aucun accès réseau métier avant authentification.
            if !state.session_authenticated.load(Ordering::Relaxed) {
                continue;
            }
            let _ = verifier(&app, &state).await;
        }
    });
}

/// Identité de la session en cours, telle qu'enregistrée au login
/// (id + email — jamais de mot de passe).
fn session_courante(db: &crate::db::Db) -> Option<(i64, String)> {
    let brut = db.get_setting("session_user").ok().flatten().unwrap_or_default();
    if brut.trim().is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(&brut).ok()?;
    let id = v.get("id").and_then(Value::as_i64).unwrap_or(0);
    let email = v.get("email").and_then(Value::as_str).unwrap_or_default().to_string();
    if id == 0 && email.is_empty() {
        return None;
    }
    Some((id, email))
}

/// Demande au cloud si le compte connecté existe toujours.
pub async fn verifier(app: &AppHandle, state: &tauri::State<'_, crate::AppState>) -> Verification {
    let Some((id, email)) = session_courante(&state.db) else {
        return Verification::indetermine();
    };
    verifier_cloud(app, state, id, email).await
}

/// Version cloud : une seule requête, une seule ligne lue.
#[cfg(feature = "supabase-sync")]
async fn verifier_cloud(
    app: &AppHandle,
    state: &tauri::State<'_, crate::AppState>,
    id: i64,
    email: String,
) -> Verification {
    let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
    // Hors ligne : aucune conclusion, la session reste utilisable.
    let Some(pool) = pool_opt else {
        return Verification::indetermine();
    };
    match crate::supabase::cloud_user_statut(&pool, id, &email).await {
        Ok(statut) => {
            let existe = statut.get("existe").and_then(Value::as_bool).unwrap_or(true);
            let desactive = statut.get("archive").and_then(Value::as_bool).unwrap_or(false);
            if !existe {
                return revoquer(app, state, "supprime");
            }
            if desactive {
                return revoquer(app, state, "desactive");
            }
            // Compte valide : on profite de la réponse pour garder le nom et le
            // rôle à jour dans l'annuaire local (toujours sans secret).
            let nom = statut
                .get("nom")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let role = statut
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("employe")
                .to_string();
            let email_cloud = statut
                .get("email")
                .and_then(Value::as_str)
                .unwrap_or(email.as_str())
                .to_string();
            let id_cloud = statut.get("id").and_then(Value::as_i64).unwrap_or(id);
            let _ = state
                .db
                .cache_identite_utilisateur(id_cloud, &email_cloud, &nom, &role, false);
            crate::commands::auth::enregistrer_session(&state.db, id_cloud, &email_cloud, &nom, &role);
            Verification::valide()
        }
        // Erreur réseau / serveur : on ne révoque JAMAIS sur un doute.
        Err(e) => {
            crate::logger::log_cloud(&format!(
                "vérification du compte impossible ({}) — session conservée",
                e.message
            ));
            Verification::indetermine()
        }
    }
}

/// Build sans cloud : il n'y a rien à vérifier, on ne conclut rien.
#[cfg(not(feature = "supabase-sync"))]
async fn verifier_cloud(
    _app: &AppHandle,
    _state: &tauri::State<'_, crate::AppState>,
    _id: i64,
    _email: String,
) -> Verification {
    Verification::indetermine()
}

/// Le compte n'existe plus : EFFACEMENT TOTAL des données de l'appareil, puis
/// retour immédiat à l'écran de connexion côté interface.
pub fn revoquer(
    app: &AppHandle,
    state: &tauri::State<'_, crate::AppState>,
    raison: &'static str,
) -> Verification {
    let message = if raison == "desactive" {
        "Votre compte a été désactivé par l'administrateur. Les données de cet appareil ont été effacées."
    } else {
        "Votre compte a été supprimé par l'administrateur. Les données de cet appareil ont été effacées."
    };
    crate::logger::log_auth(&format!(
        "compte {raison} : effacement des données locales et retour au login"
    ));
    // 1) Les processus métier s'arrêtent AVANT l'effacement : plus aucune sync,
    //    plus aucun watcher n'écrit dans une base qu'on est en train de vider.
    state.session_authenticated.store(false, Ordering::Relaxed);
    // 2) Effacement complet + session persistée détruite.
    if let Err(e) = state.db.effacer_donnees_locales() {
        crate::logger::log_auth(&format!("effacement partiel : {}", e.message));
    }
    let _ = state.db.set_setting("session_backup", "");
    let _ = state.db.set_setting("session_user", "");
    // 3) L'interface est prévenue : elle vide sa session et route vers /login.
    let _ = app.emit(
        "account-revoked",
        json!({ "reason": raison, "message": message }),
    );
    Verification {
        verifie: true,
        revoque: true,
        raison,
        message: message.to_string(),
    }
}
