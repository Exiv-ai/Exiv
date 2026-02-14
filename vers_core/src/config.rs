use anyhow::Context;
use axum::http::HeaderValue;
use std::env;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub dashboard_path: PathBuf,
    pub port: u16,
    pub cors_origins: Vec<HeaderValue>,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:./vers_memories.db".to_string());

        let dashboard_path_str =
            env::var("VERS_DASHBOARD_PATH").unwrap_or_else(|_| "./vers_dashboard/dist".to_string());
        let dashboard_path = PathBuf::from(dashboard_path_str);

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8081".to_string())
            .parse::<u16>()
            .context("Failed to parse PORT environment variable")?;

        let cors_origins_str = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string());

        let cors_origins = cors_origins_str
            .split(',')
            .map(|s| s.parse().context("Invalid CORS origin URL"))
            .collect::<anyhow::Result<Vec<HeaderValue>>>()?;

        Ok(Self {
            database_url,
            dashboard_path,
            port,
            cors_origins,
        })
    }
}
