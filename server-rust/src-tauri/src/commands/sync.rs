// /api/sync - statut et synchronisation Supabase (offline-first best practice)
use serde_json::{Value, json};
use tauri::Manager;
use tauri::State;

use crate::commands::{admin_only, claims, db};
use crate::db::now_iso;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

fn set_sync_state<'a>(state: &'a State<'_, AppState>, v: Value) {
    if let Ok(mut guard) = state.sync_state.lock() {
        *guard = Some(v);
    }
}

fn local_status(state: &State<'_, AppState>) -> ApiResult<Value> {
    // Données d'exploitation présentes ? (la table `users` n'est plus un
    // indicateur : les comptes ne sont plus stockés localement)
    let has_local = !db(state).query_all("consoles")?.is_empty();
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
        let _ = db(&state).query_all("consoles")?;
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
/// Écrit les lignes reçues du cloud EN UN SEUL LOT (une transaction, un verrou).
///
/// Avant : une transaction et un verrouillage de la base PAR LIGNE, ce qui rendait
/// la synchronisation très lente sur téléphone et bloquait l'interface par
/// à-coups. Le découpage en tranches laisse respirer les autres opérations
/// Schéma cloud déjà vérifié pendant ce lancement (voir run_sync_impl §0).
#[cfg(feature = "supabase-sync")]
static SCHEMA_VERIFIE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Horodatage (ms) du dernier entretien cloud effectué.
#[cfg(feature = "supabase-sync")]
static DERNIER_ENTRETIEN: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Vrai si le cycle courant doit faire l'entretien (au plus toutes les 5 min).
#[cfg(feature = "supabase-sync")]
fn entretien_du_cycle() -> bool {
    const PERIODE_MS: i64 = 5 * 60 * 1000;
    let maintenant = chrono::Utc::now().timestamp_millis();
    let dernier = DERNIER_ENTRETIEN.load(std::sync::atomic::Ordering::Relaxed);
    if maintenant - dernier < PERIODE_MS {
        return false;
    }
    DERNIER_ENTRETIEN.store(maintenant, std::sync::atomic::Ordering::Relaxed);
    true
}

/// (démarrage de session, login) entre deux tranches.
fn apply_rows(db: &crate::db::Db, table: &str, rows: &[Value]) -> usize {
    const TRANCHE: usize = 200;
    let mut applied = 0;
    for morceau in rows.chunks(TRANCHE) {
        applied += db.apply_remote_rows(table, morceau);
    }
    if applied > 0 {
        eprintln!("[sync] pull {}: {} lignes écrites", table, applied);
    }
    applied
}

/// Tables de la sync initiale dans l'ordre (données de référence d'abord, puis
/// données d'exploitation). Dérivé de crate::db::TABLES — aucune table inventée.
///
/// COMPTES 100 % EN LIGNE : `users` NE FAIT PLUS PARTIE de la synchronisation.
/// Les comptes ne sont ni téléchargés, ni envoyés, ni stockés localement : ils
/// sont lus et écrits directement dans le cloud (voir commands/users.rs).
#[cfg(feature = "supabase-sync")]
const SYNC_PHASES: [&str; 10] = [
    "consoles", "jeux", "joueurs", "tarifs", "parametres_fidelite",
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

/// POST /api/sync/initial-skip : BYPASS de l'écran d'initialisation. Marque la
/// première sync comme faite SANS restaurer le cloud — utilisé quand l'appareil
/// est hors ligne / cloud injoignable et que l'utilisateur choisit de travailler
/// avec les données locales au lieu de rester bloqué sur /initialisation.
/// Les données cloud manquantes seront récupérées au retour du réseau : le
/// curseur reste à 0, donc la prochaine sync repasse en delta/snapshot (pull du
/// journal complet, ou pull_all si trou de séquence) — rien n'est perdu.
#[tauri::command]
pub fn sync_initial_skip(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    let _ = db(&state).set_setting("initial_sync_completed", "1");
    crate::logger::log_sync(&format!("initial sync: SKIPPÉE par {} (mode local, reprise delta au retour du réseau)", user.email));
    eprintln!("[sync] initial sync skipped by {}", user.email);
    Ok(json!({
        "success": true,
        "initialSyncCompleted": true,
        "message": "Initialisation ignorée — données locales utilisées",
        "timestamp": now_iso()
    }))
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

    // 0) Migrations du schéma cloud (idempotentes) — UNE SEULE FOIS par lancement.
    //    Avant, ces deux requêtes DDL partaient à CHAQUE cycle de synchronisation,
    //    soit deux allers-retours réseau inutiles ajoutés à chaque écriture locale.
    //    Sur une connexion mobile, c'est plusieurs secondes perdues par cycle.
    if !SCHEMA_VERIFIE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        match crate::supabase::ensure_cloud_schema(&pool).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[sync] migration schéma cloud failed: {}", e);
                // Échec : on réessaiera au prochain cycle.
                SCHEMA_VERIFIE.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
        match crate::supabase::ensure_sync_schema(&pool).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[sync] migration delta sync failed: {}", e);
                SCHEMA_VERIFIE.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
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
        let mut batch_count = 0;
        loop {
            batch_count += 1;
            if batch_count > 10 {
                eprintln!("[sync] limite de lots delta atteinte (10 lots), fin du cycle");
                break;
            }
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
            // Le lot est préparé puis écrit EN UNE TRANSACTION (voir
            // Db::apply_remote_changes) : avant, chaque changement reverrouillait
            // la base et validait sur le disque, d'où une synchronisation très
            // lente et des à-coups dans l'interface.
            let mut lot: Vec<(&str, i64, &serde_json::Map<String, Value>, &str, &str)> = Vec::new();
            let mut metas: Vec<(&str, i64, &str, &str)> = Vec::new();
            for change in &batch {
                let seq = change.get("sequence").and_then(Value::as_i64).unwrap_or(0);
                let entity = change.get("entity").and_then(Value::as_str).unwrap_or_default();
                let record_id = change.get("record_id").and_then(Value::as_i64).unwrap_or(0);
                let change_id = change.get("change_id").and_then(Value::as_str).unwrap_or_default();
                let origin_device = change.get("device_id").and_then(Value::as_str).unwrap_or_default();
                let created_at = change.get("created_at").and_then(Value::as_str).unwrap_or_default();
                let payload_opt = change.get("payload").and_then(Value::as_object);
                last_seq = seq.max(last_seq);
                if entity.is_empty() || record_id == 0 || payload_opt.is_none() {
                    continue;
                }
                lot.push((entity, record_id, payload_opt.unwrap(), change_id, created_at));
                metas.push((entity, record_id, change_id, origin_device));
            }
            let statuts = db(state).apply_remote_changes(&lot);
            // Notifications APRÈS écriture confirmée : la cloche ne sonne jamais
            // pour un changement qui n'a pas été enregistré.
            for (statut, (entity, record_id, change_id, origin_device)) in statuts.iter().zip(metas.iter()) {
                match *statut {
                    "applied" | "tombstone" => {
                        downloaded += 1;
                        db(state).notify_remote_change(
                            entity,
                            if *statut == "tombstone" { "DELETE" } else { "UPDATE" },
                            *record_id,
                            change_id,
                            origin_device,
                        );
                    }
                    "conflict" => eprintln!("[sync] conflit {} #{}: local gagne", entity, record_id),
                    _ => {}
                }
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

    // 4) Registre appareil + nettoyages : AU PLUS une fois toutes les 5 minutes.
    //    Ce sont deux allers-retours réseau d'entretien, sans effet sur les données
    //    affichées : les exécuter à chaque écriture locale ralentissait chaque
    //    synchronisation sans rien apporter.
    // `cur` est lu DANS TOUS LES CAS : il est renvoyé dans le statut final plus bas.
    let cur = db(state).sync_cursor_get().unwrap_or(0);
    if entretien_du_cycle() {
        let device = db(state).device_id().unwrap_or_default();
        let _ = crate::supabase::peer_register(&pool, &device, cur, uploaded as i64).await;
        db(state).outbox_cleanup().ok();
        let _ = crate::supabase::sync_changes_cleanup(&pool).await;
    }

    // 4 bis) COMPTES 100 % EN LIGNE : la connexion cloud est déjà ouverte, on
    //        profite du cycle pour vérifier que le compte connecté n'a pas été
    //        supprimé ailleurs. Si c'est le cas, les données de l'appareil sont
    //        effacées et le frontend repart sur l'écran de connexion.
    let _ = crate::account_watcher::verifier(app, state).await;

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

/// GET /api/sync/outbox/stats - Statistiques de la file d'attente locale
#[tauri::command]
pub fn sync_outbox_stats(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let conn = db(&state).conn();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0)).unwrap_or(0);
    let pending: i64 = conn.query_row("SELECT COUNT(*) FROM sync_outbox WHERE status = 'PENDING'", [], |r| r.get(0)).unwrap_or(0);
    let acked: i64 = conn.query_row("SELECT COUNT(*) FROM sync_outbox WHERE status = 'ACKED'", [], |r| r.get(0)).unwrap_or(0);
    let failed: i64 = conn.query_row("SELECT COUNT(*) FROM sync_outbox WHERE status = 'FAILED'", [], |r| r.get(0)).unwrap_or(0);
    let conflicts: i64 = conn.query_row("SELECT COUNT(*) FROM sync_conflicts", [], |r| r.get(0)).unwrap_or(0);
    let cursor = db(&state).sync_cursor_get().unwrap_or(0);

    Ok(json!({
        "total": total,
        "pending": pending,
        "acked": acked,
        "failed": failed,
        "conflicts": conflicts,
        "cursor": cursor,
    }))
}

/// POST /api/sync/outbox/purge - Nettoyer la file d'attente outbox
#[tauri::command]
pub fn sync_purge_outbox(
    state: State<'_, AppState>,
    token: Option<String>,
    mode: Option<String>,
) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let conn = db(&state).conn();
    let m = mode.as_deref().unwrap_or("acked");
    let deleted_count = match m {
        "all" => {
            conn.execute("DELETE FROM sync_outbox", []).map_err(|e| ApiError::internal(format!("purge all outbox: {e}")))?
        }
        "failed" => {
            conn.execute("DELETE FROM sync_outbox WHERE status = 'FAILED'", []).map_err(|e| ApiError::internal(format!("purge failed outbox: {e}")))?
        }
        _ => {
            conn.execute("DELETE FROM sync_outbox WHERE status = 'ACKED'", []).map_err(|e| ApiError::internal(format!("purge acked outbox: {e}")))?
        }
    };
    Ok(json!({ "mode": m, "deleted": deleted_count }))
}

/// POST /api/sync/conflicts/clear - Vider les conflits
#[tauri::command]
pub fn sync_clear_conflicts(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let conn = db(&state).conn();
    let deleted_count = conn.execute("DELETE FROM sync_conflicts", []).map_err(|e| ApiError::internal(format!("clear conflicts: {e}")))?;
    Ok(json!({ "deleted": deleted_count }))
}

/// POST /api/sync/cursors/reset - Réinitialiser les curseurs de synchronisation
#[tauri::command]
pub fn sync_reset_cursors(state: State<'_, AppState>, token: Option<String>) -> ApiResult<Value> {
    let user = claims(&state, &token)?;
    admin_only(&user)?;
    let conn = db(&state).conn();
    conn.execute_batch(
        "UPDATE sync_state SET device_sequence = 0, last_uploaded = NULL, last_received = '0', last_sync_at = NULL;"
    ).map_err(|e| ApiError::internal(format!("reset cursors: {e}")))?;
    Ok(json!({ "reset": true }))
}
