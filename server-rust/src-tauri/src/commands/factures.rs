// /api/factures - liste, détail, création, modification, annulation, PDF.

use base64::Engine;
use serde_json::{Value, json};
use tauri::State;

use crate::commands::{admin_only, claims, db, get_by_id, jmap, row_id, sort_desc_by_created_at};
use crate::error::{ApiError, ApiResult};
use crate::validators;
use crate::AppState;

fn enrich_facture(f: &Value, joueur: Option<&Value>) -> Value {
    let mut o = f.clone();
    if let Value::Object(map) = &mut o {
        map.insert(
            "joueur_nom".into(),
            joueur
                .and_then(|j| j.get("nom"))
                .cloned()
                .unwrap_or_else(|| json!("N/A")),
        );
    }
    o
}

fn is_deleted(v: Option<&Value>) -> bool {
    v.map(|x| x.as_bool().unwrap_or(false) || x.as_i64().unwrap_or(0) == 1).unwrap_or(false)
}

/// GET /api/factures (?statut=&joueur_id=&date_start=&date_end=)
#[tauri::command]
pub fn factures_list(
    state: State<'_, AppState>,
    token: Option<String>,
    statut: Option<String>,
    joueur_id: Option<i64>,
    date_start: Option<String>,
    date_end: Option<String>,
    include_deleted: Option<bool>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    let db = db(&state);
    let include_del = include_deleted.unwrap_or(false);
    // query_all() filtre deja "deleted = 0" en SQL : sans query_all_all() la case
    // "Afficher archivees" ne peut rien remonter.
    let mut factures: Vec<Value> = if include_del {
        db.query_all_all("factures")?
    } else {
        db.query_all("factures")?
    };
    if let Some(s) = statut {
        factures.retain(|f| f.get("statut").and_then(Value::as_str) == Some(s.as_str()));
    }
    if let Some(j) = joueur_id {
        factures.retain(|f| f.get("joueur_id").and_then(Value::as_i64) == Some(j));
    }
    if let Some(ds) = date_start {
        factures.retain(|f| {
            f.get("created_at")
                .and_then(Value::as_str)
                .map(|c| c >= ds.as_str())
                .unwrap_or(false)
        });
    }
    if let Some(de) = date_end {
        let end = format!("{}T23:59:59", de);
        factures.retain(|f| {
            f.get("created_at")
                .and_then(Value::as_str)
                .map(|c| c <= end.as_str())
                .unwrap_or(false)
        });
    }
    if !include_del {
        factures.retain(|f| !is_deleted(f.get("deleted")));
    }
    sort_desc_by_created_at(&mut factures);

    // Une facture archivee pointe souvent un joueur lui-meme archive : on lit
    // toutes les lignes pour ne pas perdre le nom du client.
    let joueurs = db.query_all_all("joueurs")?;
    Ok(Value::Array(
        factures
            .iter()
            .map(|f| {
                let joueur = f
                    .get("joueur_id")
                    .and_then(Value::as_i64)
                    .and_then(|jid| joueurs.iter().find(|j| row_id(j) == Some(jid)));
                enrich_facture(f, joueur)
            })
            .collect(),
    ))
}

/// GET /api/factures/:id
#[tauri::command]
pub fn factures_get(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let f = get_by_id(db, "factures", id, "Facture non trouvée")?;
    let joueur = f
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());

    let lignes: Vec<Value> = db
        .query_all("lignes_facture")?
        .into_iter()
        .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id))
        .collect();

    let mut o = enrich_facture(&f, joueur.as_ref());
    let num_fac = f.get("numero_facture").and_then(Value::as_str).unwrap_or("");
    let mut transactions: Vec<Value> = db
        .query_all("jetons_transactions")?
        .into_iter()
        .filter(|t| {
            if t.get("facture_id").and_then(Value::as_i64) == Some(id) {
                return true;
            }
            if !num_fac.is_empty() {
                let r = t.get("raison").and_then(Value::as_str).unwrap_or("");
                if r.contains(num_fac) {
                    return true;
                }
            }
            false
        })
        .collect();
    sort_desc_by_created_at(&mut transactions);

    if let Value::Object(map) = &mut o {
        map.insert(
            "joueur_telephone".into(),
            joueur
                .as_ref()
                .and_then(|j| j.get("telephone"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert("lignes".into(), json!(lignes));
        map.insert("jetons_transactions".into(), json!(transactions));
    }
    Ok(o)
}

/// GET /api/factures/:id/pdf (renvoie le PDF en base64)
#[tauri::command]
pub fn factures_pdf(state: State<'_, AppState>, token: Option<String>, id: i64) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let f = get_by_id(db, "factures", id, "Facture non trouvée")?;
    let joueur = f
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
    let lignes: Vec<Value> = db
        .query_all("lignes_facture")?
        .into_iter()
        .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id))
        .collect();

    // Le nom de l'établissement (Paramètres > nom de l'application) est imprimé
    // en en-tête et en pied de page de la facture.
    let nom_app = db
        .get_setting("app_name")
        .ok()
        .flatten()
        .unwrap_or_else(|| "Game Lounge".to_string());
    let pdf = crate::pdf::facture_pdf(&f, joueur.as_ref(), &lignes, &nom_app)
        .map_err(|e| ApiError::internal(format!("Erreur génération PDF: {e}")))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(pdf);
    Ok(json!({ "pdf_base64": b64 }))
}

