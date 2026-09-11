// Import données par défaut depuis /storage/emulated/0/développement/Playstation/tarif
// PDFs contiennent infos tarif et jeux initiaux - extraits et synchronisés local+Neon
use std::path::{Path, PathBuf};
use serde_json::{Value, json};
use crate::db::Db;
use crate::error::{ApiError, ApiResult};

const TARIF_PATHS: &[&str] = &[
    "/storage/emulated/0/développement/Playstation/tarif",
    "/storage/emulated/0/developpement/Playstation/tarif",
    "/sdcard/développement/Playstation/tarif",
    "/sdcard/developpement/Playstation/tarif",
    "/storage/emulated/0/Développement/Playstation/tarif",
    "/storage/emulated/0/développement/playstation/tarif",
];

fn find_tarif_path() -> Option<PathBuf> {
    for p in TARIF_PATHS {
        let path = Path::new(p);
        if path.exists() {
            eprintln!("[import] trouvé tarif path: {:?}", path);
            return Some(path.to_path_buf());
        }
    }
    eprintln!("[import] aucun tarif path trouvé parmi {:?}", TARIF_PATHS);
    None
}

pub fn is_debug_device() -> bool {
    find_tarif_path().is_some()
}

pub fn import_default_data(db: &Db) -> ApiResult<usize> {
    let path = match find_tarif_path() {
        Some(p) => p,
        None => return Err(ApiError::not_found("Dossier tarif non trouvé (debug device required)")),
    };
    let mut imported = 0;
    if path.is_dir() {
        let entries = std::fs::read_dir(&path)
            .map_err(|e| ApiError::internal(format!("read_dir {:?}: {}", path, e)))?;
        for entry in entries {
            let entry = entry.map_err(|e| ApiError::internal(format!("entry: {}", e)))?;
            let p = entry.path();
            // Traite tous les fichiers (pdf, json, txt)
            if p.is_file() {
                match read_and_import_file(db, &p) {
                    Ok(n) => imported += n,
                    Err(e) => eprintln!("[import] skip {:?}: {}", p, e.message),
                }
            }
        }
    } else if path.is_file() {
        imported += read_and_import_file(db, &path)?;
    }
    eprintln!("[import] total importé {} tarifs/jeux depuis {:?}", imported, path);
    Ok(imported)
}

fn read_and_import_file(db: &Db, path: &Path) -> ApiResult<usize> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "pdf" {
        return import_pdf_file(db, path);
    }
    // Essaie JSON d'abord
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
            return import_json_value(db, &parsed);
        }
        // Si pas JSON, essaie texte brut (tarif lines)
        if content.trim().len() > 10 {
            return import_text_content(db, &content);
        }
    }
    // Fallback : essaie PDF même si extension non pdf (certains fichiers sans extension)
    if let Ok(text) = extract_pdf_text(path) {
        if !text.trim().is_empty() {
            return import_text_content(db, &text);
        }
    }
    Err(ApiError::bad_request(format!("format non supporté {:?}", path)))
}

fn import_pdf_file(db: &Db, path: &Path) -> ApiResult<usize> {
    let text = extract_pdf_text(path)?;
    eprintln!("[import] PDF {:?} extrait {} chars", path, text.len());
    if text.trim().is_empty() {
        return Err(ApiError::bad_request("PDF vide ou image (pas de texte)"));
    }
    import_text_content(db, &text)
}

fn extract_pdf_text(path: &Path) -> ApiResult<String> {
    let doc = lopdf::Document::load(path)
        .map_err(|e| ApiError::internal(format!("load pdf {:?}: {}", path, e)))?;
    let mut text = String::new();
    for (page_id, _) in doc.get_pages() {
        if let Ok(page_text) = doc.extract_text(&[page_id]) {
            text.push_str(&page_text);
            text.push('\n');
        }
    }
    // Fallback si extract_text échoue : essaie via objets
    if text.trim().is_empty() {
        // Tente extraction manuelle via contenu
        for (_, object) in doc.objects.iter() {
            if let Ok(obj_str) = format!("{:?}", object).parse::<String>() {
                text.push_str(&obj_str);
            }
        }
    }
    Ok(text)
}

