mod config;
mod db;
mod error;
mod handlers;
mod market_client;
mod order;

use axum::{routing::{get, post}, Router};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use config::Config;
pub use error::PortfolioError;
pub use market_client::MarketClient;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub market: Arc<MarketClient>,
    pub event_tx: tokio::sync::broadcast::Sender<shared::AuditEvent>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "portfolio=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    info!("Portfolio Service starting on {}", config.addr());

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("../../migrations").run(&pool).await?;
    info!("Migrations applied");

    let market = Arc::new(MarketClient::new(&config.market_url));

    let (event_tx, _) = tokio::sync::broadcast::channel::<shared::AuditEvent>(256);

    let event_tx_clone = event_tx.clone();
    let redis_url = config.redis_url.clone();
    tokio::spawn(async move {
        crate::order::event_forwarder(event_tx_clone, redis_url).await;
    });

    let state = AppState { db: pool, market, event_tx };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/portfolio/:user_id", get(handlers::get_portfolio))
        .route("/orders", post(handlers::create_order))
        .route("/orders/:order_id", get(handlers::get_order))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.addr()).await?;
    info!("Listening on {}", config.addr());
    axum::serve(listener, app).await?;

    Ok(())
}