/// POST factures_save_pdf : enregistre le PDF d'une facture dans un dossier de
/// l'appareil (par défaut /storage/emulated/0/Documents/GameLounge/Factures).
///
/// Android 10+ bloque l'accès direct au stockage partagé -> si l'écriture directe
/// échoue (permission), on passe par le SAF (Storage Access Framework) côté
/// Android : l'appareil retourne `saf_required: true` et le frontend demande à
/// l'utilisateur de choisir le dossier via le sélecteur système (le script de
/// patch Android déclare le répertoire via ACTION_OPEN_DOCUMENT_TREE ; les
/// écritures suivantes utilisent les URIs persistés). Le dossier choisi est
/// mémorisé côté frontend (localStorage gl_pdf_dir) pour les exports suivants.
#[tauri::command]
pub fn factures_save_pdf(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    folder: Option<String>,
    filename: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    let f = get_by_id(db, "factures", id, "Facture non trouvée")?;
    let joueur = f
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
    let lignes: Vec<Value> = db
        .query_all("lignes_facture")?
        .into_iter()
        .filter(|l| l.get("facture_id").and_then(Value::as_i64) == Some(id))
        .collect();

    // Le nom de l'établissement (Paramètres > nom de l'application) est imprimé
    // en en-tête et en pied de page de la facture.
    let nom_app = db
        .get_setting("app_name")
        .ok()
        .flatten()
        .unwrap_or_else(|| "Game Lounge".to_string());
    let pdf = crate::pdf::facture_pdf(&f, joueur.as_ref(), &lignes, &nom_app)
        .map_err(|e| ApiError::internal(format!("Erreur génération PDF: {e}")))?;

    // Dossier par défaut : /storage/emulated/0/Documents/GameLounge/Factures
    let dir = folder
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("/storage/emulated/0/Documents/GameLounge/Factures");
    // Nom de fichier sûr : numero de facture + .pdf (sanitize basique).
    let numero = f.get("numero_facture").and_then(Value::as_str).unwrap_or("facture");
    let safe = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
            .collect()
    };
    let name = safe(filename.as_deref().unwrap_or(&format!("{}.pdf", numero)));
    let name = if name.ends_with(".pdf") { name } else { format!("{}.pdf", name) };

    let full = format!("{}/{}", dir.trim_end_matches('/'), name);
    // Crée les dossiers parents (ignorer l'erreur si le FS le refuse : on
    // remonte l'erreur d'écriture réelle ensuite).
    let _ = std::fs::create_dir_all(dir);
    match std::fs::write(&full, &pdf) {
        Ok(_) => Ok(json!({
            "success": true,
            "path": full,
            "size": pdf.len(),
        })),
        Err(e) => {
            // Android 10+ : écriture directe refusée -> le frontend doit passer
            // par le SAF (sélecteur de dossier système). On renvoie le PDF en
            // base64 pour que l'écriture SAF se fasse côté frontend via le
            // DocumentFile créé par le sélecteur.
            crate::logger::log("pdf", &format!("écriture directe refusée ({}), fallback SAF", e));
            let b64 = base64::engine::general_purpose::STANDARD.encode(&pdf);
            Ok(json!({
                "success": false,
                "saf_required": true,
                "suggested_path": full,
                "filename": name,
                "pdf_base64": b64,
                "error": format!("Accès direct refusé ({e}) — utilisez le sélecteur de dossier Android"),
            }))
        }
    }
}

/// PUT /api/factures/:id/annuler
#[tauri::command]
pub fn factures_annuler(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let db = db(&state);
    get_by_id(db, "factures", id, "Facture non trouvée")?;
    let mut upd = jmap();
    upd.insert("statut".into(), json!("annulee"));
    let updated = db.update("factures", id, &upd)?;
    let joueur = updated
        .get("joueur_id")
        .and_then(Value::as_i64)
        .and_then(|jid| db.find_one("joueurs", |j| row_id(j) == Some(jid)).ok().flatten());
    Ok(enrich_facture(&updated, joueur.as_ref()))
}

