// ============================================================
// FACTURE PDF — mise en page « propre » sans aucune dépendance
// ------------------------------------------------------------
// Objectifs de ce générateur :
//   * un vrai document présentable : bandeau d'en-tête coloré, encadrés
//     « Facturé à » / « Informations », tableau zébré, bloc de totaux,
//     pied de page avec numéro de page ;
//   * les ACCENTS s'affichent (encodage WinAnsi + échappement octal) ;
//   * l'alignement est exact (tables de largeurs Helvetica / Helvetica-Bold),
//     donc les montants sont vraiment alignés à droite ;
//   * plusieurs PAGES automatiques quand il y a beaucoup de lignes ;
//   * le nom de l'application (réglable dans Paramètres) est utilisé partout.
// Aucune caisse noire : tout est écrit à la main, léger pour un téléphone.
// ============================================================

use serde_json::Value;

type PResult<T> = Result<T, String>;

// ---------- Largeurs des caractères (millièmes de point, AFM Helvetica) ----------
// Index 0 = espace (code 32) … index 94 = '~' (code 126).
const W_REGULAR: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const W_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Lettre de base utilisée pour estimer la largeur d'un caractère accentué.
fn lettre_de_base(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ç' => 'c',
        'ÿ' => 'y',
        'ñ' => 'n',
        'À' | 'Â' | 'Ä' | 'Á' => 'A',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'Î' | 'Ï' => 'I',
        'Ô' | 'Ö' => 'O',
        'Ù' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        'Œ' => 'O',
        'œ' => 'o',
        '°' => 'o',
        '€' => 'E',
        '—' | '–' => '-',
        '’' | '‘' => '\'',
        '“' | '”' | '«' | '»' => '"',
        '\u{00A0}' => ' ',
        autre => autre,
    }
}

fn largeur_caractere(c: char, gras: bool) -> f64 {
    let base = lettre_de_base(c);
    let table = if gras { &W_BOLD } else { &W_REGULAR };
    let code = base as u32;
    let m = if (32..=126).contains(&code) {
        table[(code - 32) as usize]
    } else {
        556 // accent ou caractère inhabituel : largeur moyenne
    };
    m as f64 / 1000.0
}

/// Largeur d'un texte dans la police choisie (en points).
fn largeur_texte(texte: &str, taille: f64, gras: bool) -> f64 {
    texte.chars().map(|c| largeur_caractere(c, gras)).sum::<f64>() * taille
}

/// Prépare un texte pour un littéral PDF `(...)` en encodage WinAnsi :
/// les accents deviennent des échappements octaux (\351 pour « é »), donc ils
/// s'affichent vraiment au lieu d'être supprimés comme avant.
fn pdf_texte(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        // Quelques signes typographiques ont un code propre en WinAnsi.
        let special = match c {
            '\u{2013}' => Some(0x96u32), // tiret demi-cadratin –
            '\u{2014}' => Some(0x97),    // tiret cadratin —
            '\u{2026}' => Some(0x85),    // points de suspension …
            '\u{2019}' => Some(0x92),    // apostrophe typographique ’
            '\u{20AC}' => Some(0x80),    // euro €
            _ => None,
        };
        // WinAnsi = Latin-1 pour tout le reste de ce qui nous intéresse.
        let code = special.unwrap_or(c as u32);
        let code = if code <= 255 { code } else { lettre_de_base(c) as u32 };
        let code = if code <= 255 { code } else { b'?' as u32 };
        match code {
            40 => out.push_str("\\("),
            41 => out.push_str("\\)"),
            92 => out.push_str("\\\\"),
            32..=126 => out.push(code as u8 as char),
            128..=255 => out.push_str(&format!("\\{:03o}", code)),
            _ => out.push(' '),
        }
    }
    out
}

/// Montant en francs congolais, groupé par milliers : « 20 000 FC ».
fn fmt_fc(n: f64) -> String {
    let v = n.round() as i64;
    let s = v.unsigned_abs().to_string();
    let groupes: Vec<String> = s
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect();
    format!("{}{} FC", if v < 0 { "-" } else { "" }, groupes.join(" "))
}

