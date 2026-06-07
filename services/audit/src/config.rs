pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8082),
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            redis_url: std::env::var("REDIS_URL").ok(),
        }
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
