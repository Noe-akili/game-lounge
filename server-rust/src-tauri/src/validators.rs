// Validators backend - port de server/utils/validators.ts
// Mêmes regex et règles que le backend Node/Express.

use regex::Regex;
use std::sync::OnceLock;

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap())
}
fn phone_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\+?[0-9]{8,15}$").unwrap())
}
fn password_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?=.*[A-Za-z]).{6,}$").unwrap())
}
fn nom_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-ZÀ-ÿ0-9\s\-'&]{2,50}$").unwrap())
}
fn titre_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-ZÀ-ÿ0-9\s\-'&]{2,100}$").unwrap())
}
fn genre_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-zA-ZÀ-ÿ0-9\s\-'&]{2,50}$").unwrap())
}
fn pattern(re: &str) -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(re).unwrap())
}
fn role_re() -> &'static Regex {
    pattern(r"^(admin|employe)$")
}
fn console_type_re() -> &'static Regex {
    pattern(r"^(PS5|PS4|XBOX|PC|SWITCH)$")
}
fn tarif_type_re() -> &'static Regex {
    pattern(r"^(horaire|forfait|session|partie)$")
}
fn regle_type_re() -> &'static Regex {
    pattern(r"^(temps|montant)$")
}
fn jeton_type_re() -> &'static Regex {
    pattern(r"^(gain|depense|bonus)$")
}
fn statut_session_re() -> &'static Regex {
    pattern(r"^(en_cours|pause|terminee|annulee)$")
}
fn statut_facture_re() -> &'static Regex {
    pattern(r"^(payee|en_attente|annulee)$")
}
fn mode_paiement_re() -> &'static Regex {
    pattern(r"^(especes|carte|mobile|mobile_money|jetons)$")
}

/// Supprime ` < ` et ` > `, tronque à max_length (port de sanitizeInput).
pub fn sanitize_input(input: &str, max_length: usize) -> String {
    input.replace(['<', '>'], "").trim().chars().take(max_length).collect()
}

pub fn is_valid_email(email: &str) -> bool {
    email_re().is_match(email.trim())
}

pub fn is_valid_phone(phone: &str) -> bool {
    let cleaned: String = phone.replace([' ', '-'], "");
    !cleaned.is_empty() && phone_re().is_match(&cleaned)
}

pub fn is_valid_password(password: &str) -> bool {
    password_re().is_match(password)
}

pub fn is_valid_nom(nom: &str) -> bool {
    let sanitized = sanitize_input(nom, 50);
    nom_re().is_match(sanitized.trim())
}

pub fn is_valid_titre(titre: &str) -> bool {
    let sanitized = sanitize_input(titre, 100);
    titre_re().is_match(sanitized.trim())
}

pub fn is_valid_genre(genre: Option<&str>) -> bool {
    match genre {
        Some(g) if !g.trim().is_empty() => {
            let sanitized = sanitize_input(g, 50);
            genre_re().is_match(sanitized.trim())
        }
        _ => true,
    }
}

pub fn is_valid_role(role: &str) -> bool {
    role_re().is_match(role)
}

pub fn is_valid_console_type(ty: &str) -> bool {
    console_type_re().is_match(ty)
}

pub fn is_valid_tarif_type(ty: &str) -> bool {
    tarif_type_re().is_match(ty)
}

pub fn is_valid_regle_type(ty: &str) -> bool {
    regle_type_re().is_match(ty)
}

pub fn is_valid_jeton_type(ty: &str) -> bool {
    jeton_type_re().is_match(ty)
}

pub fn is_valid_session_statut(s: &str) -> bool {
    statut_session_re().is_match(s)
}

pub fn is_valid_facture_statut(s: &str) -> bool {
    statut_facture_re().is_match(s)
}

pub fn is_valid_mode_paiement(m: &str) -> bool {
    mode_paiement_re().is_match(m)
}

pub fn is_valid_poste_numero(n: i64) -> bool {
    n >= 1 && n <= 100
}

pub fn is_valid_duree(n: i64) -> bool {
    n >= 1 && n <= 1000
}

pub fn is_valid_prix(n: f64) -> bool {
    n.is_finite() && n >= 1.0 && n <= 1_000_000.0
}

pub fn is_valid_seuil(n: i64) -> bool {
    n >= 1 && n <= 10_000
}

pub fn is_valid_jetons_attribues(n: i64) -> bool {
    n >= 1 && n <= 1000
}

pub fn is_valid_quantite(n: i64) -> bool {
    n >= 1 && n <= 10_000
}

pub fn is_valid_id(n: i64) -> bool {
    n > 0
}

pub fn is_valid_contenu(contenu: &str) -> bool {
    let sanitized = sanitize_input(contenu, 1000);
    !sanitized.is_empty() && sanitized.chars().count() <= 1000
}

/// Description de ligne de facture : 2-500 caractères après sanitisation.
pub fn is_valid_description(desc: &str) -> bool {
    let sanitized = sanitize_input(desc, 500);
    sanitized.chars().count() >= 2 && sanitized.chars().count() <= 500
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert!(is_valid_email("admin@gamelounge.com"));
        assert!(!is_valid_email("nope"));
    }

    #[test]
    fn mots_de_passe() {
        assert!(is_valid_password("admin123"));
        assert!(!is_valid_password("1234567890"));
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_input("<b>Cool</b>", 500), "bCool/b");
    }
}