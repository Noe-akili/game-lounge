// /api/sync - statut et synchronisation Supabase (offline-first best practice)
use serde_json::{Value, json};
use tauri::Manager;
use tauri::State;

use crate::commands::{admin_only, claims, db};
use crate::db::now_iso;
use crate::error::ApiResult;
use crate::AppState;

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

fn set_sync_state<'a>(state: &'a State<'_, AppState>, v: Value) {
    if let Ok(mut guard) = state.sync_state.lock() {
        *guard = Some(v);
    }
}

fn local_status(state: &State<'_, AppState>) -> ApiResult<Value> {
    let has_local = !db(state).query_all("users")?.is_empty();
    // Toggle PERSISTÉ en SQLite : reste activé après sortie des paramètres / redémarrage
    let sync_enabled = db(state).get_setting("sync_enabled").ok().and_then(|o| o).unwrap_or_default() == "1";
    #[cfg(feature = "supabase-sync")]
    let (supabase_enabled, supabase_available) = {
        let pool_opt = state.supabase_pool.lock().ok().and_then(|g| g.clone());
        // Fallback URL hardcodé assure Supabase toujours enabled (même sans .env)
        let enabled = true;
        let available = pool_opt.is_some();
        let _ = std::env::var("DATABASE_URL").is_ok(); // garde compat
        (enabled, available)
    };
    #[cfg(not(feature = "supabase-sync"))]
    let (supabase_enabled, supabase_available) = (false, false);

    let sync_state = state.sync_state.lock().ok().and_then(|g| g.clone());
    let syncing = sync_state.clone().and_then(|v| v.get("running").and_then(Value::as_bool)).unwrap_or(false);
    let last_sync_val: Value = sync_state
        .map(|v| v.get("finished_at").cloned().unwrap_or(Value::Null))
        .unwrap_or(Value::Null);

    Ok(json!({
        "supabaseEnabled": supabase_enabled,
        "supabaseAvailable": supabase_available,
        "supabaseConnected": supabase_available,
        "enabled": sync_enabled,
        "syncing": syncing,
        "hasLocalData": has_local,
        "lastSync": last_sync_val,
        "mode": if supabase_available { "cloud" } else { "offline" },
        "message": if supabase_enabled && !supabase_available { "Supabase configuré (fallback) mais offline - données locales" } else { "" }
    }))
}

/// GET /api/sync/status
#[tauri::command]
pub fn sync_status(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    local_status(&state)
}

/// POST /api/sync/toggle
#[tauri::command]
pub fn sync_toggle(
    state: State<'_, AppState>,
    token: Option<String>,
    enabled: bool,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    // Persistance : le toggle survit à la sortie des paramètres et au redémarrage
    let _ = db(&state).set_setting("sync_enabled", if enabled { "1" } else { "0" });
    eprintln!("[sync] toggle enabled={} (persisté)", enabled);
    local_status(&state)
}