// ---------- Palette (mêmes couleurs que l'application) ----------
const VIOLET: &str = "0.588 0.278 0.898";
const VIOLET_SOMBRE: &str = "0.357 0.157 0.596";
const VERT: &str = "0.106 0.620 0.360";
const ROUGE: &str = "0.850 0.240 0.240";
const ENCRE: &str = "0.129 0.137 0.176";
const GRIS: &str = "0.420 0.450 0.520";
const TRAIT: &str = "0.878 0.894 0.918";
const ZEBRE: &str = "0.973 0.976 0.984";
const BOITE: &str = "0.969 0.973 0.980";
const BLANC: &str = "1 1 1";

#[derive(Copy, Clone, PartialEq)]
enum Al {
    Gauche,
    Droite,
}

/// Une page = un flux de dessin.
struct Page {
    s: String,
}

impl Page {
    fn new() -> Page {
        Page { s: String::new() }
    }

    fn texte(&mut self, x: f64, y: f64, taille: f64, txt: &str, couleur: &str, gras: bool, al: Al) {
        let x = match al {
            Al::Gauche => x,
            Al::Droite => x - largeur_texte(txt, taille, gras),
        };
        let police = if gras { "F2" } else { "F1" };
        self.s.push_str(&format!(
            "{couleur} rg\nBT /{police} {taille} Tf {x:.1} {y:.1} Td ({}) Tj ET\n",
            pdf_texte(txt)
        ));
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, couleur: &str) {
        self.s
            .push_str(&format!("{couleur} rg {x:.1} {y:.1} {w:.1} {h:.1} re f\n"));
    }

    fn cadre(&mut self, x: f64, y: f64, w: f64, h: f64, couleur: &str) {
        self.s
            .push_str(&format!("{couleur} RG 0.6 w {x:.1} {y:.1} {w:.1} {h:.1} re S\n"));
    }

    fn trait_h(&mut self, x1: f64, x2: f64, y: f64, couleur: &str) {
        self.s
            .push_str(&format!("{couleur} RG 0.6 w {x1:.1} {y:.1} m {x2:.1} {y:.1} l S\n"));
    }
}

// ---------- Données ----------
pub struct InvoiceLigne {
    pub description: String,
    pub quantite: i64,
    pub prix_unitaire: f64,
    pub total_ligne: f64,
}

pub struct InvoiceData<'a> {
    /// Nom de l'établissement (réglage « app_name »).
    pub app: &'a str,
    pub numero: &'a str,
    pub date: &'a str,
    pub joueur_nom: &'a str,
    pub joueur_tel: &'a str,
    pub session: String,
    pub mode_paiement: String,
    /// Code brut : payee / en_attente / annulee.
    pub statut: String,
    pub montant_ht: f64,
    pub taux_tva: f64,
    pub montant_tva: f64,
    pub montant_ttc: f64,
    pub lignes: Vec<InvoiceLigne>,
}

// ---------- Géométrie de la page A4 ----------
const GAUCHE: f64 = 48.0;
const DROITE: f64 = 547.0;
const COL_QTE: f64 = 352.0;
const COL_PU: f64 = 448.0;
const COL_TOTAL: f64 = 541.0;
const HAUTEUR_LIGNE: f64 = 17.0;
/// En dessous de cette hauteur on passe à la page suivante (place pour le pied).
const PLANCHER: f64 = 120.0;

fn libelle_statut(code: &str) -> &'static str {
    match code {
        "payee" => "Payée",
        "en_attente" => "En attente",
        "annulee" => "Annulée",
        _ => "—",
    }
}

fn libelle_paiement(code: &str) -> String {
    match code {
        "especes" => "Espèces".to_string(),
        "carte" => "Carte bancaire".to_string(),
        "mobile" => "Mobile Money".to_string(),
        "jetons" => "Jetons de fidélité".to_string(),
        "" => "Non précisé".to_string(),
        autre => autre.to_string(),
    }
}

