use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Insufficient balance: required {required:.2}, available {available:.2}")]
    InsufficientBalance { required: f64, available: f64 },

    #[error("Insufficient holdings: required {required:.4} {symbol}, available {available:.4}")]
    InsufficientHoldings {
        symbol: String,
        required: f64,
        available: f64,
    },

    #[error("Market service unavailable: {0}")]
    MarketServiceUnavailable(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
