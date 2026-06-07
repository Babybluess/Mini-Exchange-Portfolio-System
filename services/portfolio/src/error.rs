use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use shared::errors::AppError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PortfolioError {
    #[error(transparent)]
    App(#[from] AppError),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl IntoResponse for PortfolioError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            PortfolioError::App(AppError::NotFound(_)) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            PortfolioError::App(AppError::InsufficientBalance { .. })
            | PortfolioError::App(AppError::InsufficientHoldings { .. })
            | PortfolioError::App(AppError::InvalidRequest(_)) => {
                (StatusCode::UNPROCESSABLE_ENTITY, self.to_string())
            }
            PortfolioError::App(AppError::MarketServiceUnavailable(_)) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
