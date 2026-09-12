// /api/sync - statut et synchronisation Neon (offline-first best practice)
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
    #[cfg(feature = "neon-sync")]
    let (neon_enabled, neon_available) = {
        let pool_opt = state.neon_pool.lock().ok().and_then(|g| g.clone());
        // Fallback URL hardcodé assure Neon toujours enabled (même sans .env)
        let enabled = true;
        let available = pool_opt.is_some();
        let _ = std::env::var("DATABASE_URL").is_ok(); // garde compat
        (enabled, available)
    };
    #[cfg(not(feature = "neon-sync"))]
    let (neon_enabled, neon_available) = (false, false);

    let sync_state = state.sync_state.lock().ok().and_then(|g| g.clone());
    let syncing = sync_state.clone().and_then(|v| v.get("running").and_then(Value::as_bool)).unwrap_or(false);
    let last_sync_val: Value = sync_state
        .map(|v| v.get("finished_at").cloned().unwrap_or(Value::Null))
        .unwrap_or(Value::Null);

    Ok(json!({
        "neonEnabled": neon_enabled,
        "neonAvailable": neon_available,
        "neonConnected": neon_available,
        "enabled": sync_enabled,
        "syncing": syncing,
        "hasLocalData": has_local,
        "lastSync": last_sync_val,
        "mode": if neon_available { "cloud" } else { "offline" },
        "message": if neon_enabled && !neon_available { "Supabase configuré (fallback) mais offline - données locales" } else { "" }
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

/// POST /api/sync/run - Best practice : pull Neon -> local, puis push local -> Neon.
/// IMPORTANT : la sync complète (11 tables pull + 11 tables push, ~40 requêtes Neon
/// séquentielles) dépasse le timeout IPC Android WebView (~20s) si on attend la fin.
/// On lance donc le travail EN ARRIÈRE-PLAN et on retourne immédiatement ; le
/// frontend suit la progression via /sync/poll.
#[tauri::command(async)]
pub async fn sync_run(app: tauri::AppHandle, state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    eprintln!("[sync] sync_run demandé par {}", user.email);

    #[cfg(feature = "neon-sync")]
    {
        // Une seule sync à la fois (évite les écritures concurrentes SQLite/Neon)
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

    #[cfg(not(feature = "neon-sync"))]
    {
        let _ = db(&state).query_all("users")?;
        return Ok(json!({
            "success": true,
            "message": "Synchronisation locale terminée (Supabase non configuré)",
            "timestamp": now_iso()
        }));
    }
}

/// Travail réel de la sync (pull + push), exécuté en arrière-plan.
/// Pub : utilisé aussi par la sync automatique (boucle du setup).
#[cfg(feature = "neon-sync")]
/// Applique un pull complet (tables -> local) SANS journaliser dans l'outbox
/// (les données viennent du cloud, les renvoyer serait un aller-retour inutile)
/// et SANS écraser un changement local non envoyé (conflits gérés par Db).
fn apply_pull_all(db: &crate::db::Db, all: &std::collections::HashMap<String, Vec<Value>>) -> usize {
    let mut total = 0;
    for (table, rows) in all {
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
        eprintln!("[sync] pull {}: {} rows écrites", table, applied);
        total += applied;
    }
    total
}

pub async fn run_sync_impl(app: &tauri::AppHandle, state: &State<'_, AppState>) -> ApiResult<Value> {
    let pool_opt = {
        let guard = state.neon_pool.lock().map_err(|_| crate::error::ApiError::internal("neon lock"))?;
        guard.clone()
    };
    let Some(pool) = pool_opt else {
        eprintln!("[sync] Neon pool non disponible (offline) - données locales conservées");
        return Ok(json!({
            "success": true,
            "message": "Mode offline: données locales utilisées (Supabase sera synchronisé à la reconnexion)",
            "timestamp": now_iso()
        }));
    };

    // 0) Migrations du schéma cloud (idempotentes). La colonne deleted + les index
    //    uniques id ne partent qu'UNE FOIS (flag persistant).
    match crate::neon::ensure_cloud_schema(&pool).await {
        Ok(_) => {}
        Err(e) => eprintln!("[sync] migration schéma cloud failed: {}", e),
    }
    match crate::neon::ensure_sync_schema(&pool).await {
        Ok(_) => {}
        Err(e) => eprintln!("[sync] migration delta sync failed: {}", e),
    }
    let migrated = db(state).get_setting("cloud_migration_v2").ok().and_then(|o| o).unwrap_or_default();
    if migrated != "1" {
        crate::neon::ensure_deleted_columns(&pool).await;
        for table in crate::db::TABLES {
            let _ = crate::neon::ensure_table_unique_id(&pool, table).await;
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
    set_sync_state(state, json!({ "running": true, "step": "upload", "message": "Envoi des changements locaux..." }));
    let mut uploaded = 0;
    loop {
        let batch = db(state).outbox_pending(100).unwrap_or_default();
        if batch.is_empty() {
            break;
        }
        match crate::neon::push_outbox_batch(&pool, &batch).await {
            Ok((acked, failed)) => {
                db(state).outbox_mark(&acked, "ACKED").ok();
                db(state).outbox_mark(&failed, "FAILED").ok();
                uploaded += acked.len();
                eprintln!("[sync] upload lot: {} ACKed, {} FAILED", acked.len(), failed.len());
                if acked.is_empty() {
                    break; // échec réseau/applicatif : reprise au prochain cycle
                }
            }
            Err(e) => {
                eprintln!("[sync] upload failed: {} (reprise au prochain cycle)", e);
                crate::logger::log_neon(&format!("sync_run: upload failed: {}", e));
                break;
            }
        }
    }
    db(state).sync_uploaded_set(uploaded as i64).ok();

    // 3) DOWNLOAD delta : uniquement les changements après le curseur (spec §4).
    let mut cursor = db(state).sync_cursor_get().unwrap_or(0);
    let mut downloaded = 0;
    if cursor == 0 {
        // PREMIÈRE SYNC : pull complet (restauration) puis seed de l'outbox avec
        // les données locales pré-existantes (elles n'ont jamais été journalisées).
        set_sync_state(state, json!({ "running": true, "step": "pull", "message": "Première synchronisation (restauration complète)..." }));
        match crate::neon::pull_all(&pool).await {
            Ok(all) => {
                downloaded = apply_pull_all(db(state), &all);
                let seeded = db(state).outbox_seed().unwrap_or(0);
                eprintln!("[sync] première sync: {} reçues, {} événements seedés", downloaded, seeded);
                let max_seq = crate::neon::sync_max_sequence(&pool).await.unwrap_or(0);
                db(state).sync_cursor_set(max_seq).ok();
            }
            Err(e) => {
                eprintln!("[sync] pull_all failed: {}", e.message);
                crate::logger::log_neon(&format!("sync_run: pull_all failed: {} -> reconnexion", e.message));
                crate::neon::schedule_reconnect(app);
                return Ok(json!({
                    "success": false,
                    "step": "erreur",
                    "message": format!("Sync cloud échouée (connexion perdue): {}. Reconnexion en arrière-plan, réessayez.", e.message),
                    "timestamp": now_iso()
                }));
            }
        }
    } else {
        // SYNC DELTA : boucle de batches de 500 changements.
        set_sync_state(state, json!({ "running": true, "step": "pull", "message": "Réception des nouveaux changements..." }));
        loop {
            let batch = crate::neon::pull_delta(&pool, cursor, 500).await.unwrap_or_default();
            if batch.is_empty() {
                break;
            }
            let first_seq = batch[0].get("sequence").and_then(Value::as_i64).unwrap_or(0);
            // SNAPSHOT_REQUIRED (spec §11) : trou de séquence -> le journal a été
            // nettoyé ou l'appareil est resté trop longtemps hors ligne -> on
            // restaure un snapshot complet puis on reprend en delta.
            if first_seq > cursor + 1 {
                eprintln!("[sync] trou de séquence (curseur {}, reçu {}) -> SNAPSHOT_REQUIRED", cursor, first_seq);
                match crate::neon::pull_all(&pool).await {
                    Ok(all) => {
                        downloaded += apply_pull_all(db(state), &all);
                        let max_seq = crate::neon::sync_max_sequence(&pool).await.unwrap_or(0);
                        db(state).sync_cursor_set(max_seq).ok();
                        eprintln!("[sync] snapshot appliqué, curseur remis à {}", max_seq);
                    }
                    Err(e) => {
                        eprintln!("[sync] snapshot pull failed: {}", e.message);
                        crate::neon::schedule_reconnect(app);
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
            if batch_len < 500 {
                break;
            }
        }
    }

    // 4) Registre appareil (pour le nettoyage du journal §12) + nettoyages.
    let device = db(state).device_id().unwrap_or_default();
    let cur = db(state).sync_cursor_get().unwrap_or(0);
    let _ = crate::neon::peer_register(&pool, &device, cur, uploaded as i64).await;
    db(state).outbox_cleanup().ok();
    let _ = crate::neon::sync_changes_cleanup(&pool).await;

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
    #[cfg(feature = "neon-sync")]
    let neon_enabled = state.neon_pool.lock().ok().and_then(|g| g.clone()).is_some();
    #[cfg(not(feature = "neon-sync"))]
    let neon_enabled = false;
    let last_sync = state.sync_state.lock().ok().and_then(|g| g.clone()).unwrap_or(Value::Null);
    Ok(json!({
        "changes": {},
        "timestamp": now_iso(),
        "neon": {
            "enabled": neon_enabled
        },
        "last_sync": last_sync
    }))
}