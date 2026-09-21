// Auth propre - Argon2id + JWT HS256 + compat scrypt/bcrypt
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use scrypt::{scrypt, Params as ScryptParams};

use crate::error::{ApiError, ApiResult};

const SCRYPT_PREFIX: &str = "scrypt$v1";
const SCRYPT_KEYLEN: usize = 64;
const SCRYPT_LOG_N: u8 = 14;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;

/// Mémoire Argon2id en Kio. 1 Mio au lieu de 2 : sur un Android d'entrée de
/// gamme, chaque Mio réservé d'un coup est un risque d'échec d'allocation, et
/// une allocation qui échoue en Rust = arrêt IMMÉDIAT du processus (l'app se
/// ferme). Le coût en temps est compensé par t_cost = 2.
const ARGON2_M_COST_KIB: u32 = 1024;
const ARGON2_T_COST: u32 = 2;

// Hash Argon2id (calibré mobile pour exécution synchrone immédiate <30ms)
pub fn hash_password(password: &str) -> ApiResult<String> {
    if password.len() < 6 || password.len() > 128 {
        return Err(ApiError::bad_request("Mot de passe invalide"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let params = match argon2::Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, 1, None) {
        Ok(p) => p,
        Err(_) => argon2::Params::default(),
    };
    let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    Ok(argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(format!("Argon2: {}", e)))?
        .to_string())
}

/// Texte lisible d'une panique (pour le journal).
fn panic_texte(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "panique sans message".to_string()
    }
}

/// HACHAGE À TOUTE ÉPREUVE — c'est la fonction que les commandes doivent utiliser.
///
/// Pourquoi tout ça pour un simple hachage ? Parce que c'était la cause exacte de
/// « l'application se ferme toute seule quand je crée un utilisateur ou change un
/// mot de passe » :
///
///  1. Créer un compte et changer un mot de passe sont les SEULES opérations qui
///     hachent un mot de passe. Modifier l'e-mail ou le rôle n'y passe pas — d'où
///     le fait que ces deux-là fonctionnent et pas les autres.
///  2. Une commande Tauri SYNCHRONE s'exécute sur le thread principal Android
///     (celui de la WebView, appelé depuis Java). Si le code panique là, la
///     panique doit traverser la frontière Java/Rust : Rust n'a pas le droit de
///     le faire et coupe le processus (SIGABRT) => l'application disparaît sans
///     message. Avant, le hachage tournait dans une tâche de fond : la même
///     panique était contenue et donnait seulement « ça ne marche pas ».
///
/// Trois protections cumulées ici :
///  * exécution sur un thread DÉDIÉ avec 8 Mio de pile (plus de débordement
///    possible, et une panique meurt dans ce thread, jamais dans le thread Java) ;
///  * `catch_unwind` par algorithme : la panique devient une erreur affichable ;
///  * REPLI d'algorithme : Argon2id, puis scrypt, puis bcrypt. Les trois sont déjà
///    acceptés à la connexion (`compare_password`), donc même si Argon2 est
///    inutilisable sur cet appareil, le compte est créé et le mot de passe
///    fonctionne.
///
/// Retourne le hash et le nom de l'algorithme retenu (journalisé et renvoyé au
/// frontend, pour savoir ce qui s'est réellement passé sur l'appareil).
pub fn hash_password_resilient(password: &str) -> ApiResult<(String, &'static str)> {
    if password.len() < 6 || password.len() > 128 {
        return Err(ApiError::bad_request(
            "Mot de passe invalide (6 à 128 caractères)",
        ));
    }
    let pwd = password.to_string();
    let handle = std::thread::Builder::new()
        .name("hash-mdp".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || -> Result<(String, &'static str), String> {
            let mut echecs: Vec<String> = Vec::new();

            // 1) Argon2id (préféré)
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hash_password(&pwd))) {
                Ok(Ok(h)) => return Ok((h, "argon2id")),
                Ok(Err(e)) => echecs.push(format!("argon2: {}", e.message)),
                Err(p) => echecs.push(format!("argon2 PANIQUE: {}", panic_texte(&*p))),
            }

            // 2) scrypt (déjà utilisé historiquement par ce projet)
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                hash_password_scrypt(&pwd)
            })) {
                Ok(Ok(h)) => return Ok((h, "scrypt")),
                Ok(Err(e)) => echecs.push(format!("scrypt: {}", e.message)),
                Err(p) => echecs.push(format!("scrypt PANIQUE: {}", panic_texte(&*p))),
            }

            // 3) bcrypt (le plus léger en mémoire : dernier filet)
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                bcrypt::hash(&pwd, 10)
            })) {
                Ok(Ok(h)) => return Ok((h, "bcrypt")),
                Ok(Err(e)) => echecs.push(format!("bcrypt: {}", e)),
                Err(p) => echecs.push(format!("bcrypt PANIQUE: {}", panic_texte(&*p))),
            }

            Err(echecs.join(" | "))
        })
        .map_err(|e| ApiError::internal(format!("Thread de hachage non créé : {e}")))?;

    match handle.join() {
        Ok(Ok((hash, algo))) => {
            crate::logger::log_auth(&format!("hachage mot de passe OK ({algo})"));
            Ok((hash, algo))
        }
        Ok(Err(detail)) => {
            crate::logger::log_auth(&format!("hachage mot de passe ÉCHEC : {detail}"));
            Err(ApiError::internal(format!(
                "Impossible de sécuriser le mot de passe sur cet appareil ({detail})"
            )))
        }
        Err(p) => {
            let txt = panic_texte(&*p);
            crate::logger::log_auth(&format!("hachage mot de passe INTERROMPU : {txt}"));
            Err(ApiError::internal(format!(
                "Le calcul du mot de passe a été interrompu ({txt})"
            )))
        }
    }
}

