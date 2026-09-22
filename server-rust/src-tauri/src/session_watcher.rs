// Watcher de sessions : toutes les 10 secondes, vérifie si une session en cours
// a dépassé son temps ALLOUÉ (duree_allouee). Si oui :
//   1. la session est finalisée automatiquement (facture + jetons de fidélité),
//   2. une VRAIE notification Android est envoyée sur l'appareil
//      ("Temps écoulé" + qui jouait, sur quel poste, et le montant à payer),
//      via tauri-plugin-notification.
//   3. tant qu'une session tourne et qu'il reste moins de 30 minutes, une
//      notification COMPTE À REBOURS se met à jour toute seule ("Il reste
//      12 min") : silencieuse, persistante, et remplacée par le message final.
//
// Pourquoi un watcher côté Rust : l'app peut être en arrière-plan (WebView
// suspendu, timers JS gelés) — seul un thread/process natif reste fiable pour
// détecter la fin du temps et faire vibrer le téléphone.

use serde_json::Value;
use tauri::Manager;

use crate::db::Db;

/// Démarre le watcher en tâche de fond (appelé une seule fois au setup).
/// `db` est l'Arc partagé de l'AppState : mêmes données que les commandes.
pub fn start(app: tauri::AppHandle, db: std::sync::Arc<Db>) {
    tauri::async_runtime::spawn(async move {
        // Petit délai : laisser le boot (DB, WebView) se terminer calmement.
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        loop {
            // RÈGLE ABSOLUE (démarrage) : aucun processus métier (finalisation
            // de sessions = facturation) avant l'authentification. Le drapeau
            // est levé par auth_business_ready (écran d'accueil affiché).
            let authenticated = app
                .try_state::<crate::AppState>()
                .map(|s| s.session_authenticated.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(false);
            if authenticated {
                let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    check_expired_sessions(&app, &db)
                }));
                if let Err(e) = checked {
                    eprintln!("[session-watcher] panic attrapé (reprise au prochain tick): {e:?}");
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });
}

/// Vérifie toutes les sessions en cours et termine celles dont le temps est écoulé.
/// Idempotent : une session déjà 'terminee' n'est jamais retouchée.
fn check_expired_sessions(app: &tauri::AppHandle, db: &Db) {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let (expired, restantes) = {
        let conn = match db.0.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(), // Mutex empoisonné : on récupère
        };
        let mut expired = Vec::new();
        // Sessions encore en cours : (id, secondes restantes) — servent au
        // compte à rebours affiché dans la barre de notifications.
        let mut restantes: Vec<(i64, i64)> = Vec::new();
        // TOUTES les sessions en cours sont évaluées, pas seulement la plus
        // ancienne : une session de 5 min démarrée après une session de 3 h doit
        // expirer la première. (Avant, la boucle s'arrêtait dès que la session la
        // plus ancienne n'était pas expirée : les tarifs courts ne se
        // terminaient jamais tout seuls.)
        for (id, _console, allouee, accumulee, debut) in crate::db::find_active_sessions(&conn) {
            // `allouee` est en MINUTES alors que `accumulee` est en SECONDES.
            // Les comparer directement faisait expirer une session au premier
            // rafraîchissement (ex. 60 - 0 était lu comme 60 secondes).
            // Le chrono = début de la période active + secondes restantes.
            let debut_ms = chrono::DateTime::parse_from_rfc3339(&debut)
                .map(|d| d.timestamp_millis())
                .unwrap_or(0);
            let reste_s = if debut_ms > 0 && allouee > 0 {
                (debut_ms + (allouee * 60 - accumulee).max(0) * 1000 - now_ms) / 1000
            } else {
                i64::MAX
            };
            if reste_s > 0 {
                if reste_s != i64::MAX {
                    restantes.push((id, reste_s));
                }
                continue; // cette session a encore du temps -> on passe à la suivante
            }
            // Durée EXACTE à facturer, À LA SECONDE : allocation entière +
            // dépassement réel (reste_s est négatif = secondes de retard du
            // watcher, PAS du temps joué : on ne l'arrondit plus à la minute).
            let depassement_s = (-reste_s).max(0);
            let duree_totale_s = allouee * 60 + depassement_s;
            expired.push((id, duree_totale_s));
            // Marque immédiatement comme traitée pour ne pas reprendre la même
            // au prochain passage de boucle si la finalisation échoue.
            let _ = conn.execute(
                "UPDATE sessions_jeu SET statut = 'terminee' WHERE id = ?1 AND statut = 'en_cours'",
                [id],
            );
        }
        (expired, restantes)
    };
    // Compte à rebours : une notification qui se met à jour toute seule pour
    // chaque session bientôt terminée (voir plus bas).
    rafraichir_comptes_a_rebours(app, db, &restantes);

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
                match crate::commands::sessions::finalize_session(db, &s, Some(duree_imposee), true, None)
                {
                    Ok(res) => {
                        let montant = res.get("montant").and_then(Value::as_i64).unwrap_or(0);
                        eprintln!(
                            "[session-watcher] session {} terminée automatiquement ({} FC)",
                            id, montant
                        );
                        // Notification Android réelle (vibration + son système).
                        notifier_fin(app, db, id, montant);
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

/// Identifiant de notification réservé au compte à rebours d'une session.
/// Même identifiant = la notification est REMPLACÉE au lieu de s'empiler.
fn id_notif(session_id: i64) -> i32 {
    900_000 + (session_id.rem_euclid(50_000) as i32)
}

/// Dernier texte affiché pour chaque session : évite de renvoyer la même
/// notification dix fois par minute (le watcher passe toutes les 10 s).
fn dernier_texte() -> &'static std::sync::Mutex<std::collections::HashMap<i64, String>> {
    static ETAT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i64, String>>> =
        std::sync::OnceLock::new();
    ETAT.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// « Jean sur PlayStation 5 (poste 3) » — phrase lisible décrivant la session.
/// Retourne une phrase générique si les noms manquent, JAMAIS un identifiant.
fn qui_joue(db: &Db, session_id: i64) -> String {
    let conn = match db.0.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let ligne = conn.query_row(
        "SELECT COALESCE(j.nom, ''), COALESCE(c.nom, ''), COALESCE(c.poste_numero, 0) \
         FROM sessions_jeu s \
         LEFT JOIN joueurs j ON j.id = s.joueur_id \
         LEFT JOIN consoles c ON c.id = s.console_id \
         WHERE s.id = ?1",
        [session_id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        },
    );
    drop(conn);
    let (joueur, console, poste) = match ligne {
        Ok(v) => v,
        Err(_) => (String::new(), String::new(), 0),
    };
    let mut ou = console.trim().to_string();
    if poste > 0 {
        if ou.is_empty() {
            ou = format!("poste {poste}");
        } else {
            ou = format!("{ou} (poste {poste})");
        }
    }
    match (joueur.trim().is_empty(), ou.is_empty()) {
        (false, false) => format!("{} sur {}", joueur.trim(), ou),
        (false, true) => joueur.trim().to_string(),
        (true, false) => format!("Le joueur sur {ou}"),
        (true, true) => "La partie en cours".to_string(),
    }
}

/// Compte à rebours : met à jour (ou crée) une notification par session dont
/// il reste moins de 30 minutes. Silencieuse et persistante : elle informe
/// sans faire sonner le téléphone à chaque minute.
fn rafraichir_comptes_a_rebours(app: &tauri::AppHandle, db: &Db, restantes: &[(i64, i64)]) {
    /// Au-delà, on n'encombre pas la barre de notifications.
    const SEUIL_S: i64 = 30 * 60;
    let encore: std::collections::HashSet<i64> = restantes.iter().map(|(id, _)| *id).collect();
    // Oublie les sessions terminées / disparues pour ne pas garder de mémoire inutile.
    if let Ok(mut map) = dernier_texte().lock() {
        map.retain(|id, _| encore.contains(id));
    }
    for (id, reste_s) in restantes.iter().copied() {
        if reste_s > SEUIL_S {
            continue;
        }
        // Dernière minute : on descend à la seconde pour que ce soit utile.
        let texte = if reste_s >= 60 {
            format!("Il reste {}", crate::notify::duree_lisible(reste_s))
        } else {
            format!("Il reste {reste_s} secondes")
        };
        // Même texte que la dernière fois -> rien à renvoyer.
        let deja = dernier_texte()
            .lock()
            .ok()
            .and_then(|m| m.get(&id).cloned())
            .unwrap_or_default();
        if deja == texte {
            continue;
        }
        let corps = format!(
            "{} — le temps de jeu se termine bientôt.",
            qui_joue(db, id)
        );
        crate::notify::envoyer(
            app,
            &texte,
            &corps,
            crate::notify::Options {
                id: Some(id_notif(id)),
                silencieuse: true,
                persistante: true,
            },
        );
        if let Ok(mut map) = dernier_texte().lock() {
            map.insert(id, texte);
        }
    }
}

/// Notification finale : remplace le compte à rebours de la même session par
/// le résultat, avec son et vibration cette fois (c'est l'information utile).
fn notifier_fin(app: &tauri::AppHandle, db: &Db, session_id: i64, montant: i64) {
    let corps = format!(
        "{} : le temps est terminé, la session a été fermée. À payer : {}.",
        qui_joue(db, session_id),
        crate::notify::montant_fc(montant)
    );
    crate::notify::envoyer(
        app,
        "Temps écoulé",
        &corps,
        crate::notify::Options::remplace(id_notif(session_id)),
    );
    if let Ok(mut map) = dernier_texte().lock() {
        map.remove(&session_id);
    }
}
