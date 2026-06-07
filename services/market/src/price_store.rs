use chrono::Utc;
use rand::Rng;
use redis::AsyncCommands;
use shared::{Price, Symbol};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::Mutex;
use tracing::warn;

const PRICE_CACHE_TTL_SECS: u64 = 5;

static SYMBOLS: &[(&str, &str, f64)] = &[
    ("BTC", "Bitcoin", 65_000.0),
    ("ETH", "Ethereum", 3_500.0),
    ("SOL", "Solana", 180.0),
    ("BNB", "BNB", 600.0),
    ("USDT", "Tether", 1.0),
];

pub struct PriceStore {
    base: RwLock<HashMap<String, f64>>,
    redis: Mutex<Option<redis::aio::MultiplexedConnection>>,
}

impl PriceStore {
    pub fn new() -> Self {
        Self {
            base: RwLock::new(HashMap::new()),
            redis: Mutex::new(None),
        }
    }

    pub fn seed_defaults(&self) {
        let mut map = self.base.write().unwrap();
        for (sym, _, price) in SYMBOLS {
            map.insert(sym.to_string(), *price);
        }
    }

    pub async fn connect_redis(&self, url: String) -> anyhow::Result<()> {
        let client = redis::Client::open(url)?;
        let conn = client.get_multiplexed_async_connection().await?;
        *self.redis.lock().await = Some(conn);
        Ok(())
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        SYMBOLS
            .iter()
            .map(|(sym, name, _)| Symbol {
                symbol: sym.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    pub async fn get_price(&self, symbol: &str) -> Option<Price> {
        if let Some(conn) = self.redis.lock().await.as_mut() {
            let key = format!("price:{}", symbol);
            let cached: redis::RedisResult<f64> = conn.get(&key).await;
            if let Ok(price) = cached {
                return Some(Price {
                    symbol: symbol.to_string(),
                    price,
                    timestamp: Utc::now(),
                });
            }
        }

        let price = {
            let mut map = self.base.write().ok()?;
            let base = map.get_mut(symbol)?;
            let drift = rand::thread_rng().gen_range(-0.002..0.002);
            *base *= 1.0 + drift;
            *base
        };

        if let Some(conn) = self.redis.lock().await.as_mut() {
            let key = format!("price:{}", symbol);
            let _: redis::RedisResult<()> = conn
                .set_ex(&key, price, PRICE_CACHE_TTL_SECS)
                .await;
        }

        Some(Price {
            symbol: symbol.to_string(),
            price,
            timestamp: Utc::now(),
        })
    }

    pub async fn get_all_prices(&self) -> Vec<Price> {
        let symbols: Vec<String> = {
            self.base.read().unwrap().keys().cloned().collect()
        };
        let mut result = Vec::with_capacity(symbols.len());
        for sym in symbols {
            if let Some(p) = self.get_price(&sym).await {
                result.push(p);
            }
        }
        result
    }
}