fn entete(page: &mut Page, d: &InvoiceData, suite: bool) {
    page.rect(0.0, 742.0, 595.0, 100.0, VIOLET);
    page.rect(0.0, 738.0, 595.0, 4.0, VIOLET_SOMBRE);
    page.texte(GAUCHE, 800.0, 21.0, d.app, BLANC, true, Al::Gauche);
    page.texte(
        GAUCHE,
        784.0,
        9.0,
        "Salle de jeux vidéo — Facture de prestation",
        "0.900 0.870 0.980",
        false,
        Al::Gauche,
    );
    let titre = if suite { "FACTURE (suite)" } else { "FACTURE" };
    page.texte(DROITE, 802.0, 11.0, titre, BLANC, true, Al::Droite);
    page.texte(
        DROITE,
        787.0,
        10.0,
        &format!("N° {}", d.numero),
        BLANC,
        false,
        Al::Droite,
    );
    page.texte(
        DROITE,
        773.0,
        9.0,
        &format!("Date : {}", d.date),
        "0.900 0.870 0.980",
        false,
        Al::Droite,
    );
}

/// Bandeau d'en-tête du tableau ; renvoie la ligne de base de la 1re ligne.
fn tete_tableau(page: &mut Page, y: f64) -> f64 {
    page.rect(GAUCHE, y, DROITE - GAUCHE, 18.0, VIOLET);
    page.texte(GAUCHE + 6.0, y + 5.5, 8.5, "DÉSIGNATION", BLANC, true, Al::Gauche);
    page.texte(COL_QTE, y + 5.5, 8.5, "QTÉ", BLANC, true, Al::Droite);
    page.texte(COL_PU, y + 5.5, 8.5, "P.U.", BLANC, true, Al::Droite);
    page.texte(COL_TOTAL, y + 5.5, 8.5, "TOTAL", BLANC, true, Al::Droite);
    y - HAUTEUR_LIGNE
}

fn pied(page: &mut Page, d: &InvoiceData, page_no: usize, total: usize) {
    page.trait_h(GAUCHE, DROITE, 78.0, TRAIT);
    page.texte(
        GAUCHE,
        64.0,
        8.0,
        &format!("{} — Merci pour votre visite !", d.app),
        GRIS,
        false,
        Al::Gauche,
    );
    page.texte(
        DROITE,
        64.0,
        8.0,
        &format!("Page {}/{}", page_no, total),
        GRIS,
        false,
        Al::Droite,
    );
    page.texte(
        GAUCHE,
        52.0,
        7.5,
        "Document généré automatiquement par l'application — aucune signature requise.",
        GRIS,
        false,
        Al::Gauche,
    );
}

/// Coupe une description trop large pour la colonne « Désignation ».
fn couper(desc: &str, largeur_max: f64, taille: f64) -> String {
    if largeur_texte(desc, taille, false) <= largeur_max {
        return desc.to_string();
    }
    let mut out = String::new();
    for c in desc.chars() {
        if largeur_texte(&format!("{out}{c}…"), taille, false) > largeur_max {
            break;
        }
        out.push(c);
    }
    format!("{}…", out.trim_end())
}

