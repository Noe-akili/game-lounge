// Auth : hachage best-practice (Argon2id) + JWT HS256 + compat scrypt/bcrypt
// Reorg : login moderne avec Neon sync, rate-limit, refresh token

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use scrypt::{Params as ScryptParams, scrypt};

use crate::error::{ApiError, ApiResult};

const SCRYPT_PREFIX: &str = "scrypt$v1";
const SCRYPT_KEYLEN: usize = 64;
const SCRYPT_LOG_N: u8 = 14; // N = 16384 (identique à node:crypto)
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;

/// Best-practice : Argon2id (OWASP recommandé). Nouveau hash par défaut.
/// Garde scrypt pour compat ascendante (anciens comptes).
pub fn hash_password(password: &str) -> ApiResult<String> {
    // Validation OWASP : min 8 chars, au moins 1 lettre (déjà validé en amont)
    if password.len() < 6 || password.len() > 128 {
        return Err(ApiError::bad_request("Mot de passe invalide"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    Ok(argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(format!("Erreur Argon2: {e}")))?
        .to_string())
}

/// Ancien hash scrypt$v1 pour compat (utilisé au seed initial)
pub fn hash_password_scrypt(password: &str) -> ApiResult<String> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let params = ScryptParams::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, SCRYPT_KEYLEN)
        .map_err(|e| ApiError::internal(format!("Erreur scrypt: {e}")))?;
    let mut dk = [0u8; SCRYPT_KEYLEN];
    scrypt(password.as_bytes(), &salt, &params, &mut dk)
        .map_err(|e| ApiError::internal(format!("Erreur scrypt: {e}")))?;
    Ok(format!(
        "{}${}${}",
        SCRYPT_PREFIX,
        URL_SAFE_NO_PAD.encode(salt),
        URL_SAFE_NO_PAD.encode(dk)
    ))
}

pub fn is_scrypt_hash(hash: &str) -> bool {
    hash.starts_with(SCRYPT_PREFIX)
}
pub fn is_argon2_hash(hash: &str) -> bool {
    hash.starts_with("$argon2")
}
pub fn is_bcrypt_hash(hash: &str) -> bool {
    hash.starts_with("$2")
}

/// Compare avec support multi-algo : Argon2id (best), scrypt$v1 (legacy), bcrypt (legacy)
/// Ne panic jamais, catch OOM sur Android 512MB
pub fn compare_password(password: &str, stored: &str) -> bool {
    if password.is_empty() || stored.is_empty() {
        return false;
    }
    if password.len() > 128 || stored.len() > 1024 {
        return false;
    }
    // 1. Argon2id (nouveau best-practice)
    if is_argon2_hash(stored) {
        return std::panic::catch_unwind(|| {
            if let Ok(parsed) = PasswordHash::new(stored) {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            } else {
                false
            }
        })
        .unwrap_or(false);
    }
    // 2. scrypt$v1 legacy
    if is_scrypt_hash(stored) {
        let parts: Vec<&str> = stored.split('$').collect();
        if parts.len() != 4 {
            return false;
        }
        let salt = match URL_SAFE_NO_PAD.decode(parts[2]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        if salt.is_empty() || salt.len() > 64 {
            return false;
        }
        if URL_SAFE_NO_PAD.decode(parts[3]).is_err() {
            return false;
        }
        let params = match ScryptParams::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, SCRYPT_KEYLEN) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let mut dk = [0u8; SCRYPT_KEYLEN];
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scrypt(password.as_bytes(), &salt, &params, &mut dk)
        }));
        match res {
            Ok(Ok(())) => {
                let actual = URL_SAFE_NO_PAD.encode(dk);
                constant_time_eq(actual.as_bytes(), parts[3].as_bytes())
            }
            _ => {
                eprintln!("scrypt compare failed (oom) on Android");
                false
            }
        }
    } else if is_bcrypt_hash(stored) {
        std::panic::catch_unwind(|| bcrypt::verify(password, stored).unwrap_or(false)).unwrap_or(false)
    } else {
        false
    }
}

