use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AuditEvent {
    OrderCreated(OrderEventPayload),
    OrderExecuted(OrderEventPayload),
    OrderRejected(OrderRejectedPayload),
}

impl AuditEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::OrderCreated(_) => "ORDER_CREATED",
            AuditEvent::OrderExecuted(_) => "ORDER_EXECUTED",
            AuditEvent::OrderRejected(_) => "ORDER_REJECTED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEventPayload {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRejectedPayload {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub reason: String,
}
