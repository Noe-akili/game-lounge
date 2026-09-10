// Générateur de facture PDF minimal (A4), sans dépendance externe.
// Équivalent du rendu jspdf côté Node (GAME LOUNGE / facture de prestation).

use serde_json::Value;

type PResult<T> = Result<T, String>;

fn acc(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            'œ' => out.push_str("oe"),
            'Œ' => out.push_str("OE"),
            '\u{00A0}' => out.push(' '),
            other => out.push(match other {
                'à' | 'â' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'î' | 'ï' => 'i',
                'ô' | 'ö' => 'o',
                'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'À' | 'Â' | 'Ä' => 'A',
                'É' | 'È' | 'Ê' | 'Ë' => 'E',
                'Î' | 'Ï' => 'I',
                'Ô' | 'Ö' => 'O',
                'Ù' | 'Û' | 'Ü' => 'U',
                'Ç' => 'C',
                other => other,
            }),
        }
    }
    out
}

/// Échappe un texte pour un littéral PDF `(...)`.
fn pdf_str(s: &str) -> String {
    let mut out = String::new();
    for c in acc(s).chars() {
        if !c.is_ascii() || (c as u8) < 32 || (c as u8) > 126 {
            continue;
        }
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            _ => out.push(c),
        }
    }
    out
}

fn fmt_fc(n: f64) -> String {
    let v = n.round() as i64;
    let neg = v < 0;
    let s = v.unsigned_abs().to_string();
    let grouped: Vec<String> = s
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or("").to_string())
        .collect();
    format!("{}{} FC", if neg { "-" } else { "" }, grouped.join(" "))
}

/// Largeur approximative d'un texte Helvetica (0.5 * taille par caractère).
fn text_width(text: &str, size: f64) -> f64 {
    let n = pdf_str(text).len() as f64;
    0.5 * size * n
}

const PURPLE: &str = "0.659 0.333 0.969";
const GRAY: &str = "0.784 0.784 0.784";

struct Content {
    s: String,
}

impl Content {
    fn text(&mut self, x: f64, y: f64, size: f64, txt: &str) {
        self.s.push_str(&format!(
            "0 0 0 rg\nBT\n/F1 {size} Tf {x:.1} {y:.1} Td ({}) Tj\nET\n",
            pdf_str(txt)
        ));
    }

    fn text_white(&mut self, x: f64, y: f64, size: f64, txt: &str) {
        self.s.push_str(&format!(
            "1 1 1 rg\nBT\n/F1 {size} Tf {x:.1} {y:.1} Td ({}) Tj\nET\n",
            pdf_str(txt)
        ));
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, width: f64) {
        self.s
            .push_str(&format!("{color} RG {width} w\n{x1:.1} {y1:.1} m {x2:.1} {y2:.1} l S\n"));
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: &str) {
        self.s
            .push_str(&format!("{color} rg {x:.1} {y:.1} {w:.1} {h:.1} re f\n"));
    }
}

pub struct InvoiceData<'a> {
    pub numero: &'a str,
    pub date: &'a str,
    pub joueur_nom: &'a str,
    pub joueur_tel: &'a str,
    pub montant_ht: f64,
    pub taux_tva: f64,
    pub montant_tva: f64,
    pub montant_ttc: f64,
    pub lignes: Vec<InvoiceLigne>,
}

pub struct InvoiceLigne {
    pub description: String,
    pub quantite: i64,
    pub prix_unitaire: f64,
    pub total_ligne: f64,
}