fn dessiner(d: &InvoiceData) -> Vec<Page> {
    let mut pages: Vec<Page> = vec![Page::new()];
    entete(pages.last_mut().unwrap(), d, false);

    // ----- Encadrés « Facturé à » et « Informations » -----
    let haut = 710.0;
    let h_boite = 76.0;
    let y_boite = haut - h_boite;
    {
        let p = pages.last_mut().unwrap();
        p.rect(GAUCHE, y_boite, 245.0, h_boite, BOITE);
        p.cadre(GAUCHE, y_boite, 245.0, h_boite, TRAIT);
        p.rect(GAUCHE + 257.0, y_boite, 242.0, h_boite, BOITE);
        p.cadre(GAUCHE + 257.0, y_boite, 242.0, h_boite, TRAIT);

        p.texte(GAUCHE + 10.0, haut - 16.0, 7.5, "FACTURÉ À", VIOLET, true, Al::Gauche);
        p.texte(GAUCHE + 10.0, haut - 33.0, 12.0, d.joueur_nom, ENCRE, true, Al::Gauche);
        p.texte(
            GAUCHE + 10.0,
            haut - 47.0,
            9.0,
            &format!("Tél. : {}", d.joueur_tel),
            GRIS,
            false,
            Al::Gauche,
        );
        p.texte(
            GAUCHE + 10.0,
            haut - 60.0,
            9.0,
            if d.joueur_nom == "N/A" { "Client anonyme" } else { "Client enregistré" },
            GRIS,
            false,
            Al::Gauche,
        );

        let x2 = GAUCHE + 267.0;
        p.texte(x2, haut - 16.0, 7.5, "INFORMATIONS", VIOLET, true, Al::Gauche);
        p.texte(x2, haut - 33.0, 9.0, &format!("Session : {}", d.session), ENCRE, false, Al::Gauche);
        p.texte(
            x2,
            haut - 46.0,
            9.0,
            &format!("Paiement : {}", libelle_paiement(&d.mode_paiement)),
            ENCRE,
            false,
            Al::Gauche,
        );
        let couleur_statut = if d.statut == "payee" { VERT } else { ROUGE };
        p.texte(
            x2,
            haut - 59.0,
            9.0,
            &format!("Statut : {}", libelle_statut(&d.statut)),
            couleur_statut,
            true,
            Al::Gauche,
        );
    }

    // ----- Tableau des lignes -----
    let mut y = tete_tableau(pages.last_mut().unwrap(), y_boite - 44.0);
    let mut zebre = 0usize;
    for l in &d.lignes {
        if y < PLANCHER {
            let mut p = Page::new();
            entete(&mut p, d, true);
            pages.push(p);
            y = tete_tableau(pages.last_mut().unwrap(), 700.0);
            zebre = 0;
        }
        let p = pages.last_mut().unwrap();
        if zebre % 2 == 1 {
            p.rect(GAUCHE, y - 4.5, DROITE - GAUCHE, HAUTEUR_LIGNE, ZEBRE);
        }
        p.texte(GAUCHE + 6.0, y, 9.0, &couper(&l.description, 285.0, 9.0), ENCRE, false, Al::Gauche);
        p.texte(COL_QTE, y, 9.0, &l.quantite.to_string(), ENCRE, false, Al::Droite);
        p.texte(COL_PU, y, 9.0, &fmt_fc(l.prix_unitaire), ENCRE, false, Al::Droite);
        p.texte(COL_TOTAL, y, 9.0, &fmt_fc(l.total_ligne), ENCRE, true, Al::Droite);
        y -= HAUTEUR_LIGNE;
        zebre += 1;
    }
    if d.lignes.is_empty() {
        let p = pages.last_mut().unwrap();
        p.texte(
            GAUCHE + 6.0,
            y,
            9.0,
            "Aucune ligne détaillée sur cette facture.",
            GRIS,
            false,
            Al::Gauche,
        );
        y -= HAUTEUR_LIGNE;
    }
    pages.last_mut().unwrap().trait_h(GAUCHE, DROITE, y + 10.0, TRAIT);

    // ----- Bloc des totaux -----
    let h_totaux = 86.0;
    if y - h_totaux < PLANCHER {
        let mut p = Page::new();
        entete(&mut p, d, true);
        pages.push(p);
        y = 700.0;
    }
    {
        let p = pages.last_mut().unwrap();
        let ty = y - 8.0;
        let bx = 320.0;
        let bw = DROITE - bx;
        p.rect(bx, ty - h_totaux, bw, h_totaux, BOITE);
        p.cadre(bx, ty - h_totaux, bw, h_totaux, TRAIT);
        p.texte(bx + 12.0, ty - 20.0, 9.0, "Montant HT", GRIS, false, Al::Gauche);
        p.texte(DROITE - 12.0, ty - 20.0, 9.0, &fmt_fc(d.montant_ht), ENCRE, false, Al::Droite);
        p.texte(
            bx + 12.0,
            ty - 36.0,
            9.0,
            &format!("TVA ({:.0} %)", d.taux_tva),
            GRIS,
            false,
            Al::Gauche,
        );
        p.texte(DROITE - 12.0, ty - 36.0, 9.0, &fmt_fc(d.montant_tva), ENCRE, false, Al::Droite);
        p.rect(bx, ty - h_totaux, bw, 34.0, VIOLET);
        p.texte(bx + 12.0, ty - h_totaux + 12.0, 10.0, "TOTAL À PAYER", BLANC, true, Al::Gauche);
        p.texte(
            DROITE - 12.0,
            ty - h_totaux + 11.0,
            13.0,
            &fmt_fc(d.montant_ttc),
            BLANC,
            true,
            Al::Droite,
        );

        // Rappels à gauche du bloc de totaux.
        p.texte(
            GAUCHE,
            ty - 20.0,
            9.0,
            &format!("Mode de paiement : {}", libelle_paiement(&d.mode_paiement)),
            ENCRE,
            false,
            Al::Gauche,
        );
        p.texte(
            GAUCHE,
            ty - 36.0,
            8.5,
            &format!("Nombre de lignes : {}", d.lignes.len()),
            GRIS,
            false,
            Al::Gauche,
        );
        if d.statut != "payee" {
            p.texte(
                GAUCHE,
                ty - 56.0,
                11.0,
                &format!("FACTURE {}", libelle_statut(&d.statut).to_uppercase()),
                ROUGE,
                true,
                Al::Gauche,
            );
        }
    }

    // ----- Pieds de page (numérotation connue seulement à la fin) -----
    let total = pages.len();
    for (i, p) in pages.iter_mut().enumerate() {
        pied(p, d, i + 1, total);
    }
    pages
}

