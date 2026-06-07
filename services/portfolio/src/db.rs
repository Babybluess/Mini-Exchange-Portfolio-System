use chrono::Utc;
use shared::{
    errors::AppError, Holding, Order, OrderStatus, Portfolio,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub async fn get_portfolio(pool: &PgPool, user_id: Uuid) -> Result<Portfolio, AppError> {
    let user = sqlx::query(
        "SELECT id, cash_balance FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    let holdings = sqlx::query(
        "SELECT symbol, quantity, avg_cost FROM holdings WHERE user_id = $1 AND quantity > 0"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| Holding {
        symbol: r.get("symbol"),
        quantity: r.get("quantity"),
        avg_cost: r.get("avg_cost"),
    })
    .collect();

    Ok(Portfolio {
        user_id,
        cash_balance: user.get("cash_balance"),
        holdings,
        updated_at: Utc::now(),
    })
}

pub async fn get_order(pool: &PgPool, order_id: Uuid) -> Result<Order, AppError> {
    let row = sqlx::query(
        r#"SELECT id, user_id, symbol, side, quantity, price, status, reject_reason, created_at
           FROM orders WHERE id = $1"#
    )
    .bind(order_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Order {} not found", order_id)))?;

    Ok(Order {
        id: row.get("id"),
        user_id: row.get("user_id"),
        symbol: row.get("symbol"),
        side: row.get("side"),
        quantity: row.get("quantity"),
        price: row.get("price"),
        status: row.get("status"),
        reject_reason: row.get("reject_reason"),
        created_at: row.get("created_at"),
    })
}

pub async fn insert_order(
    tx: &mut Transaction<'_, Postgres>,
    order: &Order,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO orders (id, user_id, symbol, side, quantity, price, status, reject_reason, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#
    )
    .bind(order.id)
    .bind(order.user_id)
    .bind(&order.symbol)
    .bind(&order.side)
    .bind(order.quantity)
    .bind(order.price)
    .bind(&order.status)
    .bind(&order.reject_reason)
    .bind(order.created_at)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

pub async fn update_order_status(
    tx: &mut Transaction<'_, Postgres>,
    order_id: Uuid,
    status: OrderStatus,
    reject_reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE orders SET status = $1, reject_reason = $2 WHERE id = $3"
    )
    .bind(&status)
    .bind(reject_reason)
    .bind(order_id)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

pub async fn lock_user_balance(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<f64, AppError> {
    let row = sqlx::query(
        "SELECT cash_balance FROM users WHERE id = $1 FOR UPDATE"
    )
    .bind(user_id)
    .fetch_optional(tx.as_mut())
    .await?
    .ok_or_else(|| AppError::NotFound(format!("User {} not found", user_id)))?;

    Ok(row.get("cash_balance"))
}

pub async fn deduct_cash(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    amount: f64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE users SET cash_balance = cash_balance - $1 WHERE id = $2"
    )
    .bind(amount)
    .bind(user_id)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

pub async fn add_cash(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    amount: f64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"
    )
    .bind(amount)
    .bind(user_id)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}

pub async fn get_holding(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    symbol: &str,
) -> Result<Option<f64>, AppError> {
    let row = sqlx::query(
        "SELECT quantity FROM holdings WHERE user_id = $1 AND symbol = $2 FOR UPDATE"
    )
    .bind(user_id)
    .bind(symbol)
    .fetch_optional(tx.as_mut())
    .await?;
    Ok(row.map(|r| r.get("quantity")))
}

pub async fn upsert_holding(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    symbol: &str,
    quantity_delta: f64,
    new_avg_cost: f64,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO holdings (user_id, symbol, quantity, avg_cost)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (user_id, symbol)
           DO UPDATE SET
             quantity = holdings.quantity + EXCLUDED.quantity,
             avg_cost = $4"#
    )
    .bind(user_id)
    .bind(symbol)
    .bind(quantity_delta)
    .bind(new_avg_cost)
    .execute(tx.as_mut())
    .await?;
    Ok(())
}
