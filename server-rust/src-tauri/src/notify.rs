//! Notifications Android : un seul endroit pour les envoyer, avec un style
//! cohérent et des phrases claires.
//!
//! Règles de rédaction (volontairement strictes) :
//!   * jamais de « # », jamais de nom de table, jamais de jargon technique ;
//!   * un titre court qui dit CE QUI SE PASSE, un corps qui dit QUI et COMBIEN ;
//!   * les montants sont écrits avec des espaces : « 12 500 FC » ;
//!   * les durées en mots : « 1 h 05 min », « 12 min », « 40 s ».
//!
//! Détails Android utilisés :
//!   * `id` : deux notifications avec le même identifiant se REMPLACENT
//!     (indispensable pour un compte à rebours qui se met à jour) ;
//!   * `silencieuse` : mise à jour sans son ni vibration (sinon le téléphone
//!     sonnerait toutes les minutes) ;
//!   * `persistante` : notification « en cours », que l'on ne balaye pas par
//!     erreur — utilisée pendant qu'une session tourne ;
//!   * `groupe` : toutes les notifications de l'app sont regroupées ensemble.

use tauri_plugin_notification::{NotificationExt, PermissionState};

/// Groupe Android commun à toutes les notifications de l'application.
pub const GROUPE: &str = "game-lounge";

/// Options d'affichage d'une notification.
pub struct Options {
    /// Même identifiant = la notification précédente est remplacée.
    pub id: Option<i32>,
    /// Sans son ni vibration (mises à jour d'un compte à rebours).
    pub silencieuse: bool,
    /// Notification « en cours » (reste affichée pendant la session).
    pub persistante: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { id: None, silencieuse: false, persistante: false }
    }
}

impl Options {
    /// Notification normale (son + vibration), effaçable d'un balayage.
    pub fn simple() -> Self {
        Self::default()
    }

    /// Notification qui remplace la précédente portant le même identifiant.
    pub fn remplace(id: i32) -> Self {
        Self { id: Some(id), ..Self::default() }
    }
}

/// Montant lisible : 12500 -> « 12 500 FC ».
pub fn montant_fc(montant: i64) -> String {
    let negatif = montant < 0;
    let chiffres = montant.abs().to_string();
    let mut sortie = String::new();
    for (i, c) in chiffres.chars().enumerate() {
        // Espace tous les 3 chiffres en partant de la gauche, calculé sur la
        // longueur restante pour rester juste quel que soit le nombre.
        if i > 0 && (chiffres.len() - i) % 3 == 0 {
            sortie.push(' ');
        }
        sortie.push(c);
    }
    if negatif {
        format!("-{sortie} FC")
    } else {
        format!("{sortie} FC")
    }
}

/// Durée lisible à partir de secondes : « 1 h 05 min », « 12 min », « 40 s ».
pub fn duree_lisible(secondes: i64) -> String {
    let s = secondes.max(0);
    if s < 60 {
        return format!("{s} s");
    }
    let minutes = s / 60;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let heures = minutes / 60;
    let reste = minutes % 60;
    if reste == 0 {
        format!("{heures} h")
    } else {
        format!("{heures} h {reste:02} min")
    }
}

/// Première lettre en majuscule, le reste inchangé (titres propres).
pub fn majuscule(texte: &str) -> String {
    let mut c = texte.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// La permission est-elle accordée ? (Android 13+ : demandée une seule fois.)
fn permission_ok(app: &tauri::AppHandle) -> bool {
    let notification = app.notification();
    match notification.permission_state() {
        Ok(PermissionState::Granted) => true,
        Ok(_) => matches!(notification.request_permission(), Ok(PermissionState::Granted)),
        Err(e) => {
            eprintln!("[notify] état de la permission indisponible : {e}");
            false
        }
    }
}

/// Envoie une notification. Ne panique jamais et n'interrompt jamais
/// l'appelant : une notification perdue ne doit pas casser une facturation.
pub fn envoyer(app: &tauri::AppHandle, titre: &str, corps: &str, opt: Options) {
    if titre.trim().is_empty() && corps.trim().is_empty() {
        return;
    }
    if !permission_ok(app) {
        return;
    }
    let mut b = app
        .notification()
        .builder()
        .title(majuscule(titre.trim()))
        .body(corps.trim())
        // Texte complet visible quand l'utilisateur déplie la notification.
        .large_body(corps.trim())
        .group(GROUPE);
    if let Some(id) = opt.id {
        b = b.id(id);
    }
    if opt.silencieuse {
        b = b.silent();
    }
    if opt.persistante {
        b = b.ongoing();
    } else {
        // Disparaît quand on appuie dessus.
        b = b.auto_cancel();
    }
    if let Err(e) = b.show() {
        eprintln!("[notify] envoi impossible : {e}");
    }
}
