//! Notifications UI des changements de données (cloche du header + Android).
//!
//! La base SQLite ne connaît pas Tauri ; elle expose un callback simple posé
//! une fois au démarrage. Chaque insert/update/remove local (y compris ceux
//! appliqués par la sync delta depuis les AUTRES appareils) déclenche :
//!   1. l'événement Tauri "db-change" -> cloche du header + toast,
//!   2. une notification Android native pour les événements importants
//!      (le watcher de session envoie déjà la sienne).
//!
//! Typage souple : le callback est `Send + Sync` et ne connaît que serde_json.

use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// Callback appelé à chaque écriture locale de données (app + sync delta).
pub type ChangeCallback = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

/// Tables qui intéressent la cloche (données d'exploitation, pas la technique).
pub const NOTIFY_TABLES: [&str; 11] = [
    "users", "consoles", "jeux", "joueurs", "tarifs", "parametres_fidelite", "messages",
    "sessions_jeu", "jetons_transactions", "factures", "lignes_facture",
];

/// Événements qui méritent une notification Android native (mode arrière-plan).
fn is_important(table: &str, operation: &str, row: &Value) -> bool {
    match table {
        "sessions_jeu" => {
            // Démarrage / terminaison de session (par un autre appareil).
            if operation == "DELETE" {
                return true;
            }
            matches!(
                row.get("statut").and_then(Value::as_str),
                Some("en_cours") | Some("terminee")
            )
        }
        "factures" | "lignes_facture" | "messages" | "users" => true,
        _ => false,
    }
}

/// Nom lisible d'une table, pour ne JAMAIS afficher un nom technique.
fn libelle_table(table: &str) -> &'static str {
    match table {
        "users" => "comptes du personnel",
        "consoles" => "consoles",
        "jeux" => "jeux",
        "joueurs" => "joueurs",
        "tarifs" => "tarifs",
        "parametres_fidelite" => "règles de fidélité",
        "messages" => "messages",
        "sessions_jeu" => "parties",
        "jetons_transactions" => "jetons",
        "factures" => "factures",
        "lignes_facture" => "lignes de facture",
        _ => "données",
    }
}

/// Libellé clair d'un statut de facture (aucun mot technique).
fn libelle_statut_facture(statut: &str) -> &'static str {
    match statut {
        "payee" | "paye" => "payée",
        "en_attente" => "en attente de paiement",
        "annulee" => "annulée",
        "remboursee" => "remboursée",
        _ => "mise à jour",
    }
}

