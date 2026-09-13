// Watcher de sessions : toutes les 10 secondes, vérifie si une session en cours
// a dépassé son temps ALLOUÉ (duree_allouee). Si oui :
//   1. la session est finalisée automatiquement (facture + jetons de fidélité),
//   2. une VRAIE notification Android est envoyée sur l'appareil
//      ("Temps écoulé — Session terminée"), via tauri-plugin-notification.
//
// Pourquoi un watcher côté Rust : l'app peut être en arrière-plan (WebView
// suspendu, timers JS gelés) — seul un thread/process natif reste fiable pour
// détecter la fin du temps et faire vibrer le téléphone.

use serde_json::Value;

use crate::db::Db;

/// Démarre le watcher en tâche de fond (appelé une seule fois au setup).
/// `db` est l'Arc partagé de l'AppState : mêmes données que les commandes.
pub fn start(app: tauri::AppHandle, db: std::sync::Arc<Db>) {
    tauri::async_runtime::spawn(async move {
        // Petit délai : laisser le boot (DB, WebView) se terminer calmement.
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        loop {
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check_expired_sessions(&app, &db)
            }));
            if let Err(e) = checked {
                eprintln!("[session-watcher] panic attrapé (reprise au prochain tick): {e:?}");
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });
}

/// Vérifie toutes les sessions en cours et termine celles dont le temps est écoulé.
/// Idempotent : une session déjà 'terminee' n'est jamais retouchée.
fn check_expired_sessions(app: &tauri::AppHandle, db: &Db) {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let expired = {
        let conn = match db.0.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(), // Mutex empoisonné : on récupère
        };
        let mut expired = Vec::new();
        // Reprise après poison Mutex : la boucle tourne tant qu'il reste des
        // sessions actives, en terminant UNE session à la fois (évite tout
        // verrou long qui bloquerait l'UI).
        for _ in 0..200 {
            let Some((id, _console, allouee, accumulee)) = crate::db::find_active_session(&conn)
            else {
                break;
            };
            // Le chrono est `debut + duree_minutes (déjà accumulée, ex. pauses)`.
            // Pour simplifier et rester EXACT dans le cas général (aucune pause),
            // une session est expirée si debut + allouée - accumulée < maintenant.
            let debut_ms = conn
                .query_row(
                    "SELECT debut FROM sessions_jeu WHERE id = ?1",
                    [id],
                    |r| r.get::<_, String>(0),
                )
                .ok()
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.timestamp_millis())
                .unwrap_or(0);
            let reste_s = if debut_ms > 0 {
                (debut_ms + (allouee - accumulee).max(0) * 1000 - now_ms) / 1000
            } else {
                i64::MAX
            };
            if reste_s > 0 {
                break; // session la plus ancienne pas encore expirée -> rien à faire
            }
            // Durée totale à facturer = allocation ENTIÈRE + dépassement (arrondi
            // à la minute supérieure). L'accumulée (pauses) fait déjà partie de
            // l'allocation — elle ne s'ajoute pas.
            let depassement_s = (-reste_s).max(0);
            let depassement_min = (depassement_s + 59) / 60;
            expired.push((id, allouee + depassement_min));
            // Marque immédiatement comme traitée pour ne pas reprendre la même
            // au prochain passage de boucle si la finalisation échoue.
            let _ = conn.execute(
                "UPDATE sessions_jeu SET statut = 'terminee' WHERE id = ?1 AND statut = 'en_cours'",
                [id],
            );
        }
        expired
    };

    for (id, duree_imposee) in expired {
        // Récupère la ligne (re-marquée en_cours si un pull cloud l'a déjà
        // restaurée) puis finalise via la MÊME logique que la terminaison
        // manuelle (facture + jetons), avec l'heure de fin exacte.
        match db.get_opt("sessions_jeu", id) {
            Ok(Some(session)) => {
                let st = session.get("statut").and_then(Value::as_str).unwrap_or("");
                if st == "en_cours" || st == "pause" {
                    // Remet en_cours le temps de finaliser proprement (la
                    // finalisation attend un statut actif pour calculer la durée).
                    let conn = match db.0.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    let _ = conn.execute(
                        "UPDATE sessions_jeu SET statut = 'en_cours' WHERE id = ?1",
                        [id],
                    );
                    drop(conn);
                }
                let mut s = session;
                if let Value::Object(map) = &mut s {
                    map.insert("statut".into(), Value::String("en_cours".into()));
                }
                match crate::commands::sessions::finalize_session(db, &s, Some(duree_imposee), true)
                {
                    Ok(res) => {
                        let montant = res.get("montant").and_then(Value::as_i64).unwrap_or(0);
                        eprintln!(
                            "[session-watcher] session {} terminée automatiquement ({} FC)",
                            id, montant
                        );
                        // Notification Android réelle (vibration + son système).
                        send_ended_notification(app, id, montant);
                    }
                    Err(e) => {
                        eprintln!(
                            "[session-watcher] finalisation auto session {} failed: {}",
                            id, e.message
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("[session-watcher] get session {} failed: {}", id, e.message),
        }
    }
}

/// Envoie la notification native "Session terminée" (Android 13+ = runtime
/// permission demandée automatiquement par le plugin au premier envoi).
fn send_ended_notification(app: &tauri::AppHandle, session_id: i64, montant: i64) {
    use tauri_plugin_notification::{NotificationExt, PermissionState};
    let title = "⏱ Temps écoulé — Session terminée";
    let body = format!(
        "La session #{} est terminée automatiquement. Montant : {} FC.",
        session_id, montant
    );
    let notification = app.notification();
    // Permission : sur Android 13+ elle peut être refusée/absente — on la
    // demande au besoin, mais on n'échoue jamais bruyamment (best effort).
    match notification.permission_state() {
        Ok(PermissionState::Granted) => {}
        Ok(_) => {
            if let Ok(state) = notification.request_permission() {
                if state != PermissionState::Granted {
                    eprintln!("[session-watcher] permission notification refusée");
                    return;
                }
            }
        }
        Err(e) => {
            eprintln!("[session-watcher] permission state failed: {e}");
            return;
        }
    }
    if let Err(e) = notification.builder().title(title).body(body).show() {
        eprintln!("[session-watcher] notification failed: {e}");
    }
}