/// POST /api/factures
#[tauri::command]
pub fn factures_create(
    state: State<'_, AppState>,
    token: Option<String>,
    session_id: i64,
    joueur_id: i64,
    montant_ht: Option<f64>,
    taux_tva: Option<f64>,
    montant_tva: Option<f64>,
    montant_ttc: f64,
    mode_paiement: Option<String>,
    statut: Option<String>,
) -> ApiResult<Value> {
    claims(&state, &token)?;
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
    let now = crate::db::now_iso();
    let mut row = jmap();
    row.insert(
        "numero_facture".into(),
        json!(format!("FAC-{}-{:04}", crate::db::today_str(), random)),
    );
    row.insert("session_id".into(), json!(session_id));
    row.insert("joueur_id".into(), json!(joueur_id));
    row.insert("montant_ht".into(), json!(montant_ht.unwrap_or(0.0)));
    row.insert("taux_tva".into(), json!(taux_tva.unwrap_or(20.0)));
    row.insert("montant_tva".into(), json!(montant_tva.unwrap_or(0.0)));
    row.insert("montant_ttc".into(), json!(montant_ttc));
    row.insert(
        "mode_paiement".into(),
        json!(mode_paiement.unwrap_or_else(|| "especes".into())),
    );
    row.insert("statut".into(), json!(statut.unwrap_or_else(|| "payee".into())));
    row.insert("date_paiement".into(), json!(now.clone()));
    row.insert("created_at".into(), json!(now));
    db(&state).insert("factures", &row)
}

