// /api/rapports/ca - chiffre d'affaires et statistiques.

use serde_json::{Value, json};
use tauri::State;

use super::{claims, db};
use crate::error::ApiResult;
use crate::AppState;

/// GET /api/rapports/ca — chiffre d'affaires et statistiques.
///
/// v1.1 — CHIFFRES JUSTES : l'ancienne version chargeait les tables entières en
/// mémoire puis calculait en Rust, en TRONQUANT à 5 000 lignes pour éviter l'OOM
/// Android. Au-delà, le chiffre d'affaires et le nombre de sessions étaient donc
/// silencieusement FAUX (une boutique qui a 6 000 factures voyait un total
/// inférieur à la réalité). Désormais SQLite agrège (SUM/COUNT/GROUP BY/COUNT
/// DISTINCT) : exact quel que soit le volume, sans charger une seule ligne, et
/// en s'appuyant sur les index de date (idx_factures_date, idx_sessions_created).
#[tauri::command]
pub fn rapports_ca(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);

    // Bornes de dates INDEXABLES : [aujourd'hui 00:00Z, demain 00:00Z). Une
    // comparaison de chaînes suffit avec le format ISO, et l'index est utilisé
    // (contrairement à `date(colonne) = ?` qui forçait un balayage complet).
    let today = chrono::Utc::now().date_naive();
    let day_start = today.format("%Y-%m-%d").to_string();
    let day_end = (today + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let today_bounds: [&dyn rusqlite::ToSql; 2] = [&day_start, &day_end];

    // ===== CARTES DU HAUT (agrégats SQL, aucune ligne transférée) =====
    let revenus_aujourd_hui = db.query_scalar_f64(
        "SELECT COALESCE(SUM(CAST(montant_ttc AS REAL)), 0) FROM factures \
         WHERE deleted = 0 AND statut = 'payee' AND date_paiement >= ?1 AND date_paiement < ?2",
        &today_bounds,
    )?;
    let sessions_aujourd_hui = db.query_scalar_i64(
        "SELECT COUNT(*) FROM sessions_jeu \
         WHERE deleted = 0 AND created_at >= ?1 AND created_at < ?2",
        &today_bounds,
    )?;
    let joueurs_actifs = db.query_scalar_i64(
        "SELECT COUNT(DISTINCT joueur_id) FROM sessions_jeu \
         WHERE deleted = 0 AND created_at >= ?1 AND created_at < ?2",
        &today_bounds,
    )?;
    let jetons_attribues = db.query_scalar_i64(
        "SELECT COALESCE(SUM(CAST(quantite AS INTEGER)), 0) FROM jetons_transactions \
         WHERE deleted = 0 AND type = 'gain' AND created_at >= ?1 AND created_at < ?2",
        &today_bounds,
    )?;
    let total_revenus = db.query_scalar_f64(
        "SELECT COALESCE(SUM(CAST(montant_ttc AS REAL)), 0) FROM factures \
         WHERE deleted = 0 AND statut = 'payee'",
        &[],
    )?;
    let total_sessions =
        db.query_scalar_i64("SELECT COUNT(*) FROM sessions_jeu WHERE deleted = 0", &[])?;
    let total_joueurs = db.query_scalar_i64("SELECT COUNT(*) FROM joueurs WHERE deleted = 0", &[])?;
    let total_sessions_f = total_sessions as f64;

    // ===== TOP JEUX / RÉPARTITION CONSOLES (GROUP BY en base) =====
    // LEFT JOIN : un jeu sans aucune session apparaît avec 0 (comportement
    // identique à l'ancien calcul) ; les sessions supprimées sont exclues par la
    // condition de jointure, comme avant.
    let top_jeux: Vec<Value> = db
        .query_rows(
            "SELECT j.titre AS titre, COUNT(s.id) AS sessions \
             FROM jeux j LEFT JOIN sessions_jeu s ON s.jeu_id = j.id AND s.deleted = 0 \
             WHERE j.deleted = 0 GROUP BY j.id ORDER BY sessions DESC LIMIT 5",
            &[],
        )?
        .into_iter()
        .map(|r| {
            let count = r.get("sessions").and_then(Value::as_i64).unwrap_or(0);
            let pct = if total_sessions > 0 {
                ((count as f64) / total_sessions_f * 100.0).round() as i64
            } else {
                0
            };
            json!({
                "titre": r.get("titre").cloned().unwrap_or(Value::Null),
                "sessions": count,
                "pct": pct,
            })
        })
        .collect();

    let repartition_consoles: Vec<Value> = db
        .query_rows(
            "SELECT c.nom AS nom, COUNT(s.id) AS sessions \
             FROM consoles c LEFT JOIN sessions_jeu s ON s.console_id = c.id AND s.deleted = 0 \
             WHERE c.deleted = 0 GROUP BY c.id ORDER BY sessions DESC",
            &[],
        )?
        .into_iter()
        .map(|r| {
            let count = r.get("sessions").and_then(Value::as_i64).unwrap_or(0);
            let pct = if total_sessions > 0 {
                ((count as f64) / total_sessions_f * 100.0).round() as i64
            } else {
                0
            };
            json!({
                "nom": r.get("nom").cloned().unwrap_or(Value::Null),
                "sessions": count,
                "pct": pct,
            })
        })
        .collect();

    // ===== HISTORIQUE 7 JOURS (GROUP BY sur la tranche de 7 jours) =====
    let hist_start = (today - chrono::Duration::days(6)).format("%Y-%m-%d").to_string();
    let hist_bounds: [&dyn rusqlite::ToSql; 2] = [&hist_start, &day_end];
    let hist_sessions = db.query_rows(
        "SELECT substr(created_at, 1, 10) AS d, COUNT(*) AS n FROM sessions_jeu \
         WHERE deleted = 0 AND created_at >= ?1 AND created_at < ?2 GROUP BY d",
        &hist_bounds,
    )?;
    let hist_revenus = db.query_rows(
        "SELECT substr(date_paiement, 1, 10) AS d, COALESCE(SUM(CAST(montant_ttc AS REAL)), 0) AS r \
         FROM factures WHERE deleted = 0 AND statut = 'payee' \
         AND date_paiement >= ?1 AND date_paiement < ?2 GROUP BY d",
        &hist_bounds,
    )?;
    // Les jours sans activité n'apparaissent pas dans un GROUP BY : on complète
    // avec 0 pour que la courbe garde toujours exactement 7 points.
    let count_of = |rows: &[Value], day: &str| -> i64 {
        rows.iter()
            .find(|r| r.get("d").and_then(Value::as_str) == Some(day))
            .and_then(|r| r.get("n").and_then(Value::as_i64))
            .unwrap_or(0)
    };
    let revenus_of = |rows: &[Value], day: &str| -> f64 {
        rows.iter()
            .find(|r| r.get("d").and_then(Value::as_str) == Some(day))
            .and_then(|r| r.get("r").and_then(Value::as_f64))
            .unwrap_or(0.0)
    };
    let mut sessions_history: Vec<Value> = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let ds = (today - chrono::Duration::days(i))
            .format("%Y-%m-%d")
            .to_string();
        sessions_history.push(json!({
            "date": ds[5..].to_string(),
            "count": count_of(&hist_sessions, &ds),
            "revenus": revenus_of(&hist_revenus, &ds),
        }));
    }

    Ok(json!({
        "revenus_aujourd_hui": revenus_aujourd_hui,
        "sessions_aujourd_hui": sessions_aujourd_hui,
        "joueurs_actifs": joueurs_actifs,
        "jetons_attribues": jetons_attribues,
        "total_revenus": total_revenus,
        "total_sessions": total_sessions,
        "total_joueurs": total_joueurs,
        "top_jeux": top_jeux,
        "repartition_consoles": repartition_consoles,
        "sessions_history": sessions_history,
    }))
}