/// Titre + corps de la notification, en phrases simples.
///
/// Règles : jamais de « # », jamais de nom de table ni d'identifiant technique,
/// les montants écrits avec des espaces (« 12 500 FC »), et une phrase complète
/// qui se comprend sans ouvrir l'application.
fn describe(table: &str, operation: &str, row: &Value) -> (String, String) {
    let texte = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(v) = row.get(*k).and_then(Value::as_str) {
                if !v.trim().is_empty() {
                    return v.trim().to_string();
                }
            }
        }
        String::new()
    };
    let nombre = |keys: &[&str]| -> Option<i64> {
        for k in keys {
            if let Some(v) = row.get(*k) {
                if let Some(n) = v.as_f64() {
                    return Some(n.round() as i64);
                }
            }
        }
        None
    };
    // « Marie » -> « Marie », rien -> phrase neutre : on n'affiche jamais un numéro.
    let qui = |keys: &[&str], neutre: &str| -> String {
        let n = texte(keys);
        if n.is_empty() { neutre.to_string() } else { n }
    };

    match (table, operation) {
        ("sessions_jeu", "INSERT") => (
            "Nouvelle partie".into(),
            format!("{} vient de commencer à jouer.", qui(&["joueur_nom", "nom"], "Un joueur")),
        ),
        ("sessions_jeu", "UPDATE") => {
            let statut = row.get("statut").and_then(Value::as_str).unwrap_or("");
            let joueur = qui(&["joueur_nom", "nom"], "Un joueur");
            match statut {
                "terminee" => {
                    let montant = nombre(&["montant"]).unwrap_or(0);
                    if montant > 0 {
                        (
                            "Partie terminée".into(),
                            format!("{joueur} a fini de jouer. À payer : {}.", crate::notify::montant_fc(montant)),
                        )
                    } else {
                        ("Partie terminée".into(), format!("{joueur} a fini de jouer."))
                    }
                }
                "pause" => ("Partie en pause".into(), format!("La partie de {joueur} est en pause.")),
                "en_cours" => ("Partie reprise".into(), format!("{joueur} rejoue, le temps repart.")),
                _ => ("Partie modifiée".into(), "Une partie vient d'être modifiée.".into()),
            }
        }
        ("sessions_jeu", "DELETE") => (
            "Partie supprimée".into(),
            "Une partie a été retirée de la liste.".into(),
        ),
        ("factures", "INSERT") => {
            let montant = nombre(&["montant_ttc", "montant"]).unwrap_or(0);
            let numero = texte(&["numero_facture"]);
            let debut = if numero.is_empty() {
                "Nouvelle facture créée".to_string()
            } else {
                format!("Facture {numero} créée")
            };
            ("Nouvelle facture".into(), format!("{debut} — {}.", crate::notify::montant_fc(montant)))
        }
        ("factures", "UPDATE") => {
            let numero = texte(&["numero_facture"]);
            let statut = libelle_statut_facture(row.get("statut").and_then(Value::as_str).unwrap_or(""));
            if numero.is_empty() {
                ("Facture modifiée".into(), format!("Une facture est maintenant {statut}."))
            } else {
                ("Facture modifiée".into(), format!("La facture {numero} est maintenant {statut}."))
            }
        }
        ("factures", "DELETE") => ("Facture supprimée".into(), "Une facture a été supprimée.".into()),
        ("lignes_facture", _) => (
            "Facture mise à jour".into(),
            "Le détail d'une facture a été modifié.".into(),
        ),
        ("messages", "INSERT") => {
            let titre = texte(&["titre"]);
            let contenu = texte(&["contenu"]);
            let auteur = texte(&["auteur"]);
            let corps = if contenu.is_empty() { titre.clone() } else { contenu };
            let corps = if auteur.is_empty() { corps } else { format!("{auteur} : {corps}") };
            (
                if titre.is_empty() { "Nouveau message".into() } else { format!("Message : {titre}") },
                if corps.is_empty() { "Un nouveau message vous attend.".into() } else { corps },
            )
        }
        ("messages", "DELETE") => ("Message supprimé".into(), "Un message a été supprimé.".into()),
        ("users", "INSERT") => (
            "Nouveau compte".into(),
            format!("{} peut maintenant se connecter.", qui(&["nom", "email"], "Un membre du personnel")),
        ),
        ("users", "UPDATE") => (
            "Compte modifié".into(),
            format!("Le compte de {} a été mis à jour.", qui(&["nom", "email"], "un membre du personnel")),
        ),
        ("users", "DELETE") => (
            "Compte supprimé".into(),
            "Un compte du personnel a été supprimé : cet appareil sera déconnecté.".into(),
        ),
        ("joueurs", "INSERT") => (
            "Nouveau joueur".into(),
            format!("{} a été enregistré.", qui(&["nom"], "Un joueur")),
        ),
        ("joueurs", "UPDATE") => (
            "Joueur modifié".into(),
            format!("La fiche de {} a été mise à jour.", qui(&["nom"], "un joueur")),
        ),
        ("joueurs", "DELETE") => ("Joueur supprimé".into(), "Une fiche joueur a été supprimée.".into()),
        ("consoles", "INSERT") => (
            "Console ajoutée".into(),
            format!("{} est disponible.", qui(&["nom"], "Une console")),
        ),
        ("consoles", "UPDATE") => (
            "Console modifiée".into(),
            format!("{} a changé d'état.", qui(&["nom"], "Une console")),
        ),
        ("consoles", "DELETE") => ("Console retirée".into(), "Une console a été retirée.".into()),
        ("jeux", "INSERT") => (
            "Jeu ajouté".into(),
            format!("{} est au catalogue.", qui(&["titre", "nom"], "Un jeu")),
        ),
        ("jeux", "UPDATE") => (
            "Jeu modifié".into(),
            format!("{} a été mis à jour.", qui(&["titre", "nom"], "Un jeu")),
        ),
        ("jeux", "DELETE") => ("Jeu retiré".into(), "Un jeu a été retiré du catalogue.".into()),
        ("tarifs", _) => ("Tarifs mis à jour".into(), "La liste des tarifs a changé.".into()),
        ("parametres_fidelite", _) => (
            "Fidélité mise à jour".into(),
            "Les règles des jetons de fidélité ont changé.".into(),
        ),
        ("jetons_transactions", "INSERT") => {
            let q = nombre(&["quantite"]).unwrap_or(0);
            let joueur = qui(&["joueur_nom", "nom"], "Un joueur");
            if q > 0 {
                ("Jetons gagnés".into(), format!("{joueur} a reçu {q} jeton(s) de fidélité."))
            } else if q < 0 {
                ("Jetons utilisés".into(), format!("{joueur} a utilisé {} jeton(s).", -q))
            } else {
                ("Jetons mis à jour".into(), format!("Le compte de jetons de {joueur} a changé."))
            }
        }
        ("jetons_transactions", _) => (
            "Jetons mis à jour".into(),
            "Une opération sur les jetons a été enregistrée.".into(),
        ),
        // Aucun cas oublié n'affiche de nom technique : phrase neutre lisible.
        _ => (
            "Mise à jour".into(),
            format!("La liste des {} a été modifiée.", libelle_table(table)),
        ),
    }
}