/// Génère le PDF de facture à partir des données JSON de la facture/lignes/joueur.
pub fn facture_pdf(f: &Value, joueur: Option<&Value>, lignes: &[Value]) -> PResult<Vec<u8>> {
    let numero = f
        .get("numero_facture")
        .and_then(Value::as_str)
        .unwrap_or("N/A");

    let created = f.get("created_at").and_then(Value::as_str).unwrap_or("");
    let date = if created.len() >= 10 {
        format!(
            "{}-{}-{}",
            &created[8..10],
            &created[5..7],
            &created[0..4]
        )
    } else {
        created.to_string()
    };

    let joueur_nom = joueur
        .and_then(|j| j.get("nom"))
        .and_then(Value::as_str)
        .unwrap_or("N/A")
        .to_string();
    let joueur_tel = joueur
        .and_then(|j| j.get("telephone"))
        .and_then(Value::as_str)
        .unwrap_or("N/A")
        .to_string();

    let lignes: Vec<InvoiceLigne> = lignes
        .iter()
        .map(|l| InvoiceLigne {
            description: l
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            quantite: l.get("quantite").and_then(Value::as_i64).unwrap_or(0),
            prix_unitaire: l.get("prix_unitaire").and_then(Value::as_f64).unwrap_or(0.0),
            total_ligne: l.get("total_ligne").and_then(Value::as_f64).unwrap_or(0.0),
        })
        .collect();

    let data = InvoiceData {
        numero,
        date: &date,
        joueur_nom: &joueur_nom,
        joueur_tel: &joueur_tel,
        montant_ht: f.get("montant_ht").and_then(Value::as_f64).unwrap_or(0.0),
        taux_tva: f.get("taux_tva").and_then(Value::as_f64).unwrap_or(20.0),
        montant_tva: f.get("montant_tva").and_then(Value::as_f64).unwrap_or(0.0),
        montant_ttc: f.get("montant_ttc").and_then(Value::as_f64).unwrap_or(0.0),
        lignes,
    };

    let mut c = Content { s: String::new() };

    // ----- Titre centré -----
    let t1 = "GAME LOUNGE";
    c.text(297.5 - text_width(t1, 22.0) / 2.0, 812.0, 22.0, t1);
    let t2 = "Facture de prestation";
    c.text(297.5 - text_width(t2, 10.0) / 2.0, 798.0, 10.0, t2);
    c.line(70.0, 788.0, 525.0, 788.0, PURPLE, 0.8);

    // ----- Informations -----
    c.text(70.0, 768.0, 11.0, &format!("N° {}", data.numero));
    c.text(70.0, 756.0, 10.0, &format!("Date : {}", data.date));
    c.text(70.0, 744.0, 10.0, &format!("Joueur : {}", data.joueur_nom));
    c.text(70.0, 732.0, 10.0, &format!("Téléphone : {}", data.joueur_tel));

    // ----- Bandeau d'en-tête du tableau -----
    c.fill_rect(70.0, 700.0, 455.0, 9.0, PURPLE);
    c.text_white(76.0, 703.5, 9.0, "Désignation");
    c.text_white(316.0, 703.5, 9.0, "Qté");
    c.text_white(352.0, 703.5, 9.0, "P.U.");
    c.text_white(448.0, 703.5, 9.0, "Total");

    // ----- Lignes -----
    let mut y: f64 = 684.0;
    for l in &data.lignes {
        c.text(74.0, y, 9.0, &l.description);
        c.text(320.0, y, 9.0, &l.quantite.to_string());
        c.text(352.0, y, 9.0, &fmt_fc(l.prix_unitaire));
        c.text(448.0, y, 9.0, &fmt_fc(l.total_ligne));
        y -= 9.0;
    }

    // ----- Totaux -----
    let line_total = y - 2.0;
    c.line(320.0, line_total, 525.0, line_total, GRAY, 0.2);
    c.text(346.0, line_total - 10.0, 9.0, &format!("Montant HT : {}", fmt_fc(data.montant_ht)));
    c.text(346.0, line_total - 20.0, 9.0, &format!("TVA ({:.0}%) : {}", data.taux_tva, fmt_fc(data.montant_tva)));
    let line_ttc = line_total - 30.0;
    c.line(320.0, line_ttc, 525.0, line_ttc, PURPLE, 0.8);
    c.text(346.0, line_ttc - 14.0, 13.0, &format!("TOTAL : {}", fmt_fc(data.montant_ttc)));

    // ----- Pied de page -----
    c.line(70.0, 60.0, 525.0, 60.0, PURPLE, 0.5);
    c.text(297.5 - text_width("Game Lounge - Merci pour votre visite !", 8.0) / 2.0, 46.0, 8.0, "Game Lounge - Merci pour votre visite !");

    // ----- Assemblage PDF -----
    let font = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>";
    let pages = "<< /Type /Pages /Kids [3 0 R] /Count 1 >>";
    let catalog = "<< /Type /Catalog /Pages 2 0 R >>";
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>";

    let content_bytes = c.s.as_bytes();
    let content = format!(
        "<< /Length {} >>\nstream\n{}endstream",
        content_bytes.len(),
        c.s
    );

    let mut pdf = Pdf::new(5);
    pdf.add_object(catalog);
    pdf.add_object(pages);
    pdf.add_object(page);
    pdf.add_object(font);
    pdf.add_object(&content);
    Ok(pdf.finish())
}

struct Pdf {
    out: Vec<u8>,
    offsets: Vec<usize>,
}

impl Pdf {
    fn new(n_objects: usize) -> Pdf {
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let offsets = Vec::with_capacity(n_objects);
        Pdf { out, offsets }
    }

    fn add_object(&mut self, body: &str) {
        self.offsets.push(self.out.len());
        let n = self.offsets.len();
        self.out
            .extend_from_slice(format!("{n} 0 obj\n{body}\nendobj\n").as_bytes());
    }

    fn finish(mut self) -> Vec<u8> {
        let count = self.offsets.len() + 1;
        self.out.extend_from_slice(b"xref\n");
        self.out.extend_from_slice(format!("0 {count}\n").as_bytes());
        self.out.extend_from_slice(b"0000000000 65535 f \n");
        for offset in &self.offsets {
            self.out
                .extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        self.out
            .extend_from_slice(format!("trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n").as_bytes());
        self.out
            .extend_from_slice(format!("{}\n%%EOF\n", self.out.len()).as_bytes());
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generate_pdf() {
        let f = json!({
            "numero_facture": "FAC-20240911-0042",
            "created_at": "2024-09-11T10:30:00.000Z",
            "montant_ht": 1667,
            "taux_tva": 20,
            "montant_tva": 333,
            "montant_ttc": 2000,
        });
        let joueur = json!({ "nom": "John Doe", "telephone": "+243990000000" });
        let lignes = json!([{
            "description": "Session PS5 - Poste 1 - FIFA 24 - 60min",
            "quantite": 1,
            "prix_unitaire": 2000,
            "total_ligne": 2000
        }]);
        let pdf = facture_pdf(&f, Some(&joueur), lignes.as_array().unwrap()).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(6).any(|w| w == b"%%EOF"));
    }
}