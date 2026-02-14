use anyhow::Context;
use axum::http::HeaderValue;
use std::env;
use std::path::PathBuf;

/// Returns the directory containing the running executable.
/// Falls back to CWD if the exe path cannot be determined.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub cors_origins: Vec<HeaderValue>,
    pub default_agent_id: String,
    pub allowed_hosts: Vec<String>,
    pub plugin_event_timeout_secs: u64,
    pub max_event_depth: u8,
    pub memory_context_limit: usize,
    pub admin_api_key: Option<String>,
    pub consensus_engines: Vec<String>,
    pub update_repo: String,
    pub event_history_size: usize,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            let db_path = exe_dir().join("data").join("vers_memories.db");
            format!("sqlite:{}", db_path.display())
        });

        let admin_api_key = env::var("VERS_API_KEY").ok();

        let default_agent_id =
            env::var("DEFAULT_AGENT_ID").unwrap_or_else(|_| "agent.karin".to_string());

        let plugin_event_timeout_secs = env::var("PLUGIN_EVENT_TIMEOUT_SECS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u64>()
            .context("Failed to parse PLUGIN_EVENT_TIMEOUT_SECS")?;

        let max_event_depth = env::var("MAX_EVENT_DEPTH")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u8>()
            .context("Failed to parse MAX_EVENT_DEPTH")?;

        let memory_context_limit = env::var("MEMORY_CONTEXT_LIMIT")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<usize>()
            .context("Failed to parse MEMORY_CONTEXT_LIMIT")?;

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

        let allowed_hosts_str = env::var("ALLOWED_HOSTS").unwrap_or_default();
        let allowed_hosts = if allowed_hosts_str.is_empty() {
            vec![]
        } else {
            allowed_hosts_str.split(',').map(|s| s.to_string()).collect()
        };

        let update_repo = env::var("VERS_UPDATE_REPO")
            .unwrap_or_else(|_| "karin-project/vers-system".to_string());

        let consensus_engines_str = env::var("CONSENSUS_ENGINES")
            .unwrap_or_else(|_| "mind.deepseek,mind.cerebras".to_string());
        let consensus_engines = consensus_engines_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let event_history_size = env::var("EVENT_HISTORY_SIZE")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<usize>()
            .context("Failed to parse EVENT_HISTORY_SIZE")?;

        Ok(Self {
            database_url,
            port,
            cors_origins,
            default_agent_id,
            allowed_hosts,
            plugin_event_timeout_secs,
            max_event_depth,
            memory_context_limit,
            admin_api_key,
            consensus_engines,
            update_repo,
            event_history_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to ensure env var tests run serially (prevents parallel test interference)
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // Guard to ensure env var cleanup even on panic
    struct EnvGuard(&'static str);

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(self.0);
        }
    }

    #[test]
    fn test_consensus_engines_parsing() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("CONSENSUS_ENGINES", "mind.deepseek,mind.anthropic");
        let _guard = EnvGuard("CONSENSUS_ENGINES");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.consensus_engines.len(), 2);
        assert_eq!(config.consensus_engines[0], "mind.deepseek");
        assert_eq!(config.consensus_engines[1], "mind.anthropic");
    }

    #[test]
    fn test_consensus_engines_default() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _guard = EnvGuard("CONSENSUS_ENGINES");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.consensus_engines, vec!["mind.deepseek", "mind.cerebras"]);
    }

    #[test]
    fn test_consensus_engines_whitespace_handling() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("CONSENSUS_ENGINES", " mind.deepseek , mind.anthropic , mind.openai ");
        let _guard = EnvGuard("CONSENSUS_ENGINES");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.consensus_engines.len(), 3);
        assert_eq!(config.consensus_engines[0], "mind.deepseek");
        assert_eq!(config.consensus_engines[1], "mind.anthropic");
        assert_eq!(config.consensus_engines[2], "mind.openai");
    }
}
