mod config;
mod handlers;

use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "audit=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    info!("Audit Service starting on {}", config.addr());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("../../migrations").run(&pool).await?;

    let state = AppState { db: pool.clone() };

    // Spawn Redis subscriber task
    if let Some(redis_url) = config.redis_url.clone() {
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            redis_subscriber(redis_url, pool_clone).await;
        });
    } else {
        warn!("No REDIS_URL — audit service will not receive events");
    }

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/audit", get(handlers::list_audit_logs))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.addr()).await?;
    info!("Listening on {}", config.addr());
    axum::serve(listener, app).await?;

    Ok(())
}

async fn redis_subscriber(redis_url: String, pool: sqlx::PgPool) {
    let client = match redis::Client::open(redis_url.clone()) {
        Ok(c) => c,
        Err(e) => {
            error!("Redis client error: {}", e);
            return;
        }
    };

    loop {
        info!("Connecting to Redis Pub/Sub at {}", redis_url);
        let mut pubsub = match client.get_async_pubsub().await {
            Ok(c) => c,
            Err(e) => {
                warn!("Redis pubsub connect failed: {}; retrying in 5s", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        if let Err(e) = pubsub.subscribe("audit_events").await {
            warn!("Subscribe failed: {}", e);
            continue;
        }

        info!("Subscribed to audit_events channel");

        use futures_util::StreamExt;
        let mut stream = pubsub.on_message();

        while let Some(msg) = stream.next().await {
            let payload: String = match msg.get_payload() {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to decode message: {}", e);
                    continue;
                }
            };

            let value: serde_json::Value = match serde_json::from_str(&payload) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to parse event JSON: {}", e);
                    continue;
                }
            };

            let event_type = value["type"].as_str().unwrap_or("UNKNOWN").to_string();
            let id = uuid::Uuid::new_v4();

            if let Err(e) = sqlx::query(
                "INSERT INTO audit_log (id, event_type, payload, created_at) VALUES ($1, $2, $3, NOW())"
            )
            .bind(id)
            .bind(&event_type)
            .bind(&value["data"])
            .execute(&pool)
            .await
            {
                error!("Failed to persist audit event: {}", e);
            } else {
                info!("Audit event persisted: {}", event_type);
            }
        }

        warn!("Redis pubsub stream ended; reconnecting...");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
