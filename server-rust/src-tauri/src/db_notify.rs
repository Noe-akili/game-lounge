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
use tauri::Emitter;

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

/// Libellé lisible de l'événement (titre + corps de notification).
fn describe(table: &str, operation: &str, row: &Value) -> (String, String) {
    let name = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(v) = row.get(*k).and_then(Value::as_str) {
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
        String::new()
    };
    match (table, operation) {
        ("sessions_jeu", "INSERT") => {
            let who = name(&["joueur_nom", "nom"]);
            (
                "Nouvelle session".into(),
                if who.is_empty() { "Une session a démarré".into() } else { format!("Session démarrée — {who}") },
            )
        }
        ("sessions_jeu", "UPDATE") => {
            let st = row.get("statut").and_then(Value::as_str).unwrap_or("");
            match st {
                "terminee" => ("Session terminée".into(), format!("Session terminée — {}", name(&["joueur_nom", "nom"]))),
                "pause" => ("Session en pause".into(), format!("Session en pause — {}", name(&["joueur_nom", "nom"]))),
                _ => ("Session modifiée".into(), "Une session a été mise à jour".into()),
            }
        }
        ("sessions_jeu", "DELETE") => ("Session supprimée".into(), "Une session a été supprimée".into()),
        ("factures", "INSERT") => {
            let montant = row.get("montant_ttc").or_else(|| row.get("montant")).and_then(Value::as_f64).unwrap_or(0.0);
            (
                "Nouvelle facture".into(),
                format!("Facture {} — {:.0} FC", name(&["numero_facture", "nom"]), montant),
            )
        }
        ("factures", "UPDATE") => ("Facture modifiée".into(), format!("Facture {} mise à jour", name(&["numero_facture", "nom"]))),
        ("factures", "DELETE") => ("Facture supprimée".into(), "Une facture a été supprimée".into()),
        ("messages", "INSERT") => ("Nouveau message".into(), name(&["titre", "contenu"])),
        ("users", "INSERT") => ("Nouvel utilisateur".into(), format!("{} a été créé", name(&["email", "nom"]))),
        ("users", "DELETE") => ("Utilisateur supprimé".into(), "Un compte utilisateur a été supprimé".into()),
        ("joueurs", "INSERT") => ("Nouveau joueur".into(), format!("{} a été enregistré", name(&["nom"]))),
        ("joueurs", "UPDATE") => ("Joueur modifié".into(), format!("{} a été mis à jour", name(&["nom"]))),
        ("consoles", "INSERT") => ("Console ajoutée".into(), format!("{} ajoutée", name(&["nom"]))),
        ("jeux", "INSERT") => ("Jeu ajouté".into(), format!("{} ajouté au catalogue", name(&["titre", "nom"]))),
        _ => (
            format!("{table} {operation}").replace('_', " "),
            "Donnée mise à jour".into(),
        ),
    }
}

/// Pose le callback sur la base : émet "db-change" au WebView + notification
/// Android pour les événements importants. Appelé UNE FOIS au setup Tauri.
pub fn attach(app: tauri::AppHandle, db: &crate::db::Db) {
    let handle = app.clone();
    let cb: ChangeCallback = Arc::new(move |table, operation, row| {
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
            use tauri_plugin_notification::{NotificationExt, PermissionState};
            let notification = handle.notification();
            if matches!(notification.permission_state(), Ok(PermissionState::Granted))
                || matches!(notification.request_permission(), Ok(PermissionState::Granted))
            {
                let _ = notification.builder().title(&title).body(&body).show();
            }
        }
    });
    if let Ok(mut slot) = db.3.lock() {
        *slot = Some(cb);
    }
}