/// Génère le PDF de facture. `app_name` = nom de l'établissement (Paramètres).
pub fn facture_pdf(
    f: &Value,
    joueur: Option<&Value>,
    lignes: &[Value],
    app_name: &str,
) -> PResult<Vec<u8>> {
    let numero = f.get("numero_facture").and_then(Value::as_str).unwrap_or("N/A");

    // Date lisible : JJ/MM/AAAA HH:MM à partir de l'ISO stocké en base.
    let brut = f
        .get("date_paiement")
        .and_then(Value::as_str)
        .filter(|s| s.len() >= 10)
        .or_else(|| f.get("created_at").and_then(Value::as_str))
        .unwrap_or("");
    let date = if brut.is_ascii() && brut.len() >= 16 {
        format!("{}/{}/{} {}", &brut[8..10], &brut[5..7], &brut[0..4], &brut[11..16])
    } else if brut.is_ascii() && brut.len() >= 10 {
        format!("{}/{}/{}", &brut[8..10], &brut[5..7], &brut[0..4])
    } else {
        brut.to_string()
    };

    let joueur_nom = joueur
        .and_then(|j| j.get("nom"))
        .and_then(Value::as_str)
        .unwrap_or("N/A")
        .to_string();
    let joueur_tel = joueur
        .and_then(|j| j.get("telephone"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("N/A")
        .to_string();

    let lignes: Vec<InvoiceLigne> = lignes
        .iter()
        .map(|l| InvoiceLigne {
            description: l.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
            quantite: l.get("quantite").and_then(Value::as_i64).unwrap_or(0),
            prix_unitaire: l.get("prix_unitaire").and_then(Value::as_f64).unwrap_or(0.0),
            total_ligne: l.get("total_ligne").and_then(Value::as_f64).unwrap_or(0.0),
        })
        .collect();

    let app = if app_name.trim().is_empty() { "Game Lounge" } else { app_name.trim() };

    let data = InvoiceData {
        app,
        numero,
        date: &date,
        joueur_nom: &joueur_nom,
        joueur_tel: &joueur_tel,
        session: f
            .get("session_id")
            .and_then(Value::as_i64)
            .map(|s| format!("#{s}"))
            .unwrap_or_else(|| "—".to_string()),
        mode_paiement: f
            .get("mode_paiement")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        statut: f.get("statut").and_then(Value::as_str).unwrap_or("payee").to_string(),
        montant_ht: f.get("montant_ht").and_then(Value::as_f64).unwrap_or(0.0),
        taux_tva: f.get("taux_tva").and_then(Value::as_f64).unwrap_or(0.0),
        montant_tva: f.get("montant_tva").and_then(Value::as_f64).unwrap_or(0.0),
        montant_ttc: f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0),
        lignes,
    };

    Ok(assembler(dessiner(&data)))
}

/// Écrit les objets PDF (catalogue, pages, 2 polices, puis page + contenu).
fn assembler(pages: Vec<Page>) -> Vec<u8> {
    let n = pages.len();
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + 2 * i)).collect();

    let mut objets: Vec<String> = Vec::with_capacity(4 + 2 * n);
    objets.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objets.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        n
    ));
    objets.push(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_string(),
    );
    objets.push(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_string(),
    );
    for (i, p) in pages.iter().enumerate() {
        let contenu_no = 5 + 2 * i + 1;
        objets.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {contenu_no} 0 R >>"
        ));
        objets.push(format!(
            "<< /Length {} >>\nstream\n{}endstream",
            p.s.len(),
            p.s
        ));
    }

    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets: Vec<usize> = Vec::with_capacity(objets.len());
    for (i, corps) in objets.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, corps).as_bytes());
    }
    // Position de la table xref : DOIT être notée avant de l'écrire (l'ancienne
    // version indiquait une position fausse, certains lecteurs s'en plaignaient).
    let debut_xref = out.len();
    let taille = objets.len() + 1;
    out.extend_from_slice(b"xref\n");
    out.extend_from_slice(format!("0 {taille}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {taille} /Root 1 0 R >>\nstartxref\n{debut_xref}\n%%EOF\n")
            .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn facture_test() -> Value {
        json!({
            "numero_facture": "FAC-20240911-0042",
            "created_at": "2024-09-11T10:30:00.000Z",
            "session_id": 128,
            "mode_paiement": "especes",
            "statut": "payee",
            "montant_ht": 16667,
            "taux_tva": 16,
            "montant_tva": 3333,
            "montant_ttc": 20000,
        })
    }

    #[test]
    fn generate_pdf() {
        let f = facture_test();
        let joueur = json!({ "nom": "Noé Akili", "telephone": "+243990000000" });
        let lignes = json!([{
            "description": "Session PS5 - Poste 1 - EA FC 26 - 60min",
            "quantite": 1,
            "prix_unitaire": 20000,
            "total_ligne": 20000
        }]);
        let pdf = facture_pdf(&f, Some(&joueur), lignes.as_array().unwrap(), "Game Lounge").unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn plusieurs_pages_quand_beaucoup_de_lignes() {
        let f = facture_test();
        let lignes: Vec<Value> = (1..=60)
            .map(|i| json!({
                "description": format!("Ligne {i}"),
                "quantite": 1,
                "prix_unitaire": 1000,
                "total_ligne": 1000
            }))
            .collect();
        let pdf = facture_pdf(&f, None, &lignes, "Game Lounge").unwrap();
        let texte = String::from_utf8_lossy(&pdf);
        assert!(texte.contains("/Count 2") || texte.contains("/Count 3"));
    }

    #[test]
    fn les_accents_sont_encodes_en_octal() {
        assert_eq!(pdf_texte("Espèces"), "Esp\\350ces");
        assert_eq!(pdf_texte("Payée"), "Pay\\351e");
    }
}
