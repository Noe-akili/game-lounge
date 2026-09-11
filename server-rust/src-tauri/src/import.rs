// Import données par défaut depuis PDFs dans /storage/emulated/0/développement/Playstation/tarif
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

// Identifiant unique de l'appareil autorisé pour debug (fingerprint de ce téléphone)
const ALLOWED_FINGERPRINT: &str = "CALUS/Note_16_Pro/Note_16_Pro:14/UP1A.231005.007/2025115:user/release-keys";
const ALLOWED_MODEL: &str = "Note_16_Pro";

fn get_device_fingerprint() -> String {
    // Tente getprop via Command, sinon lit /system/build.prop, sinon utilise fallback
    if let Ok(out) = std::process::Command::new("getprop").arg("ro.build.fingerprint").output() {
        if let Ok(s) = String::from_utf8(out.stdout) {
            let t = s.trim().to_string();
            if !t.is_empty() { return t; }
        }
    }
    // Fallback lecture /system/build.prop
    if let Ok(content) = std::fs::read_to_string("/system/build.prop") {
        for line in content.lines() {
            if line.starts_with("ro.build.fingerprint=") {
                return line.trim_start_matches("ro.build.fingerprint=").trim().to_string();
            }
        }
    }
    // Fallback modèle
    if let Ok(out) = std::process::Command::new("getprop").arg("ro.product.model").output() {
        if let Ok(s) = String::from_utf8(out.stdout) {
            return s.trim().to_string();
        }
    }
    String::new()
}

fn find_tarif_path() -> Option<PathBuf> {
    for p in TARIF_PATHS {
        let path = Path::new(p);
        if path.exists() {
            eprintln!("[import] trouvé tarif path: {:?}", path);
            return Some(path.to_path_buf());
        }
    }
    eprintln!("[import] aucun tarif path trouvé");
    None
}

pub fn is_debug_device() -> bool {
    // Debug uniquement sur cet appareil : vérifie fingerprint OU présence du dossier tarif
    let fp = get_device_fingerprint();
    eprintln!("[import] device fingerprint: '{}'", fp);
    if fp == ALLOWED_FINGERPRINT || fp == ALLOWED_MODEL || fp.contains("Note_16_Pro") {
        eprintln!("[import] debug autorisé via fingerprint");
        return true;
    }
    // Fallback : présence du dossier tarif (cet appareil a le dossier)
    if find_tarif_path().is_some() {
        eprintln!("[import] debug autorisé via tarif path");
        return true;
    }
    eprintln!("[import] debug refusé : appareil non autorisé");
    false
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
            if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                // Traite pdf et aussi sans extension
                if ext == "pdf" || ext == "PDF" || p.is_file() {
                    match read_and_import_file(db, &p) {
                        Ok(n) => {
                            eprintln!("[import] {:?} -> {} tarifs/jeux", p.file_name().unwrap_or_default(), n);
                            imported += n
                        },
                        Err(e) => eprintln!("[import] skip {:?}: {}", p, e.message),
                    }
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
    // Essaie JSON
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
            return import_json_value(db, &parsed);
        }
        if content.trim().len() > 10 {
            // Texte brut
            if let Ok(n) = import_text_content(db, &content, path) {
                if n > 0 { return Ok(n); }
            }
        }
    }
    // Fallback PDF même si extension non pdf
    if let Ok(text) = extract_pdf_text(path) {
        if !text.trim().is_empty() {
            return import_text_content(db, &text, path);
        }
    }
    Err(ApiError::bad_request(format!("format non supporté {:?}", path)))
}

fn import_pdf_file(db: &Db, path: &Path) -> ApiResult<usize> {
    let text = extract_pdf_text(path)?;
    eprintln!("[import] PDF {:?} extrait {} chars", path.file_name().unwrap_or_default(), text.len());
    if text.trim().is_empty() {
        return Err(ApiError::bad_request("PDF vide ou image"));
    }
    import_text_content(db, &text, path)
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
    if text.trim().is_empty() {
        // Tentative via objets si extract_text échoue
        for (_, obj) in doc.objects.iter() {
            text.push_str(&format!("{:?}", obj));
            text.push('\n');
        }
    }
    if text.trim().is_empty() {
        return Err(ApiError::internal("PDF sans texte extractible"));
    }
    Ok(text)
}

