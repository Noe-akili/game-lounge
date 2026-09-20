// /api/sessions - gestion des sessions de jeu, pausa/reprise/terminaison (avec facture).

use serde_json::{Map, Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap, row_id, sort_desc_by_created_at};
use crate::db::{now_iso, today_str};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn parse_iso_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp_millis())
}

fn is_deleted(v: Option<&Value>) -> bool {
    v.map(|x| x.as_bool().unwrap_or(false) || x.as_i64().unwrap_or(0) == 1).unwrap_or(false)
}

/// Montant d'une session (règle UNIQUE partagée par l'affichage et la facture) :
/// le joueur paie le forfait choisi (duree_allouee minutes pour tarif_prix FC).
/// Dépassement : prorata à la SECONDE sur le taux du forfait (tarif_prix / duree_allouee),
/// arrondi au franc supérieur UNIQUEMENT (plus d'arrondi minutes -> plus de
/// quelques secondes de retard transformées en 1 minute facturée).
/// Ex : 90 min à 3000 FC jouées 90 min -> 3000 FC (et non 6000 !).
pub fn compute_montant(tarif_prix: i64, allouee: i64, jouee_secondes: i64) -> i64 {
    if allouee > 0 {
        let allouee_s = allouee * 60;
        if jouee_secondes <= allouee_s {
            tarif_prix
        } else {
            let depassement_s = jouee_secondes - allouee_s;
            tarif_prix + (depassement_s * tarif_prix + allouee_s - 1) / allouee_s
        }
    } else {
        ((jouee_secondes + 3599) / 3600) * tarif_prix
    }
}

