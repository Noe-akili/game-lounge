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

// Hash Argon2id (best-practice OWASP)
pub fn hash_password(password: &str) -> ApiResult<String> {
    if password.len() < 6 || password.len() > 128 {
        return Err(ApiError::bad_request("Mot de passe invalide"));
    }
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(format!("Argon2: {}", e)))?
        .to_string())
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
pub fn needs_rehash(h: &str) -> bool { is_scrypt_hash(h) || is_bcrypt_hash(h) }

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