/// PUT /api/factures/:id
#[tauri::command]
pub fn factures_update(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
    statut: Option<String>,
    mode_paiement: Option<String>,
    montant_ttc: Option<f64>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    let is_admin = user.is_admin();
    if !is_admin {
        if user.role != "employe" {
            return Err(ApiError::forbidden("Accès réservé aux administrateurs et employés"));
        }
        if statut.is_some() || montant_ttc.is_some() || mode_paiement.is_none() {
            return Err(ApiError::forbidden("Un employé peut uniquement modifier le mode de paiement"));
        }
    };
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "factures", id, "Facture non trouvée")?;
    let mut updates = jmap();
    if is_admin {
        if let Some(s) = statut {
            if !validators::is_valid_facture_statut(&s) {
                return Err(ApiError::bad_request("Statut invalide"));
            }
            updates.insert("statut".into(), json!(s));
        }
    }
    let facture = get_by_id(db(&state), "factures", id, "Facture non trouvée")?;
    let old_mode = facture.get("mode_paiement").and_then(Value::as_str).unwrap_or("especes");

    if let Some(ref m) = mode_paiement {
        if !validators::is_valid_mode_paiement(m) {
            return Err(ApiError::bad_request("Mode paiement invalide"));
        }
        updates.insert("mode_paiement".into(), json!(m));

        let num_fac = facture.get("numero_facture").and_then(Value::as_str).unwrap_or("FAC");
        let joueur_id = facture.get("joueur_id").and_then(Value::as_i64).unwrap_or(0);
        let session_id = facture.get("session_id").and_then(Value::as_i64);

        // Déduction de jetons si le paiement bascule sur 'jetons'
        if m == "jetons" && old_mode != "jetons" {
            let montant_final = montant_ttc.unwrap_or_else(|| facture.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0)) as i64;
            let valeur_jeton = db(&state)
                .find_one("parametres_fidelite", |_| true)
                .ok()
                .flatten()
                .and_then(|r| r.get("valeur_jeton").and_then(Value::as_i64))
                .unwrap_or(100);
            let jetons_requis = if montant_final <= 0 || valeur_jeton <= 0 { 1 } else { ((montant_final as f64 / valeur_jeton as f64).ceil() as i64).max(1) };
            
            if joueur_id > 0 {
                let joueur = get_by_id(db(&state), "joueurs", joueur_id, "Joueur introuvable")?;
                let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);
                if solde < jetons_requis {
                    return Err(ApiError::bad_request(&format!(
                        "Solde insuffisant : {} jeton(s) requis pour {} FC (solde actuel : {} jeton(s))",
                        jetons_requis, montant_final, solde
                    )));
                }
                let mut upd_j = jmap();
                upd_j.insert("jetons_solde".into(), json!(solde - jetons_requis));
                db(&state).update("joueurs", joueur_id, &upd_j)?;

                let mut jtx = jmap();
                jtx.insert("joueur_id".into(), json!(joueur_id));
                jtx.insert("type".into(), json!("depense"));
                jtx.insert("quantite".into(), json!(jetons_requis));
                jtx.insert("facture_id".into(), json!(id));
                if let Some(s) = session_id {
                    jtx.insert("session_id".into(), json!(s));
                }
                let raison = format!("Débit changement mode paiement: {} -> jetons ({})", old_mode, num_fac);
                jtx.insert("raison".into(), json!(validators::sanitize_input(&raison, 500)));
                jtx.insert("created_at".into(), json!(crate::db::now_iso()));
                db(&state).insert("jetons_transactions", &jtx)?;
            }
        } else if old_mode == "jetons" && m != "jetons" {
            // Remboursement de jetons si le paiement bascule de 'jetons' vers un autre mode
            let montant_final = facture.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0) as i64;
            let valeur_jeton = db(&state)
                .find_one("parametres_fidelite", |_| true)
                .ok()
                .flatten()
                .and_then(|r| r.get("valeur_jeton").and_then(Value::as_i64))
                .unwrap_or(100);

            let txs: Vec<Value> = db(&state).query_all("jetons_transactions").unwrap_or_default();
            let jetons_deja_payes = txs.iter().find(|t| {
                t.get("facture_id").and_then(Value::as_i64) == Some(id)
                    && t.get("type").and_then(Value::as_str) == Some("depense")
            }).or_else(|| {
                txs.iter().find(|t| {
                    t.get("type").and_then(Value::as_str) == Some("depense")
                        && t.get("raison").and_then(Value::as_str).unwrap_or("").contains(num_fac)
                })
            }).and_then(|t| t.get("quantite").and_then(Value::as_i64));

            let jetons_remboursement = jetons_deja_payes.unwrap_or_else(|| {
                if montant_final <= 0 || valeur_jeton <= 0 { 1 } else { ((montant_final as f64 / valeur_jeton as f64).ceil() as i64).max(1) }
            });

            if joueur_id > 0 && jetons_remboursement > 0 {
                let joueur = get_by_id(db(&state), "joueurs", joueur_id, "Joueur introuvable")?;
                let solde = joueur.get("jetons_solde").and_then(Value::as_i64).unwrap_or(0);
                let mut upd_j = jmap();
                upd_j.insert("jetons_solde".into(), json!(solde + jetons_remboursement));
                db(&state).update("joueurs", joueur_id, &upd_j)?;

                let mut jtx = jmap();
                jtx.insert("joueur_id".into(), json!(joueur_id));
                jtx.insert("type".into(), json!("gain"));
                jtx.insert("quantite".into(), json!(jetons_remboursement));
                jtx.insert("facture_id".into(), json!(id));
                if let Some(s) = session_id {
                    jtx.insert("session_id".into(), json!(s));
                }
                let raison = format!("Remboursement changement mode paiement: jetons -> {} ({})", m, num_fac);
                jtx.insert("raison".into(), json!(validators::sanitize_input(&raison, 500)));
                jtx.insert("created_at".into(), json!(crate::db::now_iso()));
                db(&state).insert("jetons_transactions", &jtx)?;
            }
        }
    }
    if is_admin {
        if let Some(m) = montant_ttc {
            if !validators::is_valid_prix(m) {
                return Err(ApiError::bad_request("Montant invalide (1-1000000)"));
            }
            updates.insert("montant_ttc".into(), json!(m));
        }
    }
    db(&state).update("factures", id, &updates)
}

/// DELETE /api/factures/:id
#[tauri::command]
pub fn factures_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    get_by_id(db(&state), "factures", id, "Facture non trouvée")?;
    db(&state).remove("factures", id)?;
    Ok(json!({ "success": true }))
}

/// POST /api/factures/:id/restore - Restauration d'une facture archivée
#[tauri::command]
pub fn factures_restore(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    let row = db(&state).restore("factures", id)?;
    Ok(json!({ "id": id, "restored": true, "facture": row }))
}

/// DELETE /api/factures/:id/permanent - Suppression définitive d'une facture
#[tauri::command]
pub async fn factures_permanent_delete(
    state: State<'_, AppState>,
    token: Option<String>,
    id: i64,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    if !validators::is_valid_id(id) {
        return Err(ApiError::bad_request("ID invalide"));
    }
    if let Ok(pool) = crate::supabase::get_supabase_pool(&state).await {
        let sql = format!("DELETE FROM lignes_facture WHERE facture_id = {}; DELETE FROM factures WHERE id = {};", id, id);
        let _ = crate::supabase::supabase_batch_execute(&pool, &sql).await;
    }
    db(&state).permanent_delete("factures", id)?;
    Ok(json!({ "id": id, "permanently_deleted": true }))
}
