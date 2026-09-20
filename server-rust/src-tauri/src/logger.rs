// Logger Rust - capture tout dans 1.log (demandé utilisateur)
// Rien ne doit nous échapper, logs visibles dans l'app via get_logs
use std::fs::{OpenOptions, File};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static LOG_BUF: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn log_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    // 1.log à plusieurs endroits pour être sûr de le trouver
    paths.push(std::path::PathBuf::from("1.log"));
    paths.push(std::path::PathBuf::from("/tmp/1.log"));
    if let Ok(home) = std::env::var("HOME") {
        paths.push(std::path::PathBuf::from(home).join("1.log"));
    }
    // Chemins Android : stockage INTERNE de l'app uniquement (pas de /sdcard :
    // STORAGE permission inutile, et le dossier "développement" n'existe plus)
    for p in &[
        "/data/data/com.gamelounge.android/files/1.log",
    ] {
        paths.push(std::path::PathBuf::from(p));
    }
    // Tauri app_data_dir si dispo (sera ajouté via AppHandle)
    paths
}

fn init_file() -> Option<File> {
    for path in log_paths() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(f) = OpenOptions::new().create(true).append(true).open(&path) {
            // Teste écriture
            if let Ok(mut file) = OpenOptions::new().write(true).open(&path) {
                let _ = writeln!(file, "[logger] init log file {:?}", path);
            }
            eprintln!("[logger] log file: {:?}", path);
            return Some(f);
        }
    }
    // Fallback /tmp
    OpenOptions::new().create(true).append(true).open("/tmp/1.log").ok()
}

pub fn init() {
    let file = init_file();
    if let Some(f) = file {
        let _ = LOG_FILE.set(Mutex::new(f));
    }
    let _ = LOG_BUF.set(Mutex::new(Vec::new()));
    // Hook pour capturer eprintln! via log crate ? On utilise un simple wrapper
    eprintln!("[logger] Rust logger initialisé - tout sera dans 1.log");
    log("BOOT", "logger initialisé");
}

pub fn log(target: &str, msg: &str) {
    // Heure LOCALE dans les logs : c'est l'heure que lit l'utilisateur sur le
    // téléphone. Avant (UTC), une session démarrée à 18:51 locales loggait
    // "16:51" -> l'historique/logs semblaient afficher une autre heure.
    let line = format!("[{}] {} - {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), target, msg);
    // Mémoire
    if let Some(buf) = LOG_BUF.get() {
        if let Ok(mut guard) = buf.lock() {
            guard.push(line.clone());
            if guard.len() > 2000 {
                guard.remove(0);
            }
        }
    }
    // Fichier
    if let Some(file) = LOG_FILE.get() {
        if let Ok(mut guard) = file.lock() {
            let _ = writeln!(guard, "{}", line);
            let _ = guard.flush();
        }
    }
    // Aussi eprintln pour logcat
    eprintln!("{}", line);
}

pub fn log_cloud(msg: &str) { log("cloud", msg); }
pub fn log_auth(msg: &str) { log("auth", msg); }
pub fn log_sync(msg: &str) { log("sync", msg); }

pub fn get_logs(limit: usize) -> Vec<String> {
    if let Some(buf) = LOG_BUF.get() {
        if let Ok(guard) = buf.lock() {
            let len = guard.len();
            let start = if len > limit { len - limit } else { 0 };
            return guard[start..].to_vec();
        }
    }
    Vec::new()
}
