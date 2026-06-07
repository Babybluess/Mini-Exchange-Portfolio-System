use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use shared::models::CreateOrderRequest;
use uuid::Uuid;

use crate::{order, AppState, PortfolioError};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "portfolio" }))
}

pub async fn get_portfolio(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Value>, PortfolioError> {
    let portfolio = crate::db::get_portfolio(&state.db, user_id).await?;
    Ok(Json(json!({ "portfolio": portfolio })))
}

pub async fn create_order(
    State(state): State<AppState>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<Json<Value>, PortfolioError> {
    let executed = order::execute_order(
        &state.db,
        &state.market,
        &state.event_tx,
        req,
    )
    .await?;
    Ok(Json(json!({ "order": executed })))
}

pub async fn get_order(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Value>, PortfolioError> {
    let order = crate::db::get_order(&state.db, order_id).await?;
    Ok(Json(json!({ "order": order })))
}