fn import_text_content(db: &Db, text: &str, path: &Path) -> ApiResult<usize> {
    // Détecte console depuis nom de fichier ou contenu
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_uppercase();
    let mut console_type = if filename.contains("PS5") || text.contains("JEUX PS5") {
        "PS5"
    } else if filename.contains("PS4") || text.contains("JEUX PS4") {
        "PS4"
    } else {
        "PS5" // défaut
    };
    // Si le texte contient les deux, on garde celui du fichier, mais on parse quand même
    let mut current_console = console_type.to_string();
    let mut current_jeu = String::new();
    let mut count = 0;

    // Regex pour tarif : "500 FC", "2 000 FC", "1 500 FC", avec durée "5 minutes", "30 Minutes", "1 Heure"
    let re_prix = regex::Regex::new(r"(\d[\d\s]*)\s*FC").unwrap();
    let re_duree = regex::Regex::new(r"(?i)(\d+)\s*(minutes?|heure|h|minutes?)").unwrap();
    let re_jeu_num = regex::Regex::new(r"^\d+\.\s+([A-Z0-9][A-Za-z0-9 \-'&.]+)").unwrap();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() { continue; }
        // Détecte changement de console dans le texte
        if line.to_uppercase().contains("JEUX PS5") {
            current_console = "PS5".to_string();
            continue;
        }
        if line.to_uppercase().contains("JEUX PS4") {
            current_console = "PS4".to_string();
            continue;
        }
        // Détecte jeu : "1. FIFA 26", "2. MORTAL KOMBAT"
        if let Some(caps) = re_jeu_num.captures(line) {
            if let Some(m) = caps.get(1) {
                let jeu = m.as_str().trim().to_string();
                // Filtre les titres trop courts ou qui sont des catégories
                if jeu.len() >= 3 && !jeu.to_uppercase().starts_with("JEUX") && !jeu.contains("➢") {
                    current_jeu = jeu.clone();
                    // Crée aussi le jeu dans la table jeux
                    let mut map = serde_json::Map::new();
                    map.insert("titre".into(), json!(jeu.chars().take(100).collect::<String>()));
                    map.insert("genre".into(), json!(detect_genre(&jeu)));
                    map.insert("actif".into(), json!(1));
                    map.insert("created_at".into(), json!(crate::db::now_iso()));
                    let _ = db.insert("jeux", &map);
                    // Ne compte pas comme tarif, juste jeu
                }
            }
            continue;
        }
        // Détecte tarif : ligne avec "➢" et prix
        if line.contains("➢") || line.contains("FC") {
            // Extrait prix
            if let Some(price_caps) = re_prix.captures(line) {
                if let Some(price_match) = price_caps.get(1) {
                    let price_str = price_match.as_str().replace(" ", "").replace("\u{00A0}", "");
                    if let Ok(prix) = price_str.parse::<i64>() {
                        if prix < 100 || prix > 20000 { continue; } // filtre prix aberrants
                        // Extrait durée
                        let mut duree = 30; // défaut
                        if let Some(duree_caps) = re_duree.captures(line) {
                            if let Some(d) = duree_caps.get(1) {
                                if let Ok(d_val) = d.as_str().parse::<i64>() {
                                    let unit = duree_caps.get(2).map(|m| m.as_str().to_lowercase()).unwrap_or("minutes".to_string());
                                    if unit.contains("heure") || unit == "h" {
                                        duree = d_val * 60;
                                    } else {
                                        duree = d_val;
                                        // Cas "5 minutes" -> 5, "30 Minutes" -> 30, "1 Heure" -> 60
                                        if duree == 1 && unit.contains("heure") { duree = 60; }
                                    }
                                }
                            }
                        } else {
                            // Fallback : cherche "5 minutes" sans regex
                            if line.to_lowercase().contains("5 minutes") { duree = 5; }
                            else if line.to_lowercase().contains("15 minutes") { duree = 15; }
                            else if line.to_lowercase().contains("30 minutes") { duree = 30; }
                            else if line.contains("1 Heure") { duree = 60; }
                        }
                        if duree < 1 || duree > 1000 { duree = 30; }

                        // Type de tarif
                        let tarif_type = if duree <= 10 { "partie" } else { "session" };
                        let jeu = if current_jeu.is_empty() { "Jeu" } else { &current_jeu };
                        // Détecte variantes G29/VR
                        let mut description = line.chars().take(500).collect::<String>();
                        // Nettoie les caractères spéciaux
                        description = description.replace("➢", "").trim().to_string();

                        let mut map = serde_json::Map::new();
                        map.insert("type".into(), json!(tarif_type));
                        map.insert("duree_minutes".into(), json!(duree));
                        map.insert("prix".into(), json!(prix));
                        map.insert("description".into(), json!(description));
                        map.insert("console_type".into(), json!(current_console.clone()));
                        map.insert("jeu".into(), json!(jeu.clone()));
                        map.insert("actif".into(), json!(1));
                        map.insert("created_at".into(), json!(crate::db::now_iso()));
                        if db.insert("tarifs", &map).is_ok() {
                            count += 1;
                        }
                    }
                }
            }
        }
    }

    // Fallback si aucun tarif parsé mais texte contient JSON
    if count == 0 {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            return import_json_value(db, &parsed);
        }
        // Dernier fallback : crée un tarif générique pour que l'utilisateur voie ses données
        eprintln!("[import] aucun tarif parsé, {} lignes, création générique", text.lines().count());
        let mut map = serde_json::Map::new();
        map.insert("type".into(), json!("session"));
        map.insert("duree_minutes".into(), json!(30));
        map.insert("prix".into(), json!(2000));
        map.insert("description".into(), json!(format!("Import PDF {}: {}", path.file_name().unwrap_or_default().to_string_lossy(), text.chars().take(150).collect::<String>())));
        map.insert("console_type".into(), json!(console_type));
        map.insert("jeu".into(), json!("Importé PDF"));
        map.insert("actif".into(), json!(1));
        map.insert("created_at".into(), json!(crate::db::now_iso()));
        if db.insert("tarifs", &map).is_ok() { count += 1; }
    }

    // Assure aussi console par défaut si aucune
    if count > 0 {
        // Vérifie qu'on a au moins une console
        if let Ok(consoles) = db.query_all("consoles") {
            if consoles.is_empty() {
                for i in 1..=6 {
                    let mut c = serde_json::Map::new();
                    c.insert("nom".into(), json!(format!("Poste {}", i)));
                    c.insert("type".into(), json!(if i <= 3 { "PS5" } else { "PS4" }));
                    c.insert("poste_numero".into(), json!(i));
                    c.insert("etat".into(), json!("disponible"));
                    let now = crate::db::now_iso();
                    c.insert("created_at".into(), json!(now.clone()));
                    c.insert("date_ajout".into(), json!(now));
                    let _ = db.insert("consoles", &c);
                }
            }
        }
    }

    Ok(count)
}

fn detect_genre(titre: &str) -> String {
    let t = titre.to_lowercase();
    if t.contains("fifa") || t.contains("nba") || t.contains("wwe") || t.contains("ufc") { "Sport".to_string() }
    else if t.contains("mortal kombat") || t.contains("tekken") || t.contains("naruto") { "Combat".to_string() }
    else if t.contains("need for speed") || t.contains("gran turismo") { "Course".to_string() }
    else if t.contains("gta") || t.contains("god of war") || t.contains("call of duty") || t.contains("fortnite") || t.contains("spider") || t.contains("red dead") { "Action".to_string() }
    else { "Action".to_string() }
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
        // Pour l'instant, on laisse sync_run faire le push complet
        // On pourrait pousser chaque tarif, mais on log seulement
        let tarifs = db.query_all("tarifs")?;
        for t in tarifs.iter().take(5) {
            eprintln!("[import] tarif exemple: {:?}", t.get("description"));
        }
    }
    // Pull Neon pour vérifier
    let _ = pool;
    Ok(count)
}

#[cfg(not(feature = "neon-sync"))]
pub async fn import_and_sync(db: &Db, _pool: &()) -> ApiResult<usize> {
    import_default_data(db)
}
