use axum::{extract::State, Json};
use serde_json::{json, Value};
use sqlx::Row;

use crate::AppState;

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "audit" }))
}

pub async fn list_audit_logs(
    State(state): State<AppState>,
) -> Json<Value> {
    let rows = sqlx::query(
        "SELECT id, event_type, payload, created_at FROM audit_log ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let logs: Vec<Value> = rows.into_iter().map(|r| json!({
        "id": r.get::<uuid::Uuid, _>("id"),
        "event_type": r.get::<String, _>("event_type"),
        "payload": r.get::<serde_json::Value, _>("payload"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
    })).collect();

    Json(json!({ "audit_logs": logs }))
}