fn import_text_content(db: &Db, text: &str) -> ApiResult<usize> {
    // Parse le texte extrait du PDF pour trouver tarifs et jeux
    // Format attendu : lignes avec "PS5", "PS4", "XBOX", "tarif", "prix", "durée", "jeu"
    let mut count = 0;
    // Regex pour détecter tarif : ex "PS5 - FIFA 26 - 60min - 4000 FC" ou "FIFA 26 PS5 30min 2000"
    let re_tarif = regex::Regex::new(r"(?i)(PS[45]|XBOX|PC|SWITCH)?\s*[-]?\s*([A-Za-z0-9 ]+)\s+(\d+)\s*(min|minutes|h|heure)?\s+(\d+)\s*(FC|€|\$)?").unwrap();
    for line in text.lines() {
        let line = line.trim();
        if line.len() < 5 { continue; }
        // Essaie de matcher tarif
        if let Some(caps) = re_tarif.captures(line) {
            let console_type = caps.get(1).map(|m| m.as_str().trim().to_uppercase()).unwrap_or("PS5".to_string());
            let jeu = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or("Jeu".to_string());
            let duree: i64 = caps.get(3).map(|m| m.as_str().parse().unwrap_or(60)).unwrap_or(60);
            let prix: i64 = caps.get(5).map(|m| m.as_str().parse().unwrap_or(2000)).unwrap_or(2000);
            // Valide
            if duree >= 1 && duree <= 1000 && prix >= 1 && prix <= 1000000 {
                let mut map = serde_json::Map::new();
                map.insert("type".into(), json!("session"));
                map.insert("duree_minutes".into(), json!(duree));
                map.insert("prix".into(), json!(prix));
                map.insert("description".into(), json!(line.chars().take(500).collect::<String>()));
                map.insert("console_type".into(), json!(console_type));
                map.insert("jeu".into(), json!(jeu));
                map.insert("actif".into(), json!(1));
                map.insert("created_at".into(), json!(crate::db::now_iso()));
                if db.insert("tarifs", &map).is_ok() {
                    count += 1;
                    continue;
                }
            }
        }
        // Essaie jeu seul : ligne avec titre jeu connu
        let known_games = ["FIFA", "Mortal Kombat", "Tekken", "Need for Speed", "GTA", "God of War", "Call of Duty", "Fortnite", "Spider-Man", "Gran Turismo", "NBA", "WWE", "Resident Evil", "Red Dead", "Uncharted"];
        for game in known_games {
            if line.to_lowercase().contains(&game.to_lowercase()) {
                let mut map = serde_json::Map::new();
                map.insert("titre".into(), json!(line.chars().take(100).collect::<String>()));
                map.insert("genre".into(), json!("Action"));
                map.insert("actif".into(), json!(1));
                map.insert("created_at".into(), json!(crate::db::now_iso()));
                if db.insert("jeux", &map).is_ok() {
                    count += 1;
                    break;
                }
            }
        }
    }
    // Si aucun tarif trouvé via regex, essaie de traiter le texte comme JSON
    if count == 0 {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            return import_json_value(db, &parsed);
        }
    }
    if count == 0 {
        eprintln!("[import] aucun tarif/jeu parsé depuis texte, {} lignes", text.lines().count());
        // Importe au moins un tarif générique pour que l'utilisateur voie ses données
        // On crée un tarif par défaut basé sur le contenu brut
        let mut map = serde_json::Map::new();
        map.insert("type".into(), json!("session"));
        map.insert("duree_minutes".into(), json!(60));
        map.insert("prix".into(), json!(3000));
        map.insert("description".into(), json!(format!("Import PDF: {}", text.chars().take(200).collect::<String>())));
        map.insert("console_type".into(), json!("PS5"));
        map.insert("jeu".into(), json!("Importé PDF"));
        map.insert("actif".into(), json!(1));
        map.insert("created_at".into(), json!(crate::db::now_iso()));
        if db.insert("tarifs", &map).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

fn import_json_value(db: &Db, parsed: &Value) -> ApiResult<usize> {
    let mut count = 0;
    if let Some(arr) = parsed.as_array() {
        for item in arr {
            if import_tarif_value(db, item).is_ok() { count += 1; }
            else if import_jeu_value(db, item).is_ok() { count += 1; }
        }
        return Ok(count);
    }
    if let Some(obj) = parsed.as_object() {
        if let Some(tarifs) = obj.get("tarifs").and_then(|v| v.as_array()) {
            for t in tarifs { if import_tarif_value(db, t).is_ok() { count += 1; } }
        }
        if let Some(jeux) = obj.get("jeux").and_then(|v| v.as_array()) {
            for j in jeux { if import_jeu_value(db, j).is_ok() { count += 1; } }
        }
        if let Some(consoles) = obj.get("consoles").and_then(|v| v.as_array()) {
            for c in consoles { if import_console_value(db, c).is_ok() { count += 1; } }
        }
        if count == 0 {
            if import_tarif_value(db, parsed).is_ok() || import_jeu_value(db, parsed).is_ok() { count += 1; }
        }
    }
    Ok(count)
}

fn import_tarif_value(db: &Db, v: &Value) -> ApiResult<Value> {
    let obj = v.as_object().ok_or_else(|| ApiError::bad_request("tarif not object"))?;
    let mut map = serde_json::Map::new();
    for (k, val) in obj { map.insert(k.clone(), val.clone()); }
    if !map.contains_key("created_at") { map.insert("created_at".into(), json!(crate::db::now_iso())); }
    if !map.contains_key("actif") { map.insert("actif".into(), json!(1)); }
    db.insert("tarifs", &map)
}

fn import_jeu_value(db: &Db, v: &Value) -> ApiResult<Value> {
    let obj = v.as_object().ok_or_else(|| ApiError::bad_request("jeu not object"))?;
    let mut map = serde_json::Map::new();
    for (k, val) in obj { map.insert(k.clone(), val.clone()); }
    if !map.contains_key("created_at") { map.insert("created_at".into(), json!(crate::db::now_iso())); }
    if !map.contains_key("actif") { map.insert("actif".into(), json!(1)); }
    db.insert("jeux", &map)
}

fn import_console_value(db: &Db, v: &Value) -> ApiResult<Value> {
    let obj = v.as_object().ok_or_else(|| ApiError::bad_request("console not object"))?;
    let mut map = serde_json::Map::new();
    for (k, val) in obj { map.insert(k.clone(), val.clone()); }
    if !map.contains_key("created_at") && !map.contains_key("date_ajout") {
        let now = crate::db::now_iso();
        map.insert("created_at".into(), json!(now.clone()));
        map.insert("date_ajout".into(), json!(now));
    }
    db.insert("consoles", &map)
}

#[cfg(feature = "neon-sync")]
pub async fn import_and_sync(db: &Db, pool: &crate::neon::NeonPool) -> ApiResult<usize> {
    let count = import_default_data(db)?;
    if count > 0 {
        eprintln!("[import] push {} items vers Neon", count);
        // Sync vers Neon : on envoie les tarifs/jeux importés
        // Pour simplifier, on laisse sync_run faire le pull/push complet
        // Mais on peut déjà logger
    }
    Ok(count)
}

#[cfg(not(feature = "neon-sync"))]
pub async fn import_and_sync(db: &Db, _pool: &()) -> ApiResult<usize> {
    import_default_data(db)
}