fn enrich_session(
    s: &Value,
    consoles: &[Value],
    joueurs: &[Value],
    jeux: &[Value],
    users: &[Value],
) -> Value {
    let console_id = s.get("console_id").and_then(Value::as_i64);
    let joueur_id = s.get("joueur_id").and_then(Value::as_i64);
    let jeu_id = s.get("jeu_id").and_then(Value::as_i64);
    let employe_id = s.get("employe_id").and_then(Value::as_i64);
    let mut o = s.clone();
    if let Value::Object(map) = &mut o {
        map.insert(
            "console_nom".into(),
            consoles
                .iter()
                .find(|c| row_id(c) == console_id)
                .and_then(|c| c.get("nom"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert(
            "console_type".into(),
            consoles
                .iter()
                .find(|c| row_id(c) == console_id)
                .and_then(|c| c.get("type"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert(
            "poste_numero".into(),
            consoles
                .iter()
                .find(|c| row_id(c) == console_id)
                .and_then(|c| c.get("poste_numero"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert(
            "joueur_nom".into(),
            joueurs
                .iter()
                .find(|j| row_id(j) == joueur_id)
                .and_then(|j| j.get("nom"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert(
            "jeu_nom".into(),
            jeux
                .iter()
                .find(|j| row_id(j) == jeu_id)
                .and_then(|j| j.get("titre"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert(
            "employe_nom".into(),
            users
                .iter()
                .find(|u| row_id(u) == employe_id)
                .and_then(|u| u.get("nom"))
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    o
}

/// GET /api/sessions (?statut=)
#[tauri::command]
pub fn sessions_list(
    state: State<'_, AppState>,
    token: Option<String>,
    statut: Option<String>,
    include_deleted: Option<bool>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let include_del = include_deleted.unwrap_or(false);
    // query_all() filtre deja "deleted = 0" en SQL : sans query_all_all() la case
    // "Afficher archivees" ne peut rien remonter.
    let mut sessions: Vec<Value> = if include_del {
        db.query_all_all("sessions_jeu")?
    } else {
        db.query_all("sessions_jeu")?
    };
    if let Some(s) = statut {
        sessions.retain(|x| x.get("statut").and_then(Value::as_str) == Some(s.as_str()));
    }
    if !include_del {
        sessions.retain(|x| !is_deleted(x.get("deleted")));
    }
    sort_desc_by_created_at(&mut sessions);

    // Libelles : une session archivee reference souvent une console/un jeu/un
    // joueur lui-meme archive -> on lit TOUTES les lignes pour l'enrichissement.
    let consoles = db.query_all_all("consoles")?;
    let joueurs = db.query_all_all("joueurs")?;
    let jeux = db.query_all_all("jeux")?;
    let users = db.query_all_all("users")?;

    Ok(Value::Array(
        sessions
            .iter()
            .map(|s| enrich_session(s, &consoles, &joueurs, &jeux, &users))
            .collect(),
    ))
}

/// GET /api/sessions/:id
#[tauri::command]
pub fn sessions_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let s = get_by_id(db, "sessions_jeu", id, "Session non trouvée")?;

    let statut = s.get("statut").and_then(Value::as_str).unwrap_or("");
    let debut = s.get("debut").and_then(Value::as_str).unwrap_or("");
    let fin = s.get("fin").and_then(Value::as_str).unwrap_or("");
    let duree_minutes = s.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
    let duree_secondes_row = s.get("duree_secondes").and_then(Value::as_i64);
    // Précision seconde : duree_secondes si présent, sinon fallback minutes*60
    // (lignes créées avant la colonne).
    let accum_secondes = duree_secondes_row.unwrap_or(duree_minutes * 60);

    let now_ms = chrono::Utc::now().timestamp_millis();
    let duree_secondes = if statut == "en_cours" {
        // Temps déjà accumulé (secondes exactes, pauses comprises) + temps
        // écoulé depuis la (re)prise — duree_minutes N'EST PAS la durée du tarif.
        accum_secondes
            + now_ms
                .checked_sub(parse_iso_ms(debut).unwrap_or(now_ms))
                .unwrap_or(0)
                / 1000
    } else if statut == "pause" {
        accum_secondes
    } else {
        parse_iso_ms(fin)
            .and_then(|f| parse_iso_ms(debut).map(|d| (f - d) / 1000))
            .unwrap_or(0)
    };

    let console_id = s.get("console_id").and_then(Value::as_i64);
    let joueur_id = s.get("joueur_id").and_then(Value::as_i64);
    let jeu_id = s.get("jeu_id").and_then(Value::as_i64);
    let console_nom = db.find_one("consoles", |c| row_id(c) == console_id)?.and_then(|c| c.get("nom").cloned());
    let joueur = db.find_one("joueurs", |j| row_id(j) == joueur_id)?;
    let jeu_nom = db.find_one("jeux", |j| row_id(j) == jeu_id)?.and_then(|j| j.get("titre").cloned());

    let mut o = s.clone();
    if let Value::Object(map) = &mut o {
        map.insert("console_nom".into(), console_nom.unwrap_or(Value::Null));
        map.insert("joueur_nom".into(), joueur.as_ref().and_then(|j| j.get("nom")).cloned().unwrap_or(Value::Null));
        map.insert("joueur_telephone".into(), joueur.as_ref().and_then(|j| j.get("telephone")).cloned().unwrap_or(Value::Null));
        map.insert("jeu_nom".into(), jeu_nom.unwrap_or(Value::Null));
        map.insert("duree_secondes".into(), json!(duree_secondes.max(0)));
    }
    Ok(o)
}

/// POST /api/sessions (démarrage d'une session)
#[tauri::command]
pub fn sessions_create(
    state: State<'_, AppState>,
    token: Option<String>,
    console_id: i64,
    joueur_id: i64,
    jeu_id: i64,
    tarif_id: Option<i64>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    if !validators::is_valid_id(console_id) {
        return Err(ApiError::bad_request("Console ID invalide"));
    }
    if !validators::is_valid_id(joueur_id) {
        return Err(ApiError::bad_request("Joueur ID invalide"));
    }
    if !validators::is_valid_id(jeu_id) {
        return Err(ApiError::bad_request("Jeu ID invalide"));
    }
    let db = db(&state);
    let existing = db.find_one("sessions_jeu", |s| {
        s.get("console_id").and_then(Value::as_i64) == Some(console_id)
            && matches!(
                s.get("statut").and_then(Value::as_str),
                Some("en_cours") | Some("pause")
            )
    })?;
    if existing.is_some() {
        return Err(ApiError::bad_request("Cette console a déjà une session en cours"));
    }

    let mut tarif_prix: i64 = 2000;
    let mut duree_minutes: i64 = 60;
    let mut tarif_id_val: Option<i64> = None;
    if let Some(tid) = tarif_id {
        if let Some(tarif) = db.find_one("tarifs", |t| row_id(t) == Some(tid))? {
            tarif_prix = tarif.get("prix").and_then(Value::as_i64).unwrap_or(2000);
            duree_minutes = tarif
                .get("duree_minutes")
                .and_then(Value::as_i64)
                .unwrap_or(60);
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
    // duree_minutes = temps DÉJÀ JOUÉ (accumulée, ex. pauses). On démarre à 0 :
    // la durée du tarif va UNIQUEMENT dans duree_allouee. Avant, la présélection
    // à la durée du tarif faisait afficher 1:00:00 dès le démarrage (chrono
    // "bizarre") et déclarait la session expirée immédiatement.
    row.insert("duree_minutes".into(), json!(0));
    // Précision seconde : base d'accumulation à 0 aussi.
    row.insert("duree_secondes".into(), json!(0));
    // Durée ALLOUÉE par le tarif : c'est elle qui déclenche la terminaison
    // automatique (notification) quand le temps est écoulé.
    row.insert("duree_allouee".into(), json!(duree_minutes));
    row.insert("montant".into(), json!(tarif_prix));
    row.insert("tarif_prix".into(), json!(tarif_prix));
    row.insert("jetons_gagnes".into(), json!(0));
    row.insert("statut".into(), json!("en_cours"));
    row.insert("created_at".into(), json!(now_iso()));
    let session = db.insert("sessions_jeu", &row)?;

    let mut upd_console = jmap();
    upd_console.insert("etat".into(), json!("occupee"));
    db.update("consoles", console_id, &upd_console)?;

    let mut upd_joueur = jmap();
    upd_joueur.insert("derniere_visite".into(), json!(now_iso()));
    db.update("joueurs", joueur_id, &upd_joueur)?;

    let consoles = db.query_all("consoles")?;
    let joueurs = db.query_all("joueurs")?;
    let jeux = db.query_all("jeux")?;
    let users: Vec<Value> = Vec::new();
    Ok(enrich_session(&session, &consoles, &joueurs, &jeux, &users))
}

/// PUT /api/sessions/:id/pause
#[tauri::command]
pub fn sessions_pause(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let s = get_by_id(db, "sessions_jeu", id, "Session non trouvée")?;
    if s.get("statut").and_then(Value::as_str) != Some("en_cours") {
        return Err(ApiError::bad_request("Session non en cours"));
    }
    let debut = s.get("debut").and_then(Value::as_str).unwrap_or("");
    // Accumulation en SECONDES : tronquer à la minute (total_secondes / 60)
    // perdait les secondes restantes à chaque pause -> dérive progressive.
    let duree_minutes = s.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
    let accum_secondes = s
        .get("duree_secondes")
        .and_then(Value::as_i64)
        .unwrap_or(duree_minutes * 60);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let elapsed = now_ms
        .checked_sub(parse_iso_ms(debut).unwrap_or(now_ms))
        .unwrap_or(0)
        / 1000;
    let total_secondes = accum_secondes + elapsed;
    let console_id = s
        .get("console_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::internal("Console ID manquant"))?;

    let mut upd = jmap();
    upd.insert("statut".into(), json!("pause"));
    // duree_minutes : conservé pour compat (arrondi inférieur), la vraie valeur
    // est duree_secondes.
    upd.insert("duree_minutes".into(), json!(total_secondes / 60));
    upd.insert("duree_secondes".into(), json!(total_secondes));
    db.update("sessions_jeu", id, &upd)?;

    let mut upd_console = jmap();
    upd_console.insert("etat".into(), json!("pause"));
    db.update("consoles", console_id, &upd_console)?;

    get_by_id(db, "sessions_jeu", id, "Session non trouvée")
}

/// PUT /api/sessions/:id/reprendre
#[tauri::command]
pub fn sessions_reprendre(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let s = get_by_id(db, "sessions_jeu", id, "Session non trouvée")?;
    if s.get("statut").and_then(Value::as_str) != Some("pause") {
        return Err(ApiError::bad_request("Session non en pause"));
    }
    let console_id = s
        .get("console_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::internal("Console ID manquant"))?;

    let mut upd = jmap();
    upd.insert("statut".into(), json!("en_cours"));
    upd.insert("debut".into(), json!(now_iso()));
    // La base secondes est conservée telle quelle (déjà accumulée au pause) :
    // seule la nouvelle période en cours s'écoule depuis ce nouveau `debut`.
    db.update("sessions_jeu", id, &upd)?;

    let mut upd_console = jmap();
    upd_console.insert("etat".into(), json!("occupee"));
    db.update("consoles", console_id, &upd_console)?;

    get_by_id(db, "sessions_jeu", id, "Session non trouvée")
}

/// PUT /api/sessions/:id/terminer (génère la facture + jetons de fidélité)
#[tauri::command]
pub fn sessions_terminer(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    mode_paiement: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let s = get_by_id(db, "sessions_jeu", id, "Session non trouvée")?;
    finalize_session(db, &s, None, false, mode_paiement)
}

/// Finalise une session : statut terminee + heure de fin, libère la console,
/// génère la facture (HT/TVA/TTC + ligne), attribue les jetons de fidélité
/// (règle 'temps' par durée OU règle 'montant' selon le montant de la session)
/// et renvoie le détail { session, facture, montant, jetonsGagnes, dureeMinutes }.
///
/// `duree_imposee` : durée EXACTE en SECONDES à facturer — utilisée par la
/// terminaison AUTOMATIQUE (temps écoulé). Le watcher transmet le délai de
/// dépassement réel à la seconde : plus aucun arrondi qui transforme quelques
/// secondes de retard en 1 minute facturée.
/// `auto` : true = terminaison automatique -> l'heure de fin est calculée depuis
/// `debut + durée totale` (et non "maintenant") pour une facturation exacte.
pub(crate) fn finalize_session(
    db: &crate::db::Db,
    s: &Value,
    duree_imposee: Option<i64>,
    auto: bool,
    mode_paiement: Option<String>,
) -> ApiResult<Value> {
    let id = s
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::internal("Session ID manquant"))?;
    let statut = s.get("statut").and_then(Value::as_str).unwrap_or("");
    let debut = s.get("debut").and_then(Value::as_str).unwrap_or("");
    let duree_minutes = s.get("duree_minutes").and_then(Value::as_i64).unwrap_or(0);
    // Base accumulée en SECONDES (duree_secondes si présent, sinon fallback
    // minutes*60 pour les lignes créées avant cette colonne).
    let accum_secondes = s
        .get("duree_secondes")
        .and_then(Value::as_i64)
        .unwrap_or(duree_minutes * 60);
    let tarif_prix = s.get("tarif_prix").and_then(Value::as_i64).unwrap_or(2000);
    let console_id = s
        .get("console_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::internal("Console ID manquant"))?;
    let joueur_id = s
        .get("joueur_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::internal("Joueur ID manquant"))?;
    let session_id = id;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let elapsed = if statut == "en_cours" {
        now_ms
            .checked_sub(parse_iso_ms(debut).unwrap_or(now_ms))
            .unwrap_or(0)
            / 1000
    } else {
        0
    };
    let total_secondes = accum_secondes + elapsed;
    // Durée EXACTE facturée en secondes : soit celle imposée par le watcher
    // (terminaison auto, à la seconde), soit le temps réellement joué.
    let facture_secondes = match duree_imposee {
        Some(sec) if sec > 0 => sec,
        _ => total_secondes.max(1),
    };
    // duree_minutes : colonne de compat (ceil à la minute) — le MONTANT est
    // calculé à la seconde (compute_montant).
    let duree_minutes_final = (facture_secondes + 59) / 60;
    // Forfait choisi + dépassement prorata À LA SECONDE (voir compute_montant).
    let allouee = s
        .get("duree_allouee")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(if duree_imposee.is_some() { duree_minutes } else { 0 });
    let montant = compute_montant(tarif_prix, allouee, facture_secondes);

    // Terminer la session. En AUTO (temps écoulé), l'heure de fin est
    // l'INSTANT EXACT d'expiration : debut + secondes accumulées + allocation
    // restante — PAS "maintenant" (le watcher peut tourner en retard, cela ne
    // doit pas décaler l'heure de fin ni le montant).
    let fin_iso = if auto {
        chrono::DateTime::parse_from_rfc3339(debut)
            .map(|d| {
                let offset_s = (facture_secondes - accum_secondes).max(0);
                (d + chrono::Duration::seconds(offset_s))
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            })
            .unwrap_or_else(|_| now_iso())
    } else {
        now_iso()
    };
    let mut upd_session = jmap();
    upd_session.insert("statut".into(), json!("terminee"));
    upd_session.insert("fin".into(), json!(fin_iso));
    upd_session.insert("duree_minutes".into(), json!(duree_minutes_final));
    upd_session.insert("duree_secondes".into(), json!(facture_secondes));
    upd_session.insert("montant".into(), json!(montant));
    if auto {
        // Aligne la durée allouée sur la durée réellement jouée : sinon un pull
        // cloud plus ancien pourrait remettre duree_allouee à la valeur du tarif.
        let allouee = s
            .get("duree_allouee")
            .and_then(Value::as_i64)
            .unwrap_or(duree_minutes_final);
        upd_session.insert("duree_allouee".into(), json!(allouee.max(duree_minutes_final)));
    }
    db.update("sessions_jeu", id, &upd_session)?;

    let mut upd_console = jmap();
    upd_console.insert("etat".into(), json!("disponible"));
    db.update("consoles", console_id, &upd_console)?;

    // Facture
    let random: u32 = rand::random::<u32>() % 10000;
    let numero_facture = format!("FAC-{}-{:04}", today_str(), random);
    let montant_ht = ((montant as f64) / 1.2).round() as i64;
    let taux_tva: i64 = 20;
    let montant_tva = montant - montant_ht;
    let now = now_iso();
    let payment_mode = mode_paiement.as_deref().unwrap_or("especes");
    if !validators::is_valid_mode_paiement(payment_mode) {
        return Err(ApiError::bad_request("Mode de paiement invalide"));
    }

    if payment_mode == "jetons" {
        let valeur_jeton = db
            .find_one("parametres_fidelite", |r| is_active_flag(r.get("actif")))?
            .and_then(|r| r.get("valeur_jeton").and_then(Value::as_i64))
            .unwrap_or(100);
        let jetons_requis = jetons_pour_paiement(montant, valeur_jeton);
        let joueur = get_by_id(db, "joueurs", joueur_id, "Joueur non trouvé")?;
        let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);
        if solde < jetons_requis {
            return Err(ApiError::bad_request(format!(
                "Solde insuffisant : {} jeton(s) requis pour un montant de {} FC (solde: {} jeton(s))",
                jetons_requis, montant, solde
            )));
        }
        let mut upd_joueur = jmap();
        upd_joueur.insert("jetons_solde".into(), json!(solde - jetons_requis));
        db.update("joueurs", joueur_id, &upd_joueur)?;

        let raison = format!("Paiement session #{}", session_id);
        let mut jtx = jmap();
        jtx.insert("joueur_id".into(), json!(joueur_id));
        jtx.insert("type".into(), json!("depense"));
        jtx.insert("quantite".into(), json!(jetons_requis));
        jtx.insert("raison".into(), json!(validators::sanitize_input(&raison, 500)));
        jtx.insert("session_id".into(), json!(session_id));
        jtx.insert("created_at".into(), json!(now.clone()));
        db.insert("jetons_transactions", &jtx)?;
    }

    let mut fac = jmap();
    fac.insert("numero_facture".into(), json!(numero_facture));
    fac.insert("session_id".into(), json!(session_id));
    fac.insert("joueur_id".into(), json!(joueur_id));
    fac.insert("montant_ht".into(), json!(montant_ht));
    fac.insert("taux_tva".into(), json!(taux_tva));
    fac.insert("montant_tva".into(), json!(montant_tva));
    fac.insert("montant_ttc".into(), json!(montant));
    fac.insert("mode_paiement".into(), json!(payment_mode));
    fac.insert("statut".into(), json!("payee"));
    fac.insert("date_paiement".into(), json!(now.clone()));
    fac.insert("created_at".into(), json!(now.clone()));
    let facture = db.insert("factures", &fac)?;
    let facture_id = facture
        .get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::internal("Facture ID manquant"))?;

    // Ligne de facture
    let console_nom = db.find_one("consoles", |c| row_id(c) == Some(console_id))?.and_then(|c| c.get("nom").cloned());
    let jeu_nom = db.find_one("jeux", |j| row_id(j) == s.get("jeu_id").and_then(Value::as_i64))?.and_then(|j| j.get("titre").cloned());
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
    db.insert("lignes_facture", &ligne)?;

    // Fidélité : règle 'temps' (jetons par tranche de durée jouée) OU règle
    // 'montant' (bonus selon le montant de la session : seuil = montant en FC).
    let mut jetons_gagnes: i64 = 0;
    if let Some(regle) = db.find_one("parametres_fidelite", |r| is_active_flag(r.get("actif")))? {
        let jetons = regle
            .get("jetons_attribues")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        match regle.get("regle_type").and_then(Value::as_str) {
            Some("temps") => {
                let seuil = regle.get("seuil").and_then(Value::as_i64).unwrap_or(60);
                jetons_gagnes = (duree_minutes_final / seuil) * jetons;
            }
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
        let joueur = get_by_id(db, "joueurs", joueur_id, "Joueur non trouvé")?;
        let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);
        let mut upd_j = jmap();
        upd_j.insert("jetons_solde".into(), json!(solde + jetons_gagnes));
        db.update("joueurs", joueur_id, &upd_j)?;

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
        db.insert("jetons_transactions", &jtx)?;
    }

    let joueur = get_by_id(db, "joueurs", joueur_id, "Joueur non trouvé")?;
    let lignes: Vec<Value> = db
        .query_all("lignes_facture")?
        .into_iter()
        .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(facture_id))
        .collect();

    let mut enriched_facture = facture.clone();
    if let Value::Object(map) = &mut enriched_facture {
        map.insert(
            "joueur_nom".into(),
            joueur.get("nom").cloned().unwrap_or(Value::Null),
        );
        map.insert("lignes".into(), json!(lignes));
    }

    let session_final = get_by_id(db, "sessions_jeu", id, "Session non trouvée")?;
    Ok(json!({
        "session": session_final,
        "facture": enriched_facture,
        "montant": montant,
        "jetonsGagnes": jetons_gagnes,
        "dureeMinutes": duree_minutes_final,
    }))
}

fn is_active_flag(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_i64)
        .map(|v| v == 1)
        .or_else(|| value.and_then(Value::as_bool))
        .unwrap_or(false)
}

fn jetons_pour_paiement(montant: i64, valeur_jeton: i64) -> i64 {
    if montant <= 0 || valeur_jeton <= 0 {
        return 0;
    }
    ((montant as f64 / valeur_jeton as f64).ceil() as i64).max(1)
}

/// PUT /api/sessions/:id
#[tauri::command]
pub fn sessions_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    console_id: Option<i64>,
    joueur_id: Option<i64>,
    jeu_id: Option<i64>,
    statut: Option<String>,
    duree_minutes: Option<i64>,
    montant: Option<f64>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "sessions_jeu", id, "Session non trouvée")?;
    let mut updates: Map<String, Value> = jmap();
    if let Some(c) = console_id {
        if !validators::is_valid_id(c) {
            return Err(ApiError::bad_request("Console ID invalide"));
        }
        updates.insert("console_id".into(), json!(c));
    }
    if let Some(j) = joueur_id {
        if !validators::is_valid_id(j) {
            return Err(ApiError::bad_request("Joueur ID invalide"));
        }
        updates.insert("joueur_id".into(), json!(j));
    }
    if let Some(j) = jeu_id {
        if !validators::is_valid_id(j) {
            return Err(ApiError::bad_request("Jeu ID invalide"));
        }
        updates.insert("jeu_id".into(), json!(j));
    }
    if let Some(s) = statut {
        if !validators::is_valid_session_statut(&s) {
            return Err(ApiError::bad_request("Statut invalide"));
        }
        updates.insert("statut".into(), json!(s));
    }
    if let Some(d) = duree_minutes {
        if !validators::is_valid_duree(d) {
            return Err(ApiError::bad_request("Durée invalide (1-1000)"));
        }
        // Sur une session ACTIVE, la durée éditable = le temps ALLOUÉ
        // (prolonger/raccourcir la session) : le temps déjà joué ne doit PAS
        // être écrasé. Sur une session terminée, on modifie la durée facturée.
        let statut = get_by_id(db(&state), "sessions_jeu", id, "Session non trouvée")?
            .get("statut")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if statut == "en_cours" || statut == "pause" {
            updates.insert("duree_allouee".into(), json!(d));
        } else {
            updates.insert("duree_minutes".into(), json!(d));
            updates.insert("duree_secondes".into(), json!(d * 60));
            updates.insert("duree_allouee".into(), json!(d));
        }
    }
    if let Some(m) = montant {
        if !validators::is_valid_prix(m) {
            return Err(ApiError::bad_request("Montant invalide (1-1000000)"));
        }
        updates.insert("montant".into(), json!(m));
    }
    db(&state).update("sessions_jeu", id, &updates)
}

/// DELETE /api/sessions/:id
#[tauri::command]
pub fn sessions_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "sessions_jeu", id, "Session non trouvée")?;
    db(&state).remove("sessions_jeu", id)?;
    Ok(json!({ "success": true }))
}

/// POST /api/sessions/:id/restore - Restauration d'une session archivée
#[tauri::command]
pub fn sessions_restore(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let row = db(&state).restore("sessions_jeu", id)?;
    Ok(json!({ "id": id, "restored": true, "session": row }))
}

/// DELETE /api/sessions/:id/permanent - Suppression définitive d'une session
#[tauri::command]
pub fn sessions_permanent_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    db(&state).permanent_delete("sessions_jeu", id)?;
    Ok(json!({ "id": id, "permanently_deleted": true }))
}