// Hash scrypt legacy (seed initial, compat)
pub fn hash_password_scrypt(password: &str) -> ApiResult<String> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let params = ScryptParams::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, SCRYPT_KEYLEN)
        .map_err(|e| ApiError::internal(format!("scrypt: {}", e)))?;
    let mut dk = [0u8; SCRYPT_KEYLEN];
    scrypt(password.as_bytes(), &salt, &params, &mut dk)
        .map_err(|e| ApiError::internal(format!("scrypt: {}", e)))?;
    Ok(format!("{}${}${}", SCRYPT_PREFIX, URL_SAFE_NO_PAD.encode(salt), URL_SAFE_NO_PAD.encode(dk)))
}

pub fn is_scrypt_hash(h: &str) -> bool { h.starts_with(SCRYPT_PREFIX) }
pub fn is_argon2_hash(h: &str) -> bool { h.starts_with("$argon2") }
pub fn is_bcrypt_hash(h: &str) -> bool { h.starts_with("$2") }

// Compare multi-algo, jamais de panic (Android low-mem)
pub fn compare_password(password: &str, stored: &str) -> bool {
    if password.is_empty() || stored.is_empty() || password.len() > 128 || stored.len() > 1024 {
        return false;
    }
    if is_argon2_hash(stored) {
        return std::panic::catch_unwind(|| {
            if let Ok(parsed) = PasswordHash::new(stored) {
                Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
            } else { false }
        }).unwrap_or(false);
    }
    if is_scrypt_hash(stored) {
        let parts: Vec<&str> = stored.split('$').collect();
        if parts.len() != 4 { return false; }
        let salt = match URL_SAFE_NO_PAD.decode(parts[2]) { Ok(s) => s, Err(_) => return false };
        if salt.is_empty() || salt.len() > 64 || URL_SAFE_NO_PAD.decode(parts[3]).is_err() { return false; }
        let params = match ScryptParams::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, SCRYPT_KEYLEN) { Ok(p) => p, Err(_) => return false };
        let mut dk = [0u8; SCRYPT_KEYLEN];
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scrypt(password.as_bytes(), &salt, &params, &mut dk)));
        return match res {
            Ok(Ok(())) => {
                let actual = URL_SAFE_NO_PAD.encode(dk);
                actual.as_bytes().iter().zip(parts[3].as_bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0 && actual.len() == parts[3].len()
            }
            _ => false,
        };
    }
    if is_bcrypt_hash(stored) {
        return std::panic::catch_unwind(|| bcrypt::verify(password, stored).unwrap_or(false)).unwrap_or(false);
    }
    false
}

