// Auth : hachage de mot de passe compatible Node (scrypt$v1$) + JWT HS256.
// Port de server/utils/native.ts + server/server.ts.

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

/// Hache un mot de passe au format `scrypt$v1$<salt>$<hash>` (base64url), comme native.ts.
pub fn hash_password(password: &str) -> ApiResult<String> {
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

/// Compare un mot de passe avec un hash stocké.
/// - hash `scrypt$v1$...` : vérifié avec scrypt (paramètres Node).
/// - hash bcrypt `$2a/$2b/$2y` : vérifié avec la crate `bcrypt` (rétrocompatibilité).
pub fn compare_password(password: &str, stored: &str) -> bool {
    if password.is_empty() || stored.is_empty() {
        return false;
    }
    if is_scrypt_hash(stored) {
        let parts: Vec<&str> = stored.split('$').collect();
        // format : scrypt , v1 , salt , hash
        if parts.len() != 4 {
            return false;
        }
        let salt = match URL_SAFE_NO_PAD.decode(parts[2]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let _expected = match URL_SAFE_NO_PAD.decode(parts[3]) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let params = match ScryptParams::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, SCRYPT_KEYLEN) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let mut dk = [0u8; SCRYPT_KEYLEN];
        if scrypt(password.as_bytes(), &salt, &params, &mut dk).is_err() {
            return false;
        }
        let actual = URL_SAFE_NO_PAD.encode(dk);
        // Comparaison en temps raisonnablement constant.
        constant_time_eq(actual.as_bytes(), parts[3].as_bytes())
    } else if stored.starts_with("$2") {
        bcrypt::verify(password, stored).unwrap_or(false)
    } else {
        false
    }
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

/// Signe un JWT HS256 avec expiration de 24h (comme Express).
pub fn sign_token(claims: &Claims, secret: &str) -> ApiResult<String> {
    let now = chrono::Utc::now().timestamp();
    let c = Claims {
        iat: now,
        exp: now + 24 * 3600,
        ..claims.clone()
    };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let token = jsonwebtoken::encode(&header, &c, &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| ApiError::internal(format!("Erreur JWT: {e}")))?;
    Ok(token)
}

/// Vérifie un JWT et renvoie les claims. 401 si invalide/expiré.
pub fn verify_token(token: &str, secret: &str) -> ApiResult<Claims> {
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.leeway = 0;
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::unauthorized("Token invalide ou expiré"))?;
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
        assert!(is_scrypt_hash(&h));
        assert!(compare_password("admin123", &h));
        assert!(!compare_password("wrong", &h));
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