/// Identifiant de notification par type d'information : une nouvelle
/// notification du même type REMPLACE la précédente au lieu de s'empiler
/// (la barre de notifications reste lisible).
fn id_notif(table: &str) -> i32 {
    let base = match table {
        "sessions_jeu" => 10,
        "factures" | "lignes_facture" => 20,
        "messages" => 30,
        "users" => 40,
        "joueurs" => 50,
        "consoles" => 60,
        "jeux" => 70,
        "jetons_transactions" => 80,
        _ => 90,
    };
    800_000 + base
}

/// Pose le callback sur la base : émet "db-change" au WebView + notification
/// Android pour les événements importants. Appelé UNE FOIS au setup Tauri.
pub fn attach(app: tauri::AppHandle, db: &crate::db::Db) {
    let handle = app.clone();
    let cb: ChangeCallback = Arc::new(move |table, operation, row| {
        // RÈGLE ABSOLUE (démarrage) : aucune notification, aucun événement UI
        // avant l'authentification. Le drapeau est levé par auth_business_ready
        // (écran d'accueil affiché) ; les écritures qui surviennent avant
        // (login lui-même, initialisation) restent donc silencieuses.
        let authenticated = handle
            .try_state::<crate::AppState>()
            .map(|s| s.session_authenticated.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false);
        if !authenticated {
            return;
        }
        if !NOTIFY_TABLES.contains(&table) {
            return;
        }
        let (title, body) = describe(table, operation, &row);
        let payload = json!({
            "table": table,
            "operation": operation,
            "row": row,
            "title": title,
            "body": body,
            "ts": crate::db::now_iso(),
        });
        // 1) Événement UI (cloche). Ne bloque jamais l'appelant (émission non bloquante).
        let _ = handle.emit("db-change", payload.clone());
        // 2) Notification Android pour les événements importants — même en
        //    arrière-plan, c'est le thread natif qui notifie.
        if is_important(table, operation, &row) {
            crate::notify::envoyer(
                &handle,
                &title,
                &body,
                crate::notify::Options::remplace(id_notif(table)),
            );
        }
    });
    if let Ok(mut slot) = db.3.lock() {
        *slot = Some(cb);
    }
}
