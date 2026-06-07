use chrono::Utc;
use shared::{
    errors::AppError,
    events::{AuditEvent, OrderEventPayload, OrderRejectedPayload},
    models::{CreateOrderRequest, Order, OrderSide, OrderStatus},
};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{db, MarketClient};

pub async fn execute_order(
    pool: &PgPool,
    market: &MarketClient,
    event_tx: &broadcast::Sender<AuditEvent>,
    req: CreateOrderRequest,
) -> Result<Order, AppError> {
    if req.quantity <= 0.0 {
        return Err(AppError::InvalidRequest(
            "Quantity must be positive".into(),
        ));
    }

    let price_data = market.get_price(&req.symbol).await?;
    let price = price_data.price;
    let total_cost = req.quantity * price;

    let order = Order {
        id: Uuid::new_v4(),
        user_id: req.user_id,
        symbol: req.symbol.clone(),
        side: req.side.clone(),
        quantity: req.quantity,
        price,
        status: OrderStatus::Pending,
        reject_reason: None,
        created_at: Utc::now(),
    };

    emit_event(
        event_tx,
        AuditEvent::OrderCreated(OrderEventPayload {
            order_id: order.id,
            user_id: order.user_id,
            symbol: order.symbol.clone(),
            side: format!("{:?}", order.side),
            quantity: order.quantity,
            price,
        }),
    );

    let mut tx = pool.begin().await?;

    db::insert_order(&mut tx, &order).await?;

    let result = match req.side {
        OrderSide::Buy => execute_buy(&mut tx, &order, total_cost).await,
        OrderSide::Sell => execute_sell(&mut tx, &order).await,
    };

    match result {
        Ok(()) => {
            db::update_order_status(&mut tx, order.id, OrderStatus::Executed, None).await?;
            tx.commit().await?;
            info!("Order {} EXECUTED: {} {} {} @ {}", order.id, format!("{:?}", order.side), order.quantity, order.symbol, price);

            emit_event(
                event_tx,
                AuditEvent::OrderExecuted(OrderEventPayload {
                    order_id: order.id,
                    user_id: order.user_id,
                    symbol: order.symbol.clone(),
                    side: format!("{:?}", order.side),
                    quantity: order.quantity,
                    price,
                }),
            );

            Ok(Order { status: OrderStatus::Executed, ..order })
        }
        Err(ref app_err) => {
            let reason = app_err.to_string();
            warn!("Order {} REJECTED: {}", order.id, reason);

            db::update_order_status(
                &mut tx,
                order.id,
                OrderStatus::Rejected,
                Some(&reason),
            )
            .await?;
            tx.commit().await?;

            emit_event(
                event_tx,
                AuditEvent::OrderRejected(OrderRejectedPayload {
                    order_id: order.id,
                    user_id: order.user_id,
                    symbol: order.symbol.clone(),
                    side: format!("{:?}", order.side),
                    quantity: order.quantity,
                    reason: reason.clone(),
                }),
            );

            Ok(Order {
                status: OrderStatus::Rejected,
                reject_reason: Some(reason),
                ..order
            })
        }
    }
}

async fn execute_buy(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: &Order,
    total_cost: f64,
) -> Result<(), AppError> {
    let balance = db::lock_user_balance(tx, order.user_id).await?;

    if balance < total_cost {
        return Err(AppError::InsufficientBalance {
            required: total_cost,
            available: balance,
        });
    }

    db::deduct_cash(tx, order.user_id, total_cost).await?;
    db::upsert_holding(tx, order.user_id, &order.symbol, order.quantity, order.price).await?;

    Ok(())
}

async fn execute_sell(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: &Order,
) -> Result<(), AppError> {
    let holding = db::get_holding(tx, order.user_id, &order.symbol).await?;
    let qty_held = holding.unwrap_or(0.0);

    if qty_held < order.quantity {
        return Err(AppError::InsufficientHoldings {
            symbol: order.symbol.clone(),
            required: order.quantity,
            available: qty_held,
        });
    }

    let proceeds = order.quantity * order.price;
    db::upsert_holding(tx, order.user_id, &order.symbol, -order.quantity, order.price).await?;
    db::add_cash(tx, order.user_id, proceeds).await?;

    Ok(())
}

fn emit_event(tx: &broadcast::Sender<AuditEvent>, event: AuditEvent) {
    if let Err(e) = tx.send(event) {
        tracing::debug!("No audit receivers: {}", e);
    }
}

pub async fn event_forwarder(
    mut rx: broadcast::Sender<AuditEvent>,
    redis_url: Option<String>,
) {
    let mut receiver = rx.subscribe();

    let redis_conn: Option<redis::aio::MultiplexedConnection> = match redis_url {
        Some(url) => match redis::Client::open(url) {
            Ok(client) => match client.get_multiplexed_async_connection().await {
                Ok(conn) => {
                    info!("Audit event forwarder connected to Redis");
                    Some(conn)
                }
                Err(e) => {
                    warn!("Redis connection failed for event forwarder: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Redis client error: {}", e);
                None
            }
        },
        None => {
            info!("No REDIS_URL — audit events will not be forwarded");
            None
        }
    };

    let mut conn = redis_conn;

    loop {
        match receiver.recv().await {
            Ok(event) => {
                let channel = "audit_events";
                let payload = serde_json::json!({
                    "type": event.event_type(),
                    "data": serde_json::to_value(&event).unwrap_or_default(),
                });
                if let Some(ref mut c) = conn {
                    use redis::AsyncCommands;
                    let _: redis::RedisResult<i64> = c
                        .publish(channel, payload.to_string())
                        .await;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("Audit event forwarder lagged by {} messages", n);
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("Audit event channel closed; forwarder exiting");
                break;
            }
        }
    }
}
