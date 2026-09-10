// /api/rapports/ca - chiffre d'affaires et statistiques.

use serde_json::{Value, json};
use tauri::State;

use super::{claims, db, row_id};
use crate::error::ApiResult;
use crate::AppState;

/// GET /api/rapports/ca
#[tauri::command]
pub fn rapports_ca(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let factures: Vec<Value> = db
        .query_all("factures")?
        .into_iter()
        .filter(|f| f.get("statut").and_then(Value::as_str) == Some("payee"))
        .collect();
    let sessions = db.query_all("sessions_jeu")?;
    let joueurs = db.query_all("joueurs")?;
    let jeux = db.query_all("jeux")?;
    let consoles = db.query_all("consoles")?;
    let jetons_tx = db.query_all("jetons_transactions")?;

    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let date_prefix = |row: &Value, key: &str| -> String {
        row.get(key)
            .and_then(Value::as_str)
            .map(|s| s.chars().take(10).collect())
            .unwrap_or_default()
    };

    let today_factures: Vec<&Value> = factures
        .iter()
        .filter(|f| date_prefix(f, "date_paiement") == today)
        .collect();
    let today_sessions: Vec<&Value> = sessions
        .iter()
        .filter(|s| date_prefix(s, "created_at") == today)
        .collect();

    let total_revenus: f64 = factures
        .iter()
        .map(|f| f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0))
        .sum();
    let total_sessions = sessions.len();

    let mut top_jeux: Vec<Value> = jeux
        .iter()
        .map(|j| {
            let jid = row_id(j);
            let count = sessions
                .iter()
                .filter(|s| s.get("jeu_id").and_then(Value::as_i64) == jid)
                .count();
            let pct = if total_sessions > 0 {
                ((count as f64) / (total_sessions as f64) * 100.0).round() as i64
            } else {
                0
            };
            json!({ "titre": j["titre"], "sessions": count, "pct": pct })
        })
        .collect();
    top_jeux.sort_by(|a, b| {
        b.get("sessions")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .cmp(&a.get("sessions").and_then(Value::as_i64).unwrap_or(0))
    });
    top_jeux.truncate(5);

    let mut repartition_consoles: Vec<Value> = consoles
        .iter()
        .map(|c| {
            let cid = row_id(c);
            let count = sessions.iter().filter(|s| s.get("console_id").and_then(Value::as_i64) == cid).count();
            let pct = if total_sessions > 0 {
                ((count as f64) / (total_sessions as f64) * 100.0).round() as i64
            } else {
                0
            };
            json!({ "nom": c["nom"], "sessions": count, "pct": pct })
        })
        .collect();
    repartition_consoles.sort_by(|a, b| {
        b.get("sessions")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .cmp(&a.get("sessions").and_then(Value::as_i64).unwrap_or(0))
    });

    let mut sessions_history: Vec<Value> = Vec::new();
    for i in (0..7).rev() {
        let d = chrono::Utc::now().date_naive() - chrono::Duration::days(i);
        let ds = d.format("%Y-%m-%d").to_string();
        let count = sessions
            .iter()
            .filter(|s| date_prefix(s, "created_at") == ds)
            .count();
        let revenus: f64 = factures
            .iter()
            .filter(|f| date_prefix(f, "date_paiement") == ds)
            .map(|f| f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0))
            .sum();
        sessions_history.push(json!({
            "date": ds[5..].to_string(),
            "count": count,
            "revenus": revenus,
        }));
    }

    let jetons_attribues_auj: i64 = jetons_tx
        .iter()
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
}