/// POST /api/sync/run - Best practice : pull Supabase -> local, puis push local -> Supabase.
/// IMPORTANT : la sync complète (11 tables pull + 11 tables push, ~40 requêtes Supabase
/// séquentielles) dépasse le timeout IPC Android WebView (~20s) si on attend la fin.
/// On lance donc le travail EN ARRIÈRE-PLAN et on retourne immédiatement ; le
/// frontend suit la progression via /sync/poll ET les événements sync-progress.
///
/// Mission §12 : un appareil NEUF (première installation) doit pouvoir initialiser
/// sa base même connecté en tant qu'employé. La commande admin_only reste réservée
/// aux resynchronisations manuelles ; l'initialisation est permise à tout
/// utilisateur authentifié tant que initial_sync_completed = false.
#[tauri::command(async)]
pub async fn sync_run(app: tauri::AppHandle, state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    #[cfg(feature = "supabase-sync")]
    let initial_pending = !initial_sync_completed(db(&state));
    #[cfg(not(feature = "supabase-sync"))]
    let initial_pending = false;
    if !initial_pending {
        admin_only(&user)?;
    }
    eprintln!("[sync] sync_run demandé par {} (initial_pending={})", user.email, initial_pending);

    #[cfg(feature = "supabase-sync")]
    {
        // Une seule sync à la fois (évite les écritures concurrentes SQLite/Supabase)
        let running = state.sync_state.lock().ok()
            .and_then(|g| g.clone())
            .and_then(|v| v.get("running").and_then(Value::as_bool))
            .unwrap_or(false);
        if running {
            eprintln!("[sync] sync déjà en cours, ignorée");
            return Ok(json!({
                "success": true,
                "started": false,
                "message": "Sync déjà en cours, patientez..."
            }));
        }
        set_sync_state(&state, json!({
            "running": true,
            "step": "démarrage",
            "started_at": now_iso(),
            "finished_at": Value::Null,
            "success": false,
            "message": "Sync en cours..."
        }));

        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(st) = handle.try_state::<crate::AppState>() else { return };
            let t0 = now_ms();
            match run_sync_impl(&handle, &st).await {
                Ok(v) => {
                    let mut o = v;
                    if let Value::Object(map) = &mut o {
                        map.insert("running".into(), json!(false));
                        map.insert("finished_at".into(), json!(now_iso()));
                        map.insert("duration_ms".into(), json!(now_ms() - t0));
                    }
                    set_sync_state(&st, o);
                }
                Err(e) => {
                    eprintln!("[sync] sync_run error: {}", e.message);
                    set_sync_state(&st, json!({
                        "running": false,
                        "step": "erreur",
                        "started_at": now_iso(),
                        "finished_at": now_iso(),
                        "success": false,
                        "message": format!("Sync échouée: {}", e.message),
                    }));
                }
            }
        });

        return Ok(json!({
            "success": true,
            "started": true,
            "message": "Sync lancée en arrière-plan (résultat dans quelques secondes)"
        }));
    }

    #[cfg(not(feature = "supabase-sync"))]
    {
        let _ = db(&state).query_all("users")?;
        return Ok(json!({
            "success": true,
            "message": "Synchronisation locale terminée (Supabase non configuré)",
            "timestamp": now_iso()
        }));
    }
}

/// Applique les lignes d'une table venue du cloud SANS journaliser dans l'outbox
/// (les données viennent du cloud, les renvoyer serait un aller-retour inutile)
/// et SANS écraser un changement local non envoyé (conflits gérés par Db).
/// Compte uniquement les changements RÉELS (applied/tombstone) pour une
/// progression vraie (mission §7).
#[cfg(feature = "supabase-sync")]
fn apply_rows(db: &crate::db::Db, table: &str, rows: &[Value]) -> usize {
    let mut applied = 0;
    for row in rows {
        let Some(obj) = row.as_object() else { continue };
        let id = obj.get("id").and_then(Value::as_i64).unwrap_or(0);
        if id == 0 {
            continue;
        }
        match db.apply_remote_change(table, id, obj, "", "") {
            Ok(status) => {
                if status == "applied" || status == "tombstone" {
                    applied += 1;
                }
            }
            Err(e) => eprintln!("[sync] pull {} #{} failed: {}", table, id, e.message),
        }
    }
    if applied > 0 {
        eprintln!("[sync] pull {}: {} rows écrites", table, applied);
    }
    applied
}

/// Tables de la sync initiale dans l'ordre (données de référence d'abord, puis
/// données d'exploitation). Dérivé de crate::db::TABLES — aucune table inventée.
#[cfg(feature = "supabase-sync")]
const SYNC_PHASES: [&str; 11] = [
    "users", "consoles", "jeux", "joueurs", "tarifs", "parametres_fidelite",
    "messages", "sessions_jeu", "jetons_transactions", "factures", "lignes_facture",
];

/// Tables d'init déjà terminées (persistées dans app_settings, JSON array).
/// Reprise après interruption : à 62 % puis fermeture de l'app, on ne redémarre
/// PAS depuis zéro (mission §11) — les tables cochées sont sautées.
#[cfg(feature = "supabase-sync")]
fn init_tables_done(db: &crate::db::Db) -> Vec<String> {
    let raw = db.get_setting("initial_sync_tables_done").ok().and_then(|o| o).unwrap_or_default();
    serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
}

#[cfg(feature = "supabase-sync")]
fn mark_init_table_done(db: &crate::db::Db, table: &str) {
    let mut done = init_tables_done(db);
    if !done.iter().any(|t| t == table) {
        done.push(table.to_string());
        let _ = db.set_setting("initial_sync_tables_done", &serde_json::to_string(&done).unwrap_or_default());
    }
}