// JWT
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub id: i64,
    pub email: String,
    pub role: String,
    pub nom: String,
    pub iat: i64,
    pub exp: i64,
}
impl Claims {
    pub fn is_admin(&self) -> bool { self.role == "admin" }
}

pub fn sign_token(claims: &Claims, secret: &str) -> ApiResult<String> {
    sign_token_with_exp(claims, secret, 24 * 3600)
}
pub fn sign_token_with_exp(claims: &Claims, secret: &str, exp_secs: i64) -> ApiResult<String> {
    let now = chrono::Utc::now().timestamp();
    let c = Claims { iat: now, exp: now + exp_secs, ..claims.clone() };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    jsonwebtoken::encode(&header, &c, &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| ApiError::internal(format!("JWT: {}", e)))
}
pub fn sign_refresh_token(claims: &Claims, secret: &str) -> ApiResult<String> {
    sign_token_with_exp(claims, secret, 7 * 24 * 3600)
}
pub fn generate_token_pair(claims: &Claims, secret: &str) -> ApiResult<(String, String)> {
    Ok((sign_token(claims, secret)?, sign_refresh_token(claims, secret)?))
}
pub fn verify_token(token: &str, secret: &str) -> ApiResult<Claims> {
    if token.is_empty() || token.len() > 4096 || secret.is_empty() {
        return Err(ApiError::unauthorized("Token invalide ou expiré"));
    }
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.leeway = 5;
    validation.validate_exp = true;
    let data = std::panic::catch_unwind(|| {
        jsonwebtoken::decode::<Claims>(token, &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()), &validation)
    })
    .map_err(|_| ApiError::unauthorized("Token invalide ou expiré"))?
    .map_err(|_| ApiError::unauthorized("Token invalide ou expiré"))?;
    if data.claims.email.is_empty() || data.claims.role.is_empty() {
        return Err(ApiError::unauthorized("Token invalide ou expiré"));
    }
    Ok(data.claims)
}
pub fn bearer_token(authorization: Option<String>) -> Option<String> {
    match authorization {
        Some(h) if h.starts_with("Bearer ") => Some(h[7..].trim().to_string()),
        Some(h) if !h.is_empty() => Some(h.trim().to_string()),
        _ => None,
    }
}
pub fn require_auth(token: Option<&str>, secret: &str) -> ApiResult<Claims> {
    let token = token.ok_or_else(|| ApiError::unauthorized("Token manquant"))?;
    let token = bearer_token(Some(token.to_string())).ok_or_else(|| ApiError::unauthorized("Token manquant"))?;
    verify_token(&token, secret)
}
pub fn require_admin(user: &Claims) -> ApiResult<()> {
    if user.is_admin() { Ok(()) } else { Err(ApiError::forbidden("Accès réservé aux administrateurs")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_roundtrip() {
        let h = hash_password("admin123").unwrap();
        assert!(is_argon2_hash(&h));
        assert!(compare_password("admin123", &h));
        assert!(!compare_password("wrong", &h));
        let h2 = hash_password_scrypt("admin123").unwrap();
        assert!(is_scrypt_hash(&h2));
        assert!(compare_password("admin123", &h2));
    }
    #[test]
    fn jwt_roundtrip() {
        let claims = Claims { id: 1, email: "a@b.c".into(), role: "admin".into(), nom: "Admin".into(), iat: 0, exp: 0 };
        let token = sign_token(&claims, "secret").unwrap();
        let decoded = verify_token(&token, "secret").unwrap();
        assert_eq!(decoded.email, "a@b.c");
        assert!(verify_token(&token, "autre").is_err());
    }
}
