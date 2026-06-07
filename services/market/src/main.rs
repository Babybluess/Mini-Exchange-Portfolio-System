mod config;
mod error;
mod handlers;
mod price_store;

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub use config::Config;
pub use error::MarketError;
pub use price_store::PriceStore;

#[derive(Clone)]
pub struct AppState {
    pub prices: Arc<PriceStore>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "market=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    info!("Market Service starting on {}", config.addr());

    let price_store = Arc::new(PriceStore::new());
    price_store.seed_defaults();

    let state = AppState {
        prices: price_store.clone(),
    };

    if let Ok(redis_url) = std::env::var("REDIS_URL") {
        info!("Redis cache enabled at {}", redis_url);
        price_store.connect_redis(redis_url).await?;
    }

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/symbols", get(handlers::get_symbols))
        .route("/prices", get(handlers::get_prices))
        .route("/prices/:symbol", get(handlers::get_price_by_symbol))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(config.addr()).await?;
    info!("Listening on {}", config.addr());
    axum::serve(listener, app).await?;

    Ok(())
}
