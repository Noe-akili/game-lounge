use serde::Serialize;

/// Erreur API renvoyée au frontend, au format `{ message: "...", status: N }`
/// (message identique au backend Express).
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub message: String,
    pub status: u16,
}

impl ApiError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        ApiError {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(500, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503, message)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        ApiError::internal(format!("Erreur base de données: {e}"))
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::bad_request(format!("JSON invalide: {e}"))
    }
}

/// Alias pratique pour les retours de commandes Tauri.
pub type ApiResult<T = serde_json::Value> = Result<T, ApiError>;

/// Convertit une valeur JS en i64 (les JSON numbers arrivent comme i64/f64/u64).
pub fn to_i64(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_u64().map(|u| u as i64)),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Convertit une valeur JS en String.
pub fn to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Convertit une valeur JS en booléen (0/1/true/false).
pub fn to_bool(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_i64().map(|i| i != 0).or_else(|| n.as_u64().map(|u| u != 0)).unwrap_or(false),
        serde_json::Value::String(s) => s == "true" || s == "1",
        _ => false,
    }
}