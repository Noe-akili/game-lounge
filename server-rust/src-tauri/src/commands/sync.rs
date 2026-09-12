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
        "message": if neon_enabled && !neon_available { "Neon configuré (fallback) mais offline - données locales" } else { "" }
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
            "message": "Synchronisation locale terminée (Neon non configuré)",
            "timestamp": now_iso()
        }));
    }
}

/// Travail réel de la sync (pull + push), exécuté en arrière-plan.
/// Pub : utilisé aussi par la sync automatique (boucle du setup).
#[cfg(feature = "neon-sync")]
pub async fn run_sync_impl(app: &tauri::AppHandle, state: &State<'_, AppState>) -> ApiResult<Value> {
    let pool_opt = {
        let guard = state.neon_pool.lock().map_err(|_| crate::error::ApiError::internal("neon lock"))?;
        guard.clone()
    };
    let Some(pool) = pool_opt else {
        eprintln!("[sync] Neon pool non disponible (offline) - données locales conservées");
        return Ok(json!({
            "success": true,
            "message": "Mode offline: données locales utilisées (Neon sera synchronisé à la reconnexion)",
            "timestamp": now_iso()
        }));
    };

    // 0) Migration du schéma cloud (tables si base neuve, idempotent) puis
    //    colonne deleted (soft-delete) + index unique id (anti-doublons)
    match crate::neon::ensure_cloud_schema(&pool).await {
        Ok(_) => {}
        Err(e) => eprintln!("[sync] migration schéma cloud failed: {}", e),
    }
    crate::neon::ensure_deleted_columns(&pool).await;

    // 1) PULL Neon -> local (toutes tables, colonnes filtrées sur le schéma SQLite local)
    set_sync_state(state, json!({ "running": true, "step": "pull", "message": "Réception des données Neon..." }));
    match crate::neon::pull_all(&pool).await {
        Ok(all) => {
            let db = db(state);
            let mut total_pulled = 0;
            let mut pulled_map = serde_json::Map::new();
            for (table, rows) in &all {
                let local_cols = db.columns(table).unwrap_or_default();
                let mut pulled = 0;
                for row in rows {
                    let Some(obj) = row.as_object() else { continue };
                    // ANTI-DOUBLON : sans id entier pas d'upsert possible -> chaque sync
                    // créerait une nouvelle ligne. On ignore et on log.
                    let id_opt = obj.get("id").and_then(Value::as_i64);
                    if id_opt.is_none() || id_opt.unwrap_or(0) == 0 {
                        eprintln!("[sync] pull {}: ligne sans id entier, ignorée (anti-doublon)", table);
                        continue;
                    }
                    let id = id_opt.unwrap_or(0);
                    // Filtre sur les colonnes locales : une colonne Neon inconnue
                    // ne fait plus échouer l'insert (avant : "no such column" avalé)
                    let mut map = serde_json::Map::new();
                    let mut upd = serde_json::Map::new();
                    for (k, v) in obj {
                        if !local_cols.contains(k) { continue; }
                        // Le hash local est LA référence pour le login : on ne
                        // l'écrase jamais avec le hash Neon (algos possiblement
                        // incompatibles -> "Identifiants incorrects" après sync)
                        if table == "users" && k == "password_hash" { continue; }
                        map.insert(k.clone(), v.clone());
                        if *k != "id" { upd.insert(k.clone(), v.clone()); }
                    }
                    if map.is_empty() { continue; }
                    match db.get_opt(table, id) {
                        Ok(Some(local)) => {
                            // Soft-deleted localement : on ne ressuscite JAMAIS la ligne
                            if local.get("deleted").and_then(Value::as_i64).unwrap_or(0) == 1 {
                                eprintln!("[sync] pull {} #{}: supprimée localement, ignorée", table, id);
                                continue;
                            }
                            match db.update(table, id, &upd) {
                                Ok(_) => pulled += 1,
                                Err(e) => eprintln!("[sync] update {} #{} failed: {}", table, id, e.message),
                            }
                        }
                        Ok(None) => match db.insert(table, &map) {
                            Ok(_) => pulled += 1,
                            Err(e) => eprintln!("[sync] insert {} failed: {}", table, e.message),
                        },
                        Err(e) => eprintln!("[sync] get {} #{} failed: {}", table, id, e.message),
                    }
                }
                eprintln!("[sync] pull {}: {} rows écrites", table, pulled);
                pulled_map.insert(table.to_string(), json!(pulled));
                total_pulled += pulled;
            }
            set_sync_state(state, json!({
                "running": true,
                "step": "push",
                "message": format!("{} lignes reçues, envoi des données locales...", total_pulled)
            }));

            // 2) PUSH local -> Neon : toutes les lignes, Y COMPRIS les soft-deleted
            //    (query_all_all) pour propager les suppressions vers Neon.
            let mut total_pushed = 0;
            let mut pushed_map = serde_json::Map::new();
            for table in crate::db::TABLES {
                let rows = db.query_all_all(table).unwrap_or_default();
                if rows.is_empty() { continue; }
                match crate::neon::push_table(&pool, table, &rows).await {
                    Ok(n) => {
                        eprintln!("[sync] push {}: {} rows", table, n);
                        pushed_map.insert(table.to_string(), json!(n));
                        total_pushed += n;
                    }
                    Err(e) => {
                        eprintln!("[sync] push {} failed: {}", table, e.message);
                        crate::logger::log_neon(&format!("sync_run: push {} failed: {}", table, e.message));
                    }
                }
            }
            return Ok(json!({
                "success": true,
                "step": "terminé",
                "message": format!("Sync terminée: {} reçues, {} envoyées", total_pulled, total_pushed),
                "pulled": Value::Object(pulled_map),
                "pushed": Value::Object(pushed_map),
                "pulled_total": total_pulled,
                "pushed_total": total_pushed,
                "timestamp": now_iso()
            }));
        }
        Err(e) => {
            eprintln!("[sync] pull_all failed: {}", e.message);
            crate::logger::log_neon(&format!("sync_run: pull_all failed: {} -> reconnexion", e.message));
            crate::neon::schedule_reconnect(app);
            return Ok(json!({
                "success": false,
                "step": "erreur",
                "message": format!("Neon sync échouée (connexion perdue): {}. Reconnexion en arrière-plan, réessayez.", e.message),
                "timestamp": now_iso()
            }));
        }
    }
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