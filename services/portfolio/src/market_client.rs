use reqwest::Client;
use shared::{errors::AppError, Price};
use std::time::Duration;
use tracing::warn;

pub struct MarketClient {
    client: Client,
    base_url: String,
}

impl MarketClient {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn get_price(&self, symbol: &str) -> Result<Price, AppError> {
        let url = format!("{}/prices/{}", self.base_url, symbol);
        let resp = self.client.get(&url).send().await.map_err(|e| {
            warn!("Market service unreachable: {}", e);
            AppError::MarketServiceUnavailable(e.to_string())
        })?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!("Symbol {} not found", symbol)));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            warn!("Market service error: {}", status);
            return Err(AppError::MarketServiceUnavailable(
                format!("Market returned status {}", status),
            ));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            AppError::MarketServiceUnavailable(format!("Invalid response: {}", e))
        })?;

        let price = body["price"]["price"]
            .as_f64()
            .ok_or_else(|| AppError::MarketServiceUnavailable("Missing price field".into()))?;

        Ok(Price {
            symbol: symbol.to_string(),
            price,
            timestamp: chrono::Utc::now(),
        })
    }
}