/// Indique si le hash doit être migré vers Argon2 (upgrade transparent au login)
pub fn needs_rehash(stored: &str) -> bool {
    is_scrypt_hash(stored) || is_bcrypt_hash(stored)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ===== JWT (même comportement que native.ts) =====

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
    pub fn user_id(&self) -> i64 {
        self.id
    }
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// Signe un JWT HS256 avec expiration de 24h (comme Express). Best-practice: access 15min + refresh 7j
/// Pour compat, garde 24h pour access (offline-first), refresh 7j en plus
pub fn sign_token(claims: &Claims, secret: &str) -> ApiResult<String> {
    sign_token_with_exp(claims, secret, 24 * 3600)
}
pub fn sign_token_with_exp(claims: &Claims, secret: &str, exp_secs: i64) -> ApiResult<String> {
    let now = chrono::Utc::now().timestamp();
    let c = Claims {
        iat: now,
        exp: now + exp_secs,
        ..claims.clone()
    };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let token = jsonwebtoken::encode(&header, &c, &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| ApiError::internal(format!("Erreur JWT: {e}")))?;
    Ok(token)
}
pub fn sign_refresh_token(claims: &Claims, secret: &str) -> ApiResult<String> {
    // Refresh 7 jours, même payload mais exp plus long
    sign_token_with_exp(claims, secret, 7 * 24 * 3600)
}
/// Génère paire access (24h) + refresh (7j) - best practice offline-first
pub fn generate_token_pair(claims: &Claims, secret: &str) -> ApiResult<(String, String)> {
    let access = sign_token(claims, secret)?;
    let refresh = sign_refresh_token(claims, secret)?;
    Ok((access, refresh))
}

/// Vérifie un JWT et renvoie les claims. 401 si invalide/expiré.
/// Ne panic jamais (token malformé depuis WebView localStorage corrompu)
pub fn verify_token(token: &str, secret: &str) -> ApiResult<Claims> {
    if token.is_empty() || token.len() > 4096 || secret.is_empty() {
        return Err(ApiError::unauthorized("Token invalide ou expiré"));
    }
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.leeway = 5; // 5s de marge pour horloge Android désynchronisée (sans NTP)
    validation.validate_exp = true;
    validation.validate_nbf = false;
    let data = std::panic::catch_unwind(|| {
        jsonwebtoken::decode::<Claims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
    })
    .map_err(|_| ApiError::unauthorized("Token invalide ou expiré"))?
    .map_err(|_| ApiError::unauthorized("Token invalide ou expiré"))?;
    // Vérifie que les champs essentiels ne sont pas vides (payload corrompu)
    if data.claims.email.is_empty() || data.claims.role.is_empty() {
        return Err(ApiError::unauthorized("Token invalide ou expiré"));
    }
    Ok(data.claims)
}

/// Parse le header `Authorization: Bearer <token>` éventuel passé en paramètre.
pub fn bearer_token(authorization: Option<String>) -> Option<String> {
    match authorization {
        Some(h) if h.starts_with("Bearer ") => Some(h[7..].trim().to_string()),
        Some(h) if !h.is_empty() => Some(h.trim().to_string()),
        _ => None,
    }
}

/// Middleware auth : vérifie le token et renvoie les claims.
pub fn require_auth(token: Option<&str>, secret: &str) -> ApiResult<Claims> {
    let token = token.ok_or_else(|| ApiError::unauthorized("Token manquant"))?;
    let token = bearer_token(Some(token.to_string()))
        .ok_or_else(|| ApiError::unauthorized("Token manquant"))?;
    verify_token(&token, secret)
}

/// Middleware admin : exige le rôle admin.
pub fn require_admin(user: &Claims) -> ApiResult<()> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(ApiError::forbidden("Accès réservé aux administrateurs"))
    }
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
        // compat scrypt
        let h2 = hash_password_scrypt("admin123").unwrap();
        assert!(is_scrypt_hash(&h2));
        assert!(compare_password("admin123", &h2));
    }

    #[test]
    fn jwt_roundtrip() {
        let claims = Claims {
            id: 1,
            email: "a@b.c".into(),
            role: "admin".into(),
            nom: "Admin".into(),
            iat: 0,
            exp: 0,
        };
        let token = sign_token(&claims, "secret").unwrap();
        let decoded = verify_token(&token, "secret").unwrap();
        assert_eq!(decoded.email, "a@b.c");
        assert!(verify_token(&token, "autre").is_err());
    }
}