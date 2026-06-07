use axum::{extract::{Path, State}, Json};
use serde_json::{json, Value};

use crate::{AppState, MarketError};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "market" }))
}

pub async fn get_symbols(State(state): State<AppState>) -> Json<Value> {
    let symbols = state.prices.symbols();
    Json(json!({ "symbols": symbols }))
}

pub async fn get_prices(State(state): State<AppState>) -> Json<Value> {
    let prices = state.prices.get_all_prices().await;
    Json(json!({ "prices": prices }))
}

pub async fn get_price_by_symbol(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<Value>, MarketError> {
    let sym = symbol.to_uppercase();
    match state.prices.get_price(&sym).await {
        Some(price) => Ok(Json(json!({ "price": price }))),
        None => Err(MarketError::SymbolNotFound(sym)),
    }
}