/// La première synchronisation (restauration complète) est-elle déjà terminée ?
#[cfg(feature = "supabase-sync")]
fn initial_sync_completed(db: &crate::db::Db) -> bool {
    db.get_setting("initial_sync_completed").ok().and_then(|o| o).unwrap_or_default() == "1"
}

/// Expose l'état d'initialisation au frontend (mission §5) : réutilise la structure
/// existante (app_settings + curseur sync_state), ne crée AUCUNE nouvelle table.
#[tauri::command]
pub fn sync_initial_status(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let _ = claims(&state, &token)?;
    #[cfg(feature = "supabase-sync")]
    let (completed, tables_done) = {
        let database = db(&state);
        (initial_sync_completed(database), init_tables_done(database))
    };
    #[cfg(not(feature = "supabase-sync"))]
    let (completed, tables_done) = (true, Vec::<String>::new());
    Ok(json!({
        "initialSyncCompleted": completed,
        "tablesDone": tables_done,
        "phases": SYNC_PHASES,
    }))
}

#[cfg(feature = "supabase-sync")]
fn set_step(state: &State<'_, AppState>, step: &str, message: &str) {
    set_sync_state(state, json!({
        "running": true,
        "step": step,
        "message": message,
    }));
}

pub async fn run_sync_impl(app: &tauri::AppHandle, state: &State<'_, AppState>) -> ApiResult<Value> {
    // Ce run est-il la première synchronisation (restauration complète) ? L'événement
    // final sync-completed est alors déjà émis par la branche initiale (mission §8 :
    // pas d'événement dupliqué).
    let was_initial_sync = {
        let cur = db(state).sync_cursor_get().unwrap_or(0);
        let completed = initial_sync_completed(db(state));
        cur == 0 && !completed
    };
    let pool_opt = {
        let guard = state.supabase_pool.lock().map_err(|_| crate::error::ApiError::internal("supabase lock"))?;
        guard.clone()
    };
    let Some(pool) = pool_opt else {
        eprintln!("[sync] Supabase pool non disponible (offline) - données locales conservées");
        return Ok(json!({
            "success": true,
            "message": "Mode offline: données locales utilisées (Supabase sera synchronisé à la reconnexion)",
            "timestamp": now_iso()
        }));
    };

    // 0) Migrations du schéma cloud (idempotentes). La colonne deleted + les index
    //    uniques id ne partent qu'UNE FOIS (flag persistant).
    match crate::supabase::ensure_cloud_schema(&pool).await {
        Ok(_) => {}
        Err(e) => eprintln!("[sync] migration schéma cloud failed: {}", e),
    }
    match crate::supabase::ensure_sync_schema(&pool).await {
        Ok(_) => {}
        Err(e) => eprintln!("[sync] migration delta sync failed: {}", e),
    }
    let migrated = db(state).get_setting("cloud_migration_v2").ok().and_then(|o| o).unwrap_or_default();
    if migrated != "1" {
        crate::supabase::ensure_deleted_columns(&pool).await;
        for table in crate::db::TABLES {
            let _ = crate::supabase::ensure_table_unique_id(&pool, table).await;
        }
        let _ = db(state).set_setting("cloud_migration_v2", "1");
        eprintln!("[sync] migration v2 cloud appliquée (deleted + index uniques)");
    }

    // 1) Reprise : les événements SENDING sans ACK (coupure/crash) repassent PENDING.
    db(state).outbox_reset_stale().ok();

    // 2) UPLOAD delta : batches de 100 PENDING -> Supabase -> ACK.
    //    Si le réseau coupe au milieu, les non-ACKés restent PENDING : le prochain
    //    cycle reprend exactement où il s'est arrêté (spec §6) et l'upsert cloud est
    //    idempotent (spec §7) : un changement renvoyé est simplement re-ACKé.
    set_step(state, "upload", "Envoi des changements locaux...");
    let mut uploaded = 0;
    // Progression upload : total = PENDING actuels (lu UNE fois, l'outbox se vide
    // pendant l'envoi ; les nouveaux changements partiront au cycle suivant).
    let upload_total = db(state).outbox_pending_count().unwrap_or(0) as u64;
    if upload_total > 0 {
        let up = crate::supabase::SyncProgress::new("envoi", "Envoi des changements locaux", "uploading");
        crate::supabase::emit_progress(app, &up.with_counts(0, upload_total, 0), None);
    }
    loop {
        let batch = db(state).outbox_pending(100).unwrap_or_default();
        if batch.is_empty() {
            break;
        }
        match crate::supabase::push_outbox_batch(&pool, &batch).await {
            Ok((acked, failed)) => {
                db(state).outbox_mark(&acked, "ACKED").ok();
                db(state).outbox_mark(&failed, "FAILED").ok();
                uploaded += acked.len();
                if upload_total > 0 {
                    let up = crate::supabase::SyncProgress::new("envoi", "Envoi des changements locaux", "uploading");
                    crate::supabase::emit_progress(app, &up.with_counts(uploaded as u64, upload_total, uploaded as u64), None);
                }
                eprintln!("[sync] upload lot: {} ACKed, {} FAILED", acked.len(), failed.len());
                if acked.is_empty() {
                    break; // échec réseau/applicatif : reprise au prochain cycle
                }
            }
            Err(e) => {
                eprintln!("[sync] upload failed: {} (reprise au prochain cycle)", e);
                crate::logger::log_cloud(&format!("sync_run: upload failed: {}", e));
                break;
            }
        }
    }
    db(state).sync_uploaded_set(uploaded as i64).ok();

    // 3) DOWNLOAD delta : uniquement les changements après le curseur (spec §4).
    let mut cursor = db(state).sync_cursor_get().unwrap_or(0);
    let mut downloaded = 0;
    if cursor == 0 && !initial_sync_completed(db(state)) {
        // ===== PREMIÈRE SYNC (mission §6-§11) =====
        // Restauration par TABLE, avec progression RÉELLE émise au WebView et
        // reprise après interruption (tables déjà faites persistées dans
        // app_settings). Réutilise pull_table (fallback résilient existant) :
        // une table absente du cloud (base neuve) est simplement vide.
        set_sync_state(state, json!({ "running": true, "step": "initial", "message": "Première synchronisation..." }));
        let progress = crate::supabase::SyncProgress::new("préparation", "Préparation de la synchronisation initiale", "downloading");
        crate::supabase::emit_progress(app, &progress.with_counts(0, SYNC_PHASES.len() as u64, 0), Some("sync-started"));

        let mut done = init_tables_done(db(state));
        let phases_total = SYNC_PHASES.len() as u64;
        let mut phase_idx: u64 = 0;
        for table in SYNC_PHASES {
            phase_idx += 1;
            if done.iter().any(|t| t == table) {
                continue; // déjà fait lors d'une session précédente -> reprise (mission §11)
            }
            let label = format!("Téléchargement de {}", table.replace('_', " "));
            let p = crate::supabase::SyncProgress::new(table, &label, "downloading");
            crate::supabase::emit_progress(app, &p.clone().with_counts(phase_idx - 1, phases_total, downloaded as u64), None);
            set_step(state, "initial", &label);
            crate::logger::log_sync(&format!("initial sync: pull {} (phase {}/{})", table, phase_idx, phases_total));

            match crate::supabase::pull_table(&pool, table).await {
                Ok(rows) => {
                    downloaded += apply_rows(db(state), table, &rows);
                    mark_init_table_done(db(state), table);
                    done.push(table.to_string());
                    let pf = crate::supabase::SyncProgress::new(table, &label, "phase_done");
                    crate::supabase::emit_progress(app, &pf.with_counts(phase_idx, phases_total, downloaded as u64), None);
                    crate::logger::log_sync(&format!("initial sync: {} -> {} lignes reçues (total {})", table, rows.len(), downloaded));
                }
                Err(e) => {
                    // Réseau coupé / timeout / Supabase indisponible : on s'arrête
                    // PROPREMENT. Les tables déjà terminées sont persistées -> la
                    // reprise (Réessayer ou relance de l'app) continue la table
                    // suivante sans rien recommencer ni rien supprimer (mission §10-11).
                    eprintln!("[sync] initial pull {} failed: {}", table, e.message);
                    crate::logger::log_sync(&format!("initial sync interrompue à {} : {}", table, e.message));
                    let err = crate::supabase::SyncProgress::new(table, &format!("Impossible de récupérer les données : {}", e.message), "error");
                    crate::supabase::emit_progress(app, &err.with_counts(phase_idx - 1, phases_total, downloaded as u64), Some("sync-error"));
                    set_sync_state(state, json!({
                        "running": false,
                        "step": "erreur",
                        "started_at": now_iso(),
                        "finished_at": now_iso(),
                        "success": false,
                        "message": format!("Synchronisation initiale interrompue à {}: {}", table, e.message),
                    }));
                    return Ok(json!({
                        "success": false,
                        "step": "erreur",
                        "initialSync": true,
                        "phase": table,
                        "message": format!("Impossible de récupérer les données : {}. Réessayez.", e.message),
                        "timestamp": now_iso()
                    }));
                }
            }
        }

        // Toutes les tables sont faites : seed de l'outbox avec les données locales
        // pré-existantes (elles n'ont jamais été journalisées) puis curseur -> fin du
        // journal cloud (les prochains logins passeront en DELTA, mission §4/§12).
        let seeded = db(state).outbox_seed().unwrap_or(0);
        let max_seq = crate::supabase::sync_max_sequence(&pool).await.unwrap_or(0);
        db(state).sync_cursor_set(max_seq).ok();
        let _ = db(state).set_setting("initial_sync_completed", "1");
        eprintln!("[sync] première sync terminée: {} reçues, {} événements seedés, curseur {}", downloaded, seeded, max_seq);
        let done_p = crate::supabase::SyncProgress::new("finalisation", "Synchronisation terminée", "completed");
        crate::supabase::emit_progress(app, &done_p.with_counts(phases_total, phases_total, downloaded as u64), Some("sync-completed"));
    } else {
        // SYNC DELTA : boucle de batches de 500 changements.
        set_step(state, "pull", "Réception des nouveaux changements...");
        // Progression delta : total = changements restants dans le journal cloud.
        let delta_total = crate::supabase::sync_changes_count_after(&pool, cursor).await.unwrap_or(0).max(0) as u64;
        if delta_total > 0 {
            let dl = crate::supabase::SyncProgress::new("delta", "Réception des nouveaux changements", "downloading");
            crate::supabase::emit_progress(app, &dl.with_counts(0, delta_total, 0), None);
        }
        loop {
            let batch = crate::supabase::pull_delta(&pool, cursor, 500).await.unwrap_or_default();
            if batch.is_empty() {
                break;
            }
            let first_seq = batch[0].get("sequence").and_then(Value::as_i64).unwrap_or(0);
            // SNAPSHOT_REQUIRED (spec §11) : trou de séquence -> le journal a été
            // nettoyé ou l'appareil est resté trop longtemps hors ligne -> on
            // restaure un snapshot complet puis on reprend en delta.
            if first_seq > cursor + 1 {
                eprintln!("[sync] trou de séquence (curseur {}, reçu {}) -> SNAPSHOT_REQUIRED", cursor, first_seq);
                match crate::supabase::pull_all(&pool).await {
                    Ok(all) => {
                        for (table, rows) in &all {
                            downloaded += apply_rows(db(state), table, rows);
                        }
                        let max_seq = crate::supabase::sync_max_sequence(&pool).await.unwrap_or(0);
                        db(state).sync_cursor_set(max_seq).ok();
                        eprintln!("[sync] snapshot appliqué, curseur remis à {}", max_seq);
                    }
                    Err(e) => {
                        eprintln!("[sync] snapshot pull failed: {}", e.message);
                        crate::supabase::schedule_reconnect(app);
                    }
                }
                break;
            }
            let mut last_seq = cursor;
            let batch_len = batch.len();
            for change in &batch {
                let seq = change.get("sequence").and_then(Value::as_i64).unwrap_or(0);
                let entity = change.get("entity").and_then(Value::as_str).unwrap_or_default();
                let record_id = change.get("record_id").and_then(Value::as_i64).unwrap_or(0);
                let change_id = change.get("change_id").and_then(Value::as_str).unwrap_or_default();
                let created_at = change.get("created_at").and_then(Value::as_str).unwrap_or_default();
                let payload_opt = change.get("payload").and_then(Value::as_object);
                if entity.is_empty() || record_id == 0 || payload_opt.is_none() {
                    last_seq = seq;
                    continue;
                }
                let payload = payload_opt.unwrap();
                match db(state).apply_remote_change(entity, record_id, payload, change_id, created_at) {
                    Ok(status) => {
                        if status == "applied" || status == "tombstone" {
                            downloaded += 1;
                            // Cloche : changement effectué par un AUTRE appareil.
                            // Native thread -> l'écoute continue même quand aucun
                            // écran n'est affiché / app en arrière-plan.
                            db(state).notify_remote_change(
                                entity,
                                if status == "tombstone" { "DELETE" } else { "UPDATE" },
                                record_id,
                            );
                        } else if status == "conflict" {
                            eprintln!("[sync] conflit {} #{}: local gagne", entity, record_id);
                        }
                    }
                    Err(e) => eprintln!("[sync] apply {} #{} failed: {}", entity, record_id, e.message),
                }
                last_seq = seq;
            }
            db(state).sync_cursor_set(last_seq).ok();
            cursor = last_seq;
            // Progression RÉELLE du delta (mission §7-8) : current = changements
            // effectivement traités, total = count initial du journal cloud.
            if delta_total > 0 {
                let dl = crate::supabase::SyncProgress::new("delta", "Réception des nouveaux changements", "downloading");
            crate::supabase::emit_progress(app, &dl.with_counts(downloaded as u64, delta_total, downloaded as u64), None);
            }
            if batch_len < 500 {
                break;
            }
        }
    }

    // 4) Registre appareil (pour le nettoyage du journal §12) + nettoyages.
    let device = db(state).device_id().unwrap_or_default();
    let cur = db(state).sync_cursor_get().unwrap_or(0);
    let _ = crate::supabase::peer_register(&pool, &device, cur, uploaded as i64).await;
    db(state).outbox_cleanup().ok();
    let _ = crate::supabase::sync_changes_cleanup(&pool).await;

    // 5) Statut final.
    let pending = db(state).outbox_pending_count().unwrap_or(0);
    let msg = if pending > 0 {
        format!("Sync terminée: {} reçues, {} envoyées ({} restent en attente)", downloaded, uploaded, pending)
    } else {
        format!("Sync terminée: {} reçues, {} envoyées", downloaded, uploaded)
    };
    set_sync_state(state, json!({
        "running": false,
        "step": "terminé",
        "message": msg,
        "pulled_total": downloaded,
        "pushed_total": uploaded,
        "cursor": cur,
        "pending": pending,
        "timestamp": now_iso()
    }));
    // Événement final (delta sync discret : le dashboard affiche "Synchronisation..."
    // pendant running=true, puis un toast peut réagir à sync-completed).
    if !was_initial_sync {
        let fin = crate::supabase::SyncProgress::new("terminé", &msg, "completed");
        crate::supabase::emit_progress(app, &fin.with_counts(downloaded as u64, (downloaded + uploaded).max(1) as u64, (downloaded + uploaded) as u64), Some("sync-completed"));
    }
    Ok(json!({
        "success": true,
        "step": "terminé",
        "message": msg,
        "pulled_total": downloaded,
        "pushed_total": uploaded,
        "cursor": cur,
        "pending": pending,
        "timestamp": now_iso()
    }))
}

/// GET /api/sync/poll - Best practice : retourne l'état de la sync en arrière-plan
/// (running, step, résultat final) + timestamp, utilisé par frontend pour refresh.
#[tauri::command]
pub fn sync_poll(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    claims(&state, &token)?;
    #[cfg(feature = "supabase-sync")]
    let supabase_enabled = state.supabase_pool.lock().ok().and_then(|g| g.clone()).is_some();
    #[cfg(not(feature = "supabase-sync"))]
    let supabase_enabled = false;
    let last_sync = state.sync_state.lock().ok().and_then(|g| g.clone()).unwrap_or(Value::Null);
    Ok(json!({
        "changes": {},
        "timestamp": now_iso(),
        "supabase": {
            "enabled": supabase_enabled
        },
        "last_sync": last_sync
    }))
}