use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarketError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for MarketError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            MarketError::SymbolNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            MarketError::Redis(_) | MarketError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
