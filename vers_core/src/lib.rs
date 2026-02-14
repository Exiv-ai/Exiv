pub mod config;
pub mod db;
pub mod events;
pub mod handlers;
pub mod managers;
pub mod capabilities;
pub mod middleware;

// Re-export audit log and permission request types for external use
pub use db::{
    AuditLogEntry, write_audit_log, query_audit_logs,
    PermissionRequest, create_permission_request, get_pending_permission_requests, update_permission_request,
};

// Static Linker: Force plugin crates to be linked for inventory discovery
// Without these imports, the Rust linker will not include plugin code,
// causing inventory::submit! to never execute and plugins to be undiscoverable.
#[allow(unused_imports)]
use plugin_cerebras;
#[allow(unused_imports)]
use plugin_cursor;
#[allow(unused_imports)]
use plugin_deepseek;
#[allow(unused_imports)]
use plugin_ks22;
#[allow(unused_imports)]
use plugin_mcp;
#[allow(unused_imports)]
use plugin_moderator;
#[allow(unused_imports)]
use plugin_python_bridge;
#[allow(unused_imports)]
use plugin_vision;

use vers_shared::VersEvent;
use sqlx::SqlitePool;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

#[derive(Debug, Clone)]
pub struct EnvelopedEvent {
    pub event: Arc<VersEvent>,
    pub issuer: Option<vers_shared::VersId>, // None = System/Kernel
    pub correlation_id: Option<vers_shared::VersId>, // 親イベントの trace_id
    pub depth: u8,
}

pub struct DynamicRouter {
    pub router: RwLock<axum::Router<Arc<dyn std::any::Any + Send + Sync>>>,
}

pub struct AppState {
    pub tx: broadcast::Sender<Arc<VersEvent>>,
    pub registry: Arc<managers::PluginRegistry>,
    pub event_tx: mpsc::Sender<EnvelopedEvent>,
    pub pool: SqlitePool,
    pub agent_manager: managers::AgentManager,
    pub plugin_manager: Arc<managers::PluginManager>,
    pub dynamic_router: Arc<DynamicRouter>,
    pub config: config::AppConfig,
    pub event_history: Arc<RwLock<VecDeque<Arc<VersEvent>>>>,
    pub metrics: Arc<managers::SystemMetrics>,
    pub rate_limiter: Arc<middleware::RateLimiter>,
}

pub enum AppError {
    Vers(vers_shared::VersError),
    Internal(anyhow::Error),
    NotFound(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, err_type, message) = match self {
            AppError::Vers(e) => (axum::http::StatusCode::BAD_REQUEST, format!("{:?}", e), e.to_string()),
            AppError::Internal(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "InternalError".to_string(), e.to_string()),
            AppError::NotFound(m) => (axum::http::StatusCode::NOT_FOUND, "NotFound".to_string(), m),
        };

        let body = axum::Json(serde_json::json!({
            "status": "error",
            "error": {
                "type": err_type,
                "message": message
            }
        }));

        (status, body).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err)
    }
}

impl From<vers_shared::VersError> for AppError {
    fn from(err: vers_shared::VersError) -> Self {
        AppError::Vers(err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Internal(anyhow::anyhow!(err))
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Kernel 起動用のエントリポイント
pub async fn run_kernel() -> anyhow::Result<()> {
    use crate::config::AppConfig;
    use crate::db;
    use crate::events::EventProcessor;
    use crate::handlers::{self, system::SystemHandler};
    use crate::managers::{AgentManager, PluginManager};
    use axum::{routing::{get, post, any}, Router};
    use tower_http::cors::CorsLayer;
    use tracing::info;

    info!("+---------------------------------------+");
    info!("|            VERS-SYSTEM Kernel         |");
    info!("|             Version {:<10}      |", env!("CARGO_PKG_VERSION"));
    info!("+---------------------------------------+");

    let config = AppConfig::load()?;
    info!(
        "📍 Loaded Config: DB_URL={}, DEFAULT_AGENT={}",
        config.database_url, config.default_agent_id
    );

    // 1. データベースの初期化
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;
    let opts = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(opts).await?;
    db::init_db(&pool, &config.database_url).await?;

    // 2. Plugin Manager Setup
    let mut plugin_manager_obj = PluginManager::new(
        pool.clone(),
        config.allowed_hosts.clone(),
        config.plugin_event_timeout_secs,
        config.max_event_depth,
    );
    plugin_manager_obj.register_builtins();

    // 3. Channel Setup
    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<EnvelopedEvent>(100);
    plugin_manager_obj.set_event_tx(event_tx.clone());
    let plugin_manager = Arc::new(plugin_manager_obj);

    // 4. Initialize External Plugins
    let registry = plugin_manager.initialize_all().await?;
    let registry_arc = Arc::new(registry);

    // 5. Managers & Internal Handlers
    let agent_manager = AgentManager::new(pool.clone());
    let (tx, _rx) = tokio::sync::broadcast::channel(100);

    let mut dynamic_routes = Router::new();
    let plugins_snapshot = registry_arc.plugins.read().await;
    for (id, plugin) in plugins_snapshot.iter() {
        if let Some(web) = plugin.as_web() {
            dynamic_routes = web.register_routes(dynamic_routes);
            info!("🔌 Registered dynamic routes for web-enabled plugin: {}", id);
        }
    }
    drop(plugins_snapshot);

    let dynamic_router = Arc::new(DynamicRouter {
        router: tokio::sync::RwLock::new(dynamic_routes),
    });

    let metrics = Arc::new(managers::SystemMetrics::new());
    let event_history = Arc::new(tokio::sync::RwLock::new(VecDeque::new()));

    // 🔌 System Handler の登録
    let system_handler = Arc::new(SystemHandler::new(
        registry_arc.clone(),
        agent_manager.clone(),
        config.default_agent_id.clone(),
        event_tx.clone(),
        config.memory_context_limit,
        metrics.clone(),
        config.consensus_engines.clone(),
    ));
    
    {
        let mut plugins = registry_arc.plugins.write().await;
        plugins.insert("core.system".to_string(), system_handler);
    }

    // 5. Rate Limiter & App State
    let rate_limiter = Arc::new(middleware::RateLimiter::new(10, 20));

    let app_state = Arc::new(AppState {
        tx: tx.clone(),
        registry: registry_arc.clone(),
        event_tx: event_tx.clone(),
        pool: pool.clone(),
        agent_manager: agent_manager.clone(),
        plugin_manager: plugin_manager.clone(),
        dynamic_router: dynamic_router.clone(),
        config: config.clone(),
        event_history: event_history.clone(),
        metrics: metrics.clone(),
        rate_limiter,
    });

    // 6. Event Loop
    let processor = EventProcessor::new(
        registry_arc.clone(),
        plugin_manager.clone(),
        tx.clone(),
        dynamic_router.clone(),
        event_history,
        metrics,
    );

    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        processor.process_loop(event_rx, event_tx_clone).await;
    });

    // 7. Web Server

    // Admin endpoints: rate-limited (10 req/s, burst 20)
    let admin_routes = Router::new()
        .route("/system/shutdown", post(handlers::shutdown_handler))
        .route("/plugins/apply", post(handlers::apply_plugin_settings))
        .route("/plugins/:id/config", post(handlers::update_plugin_config))
        .route("/plugins/:id/permissions/grant", post(handlers::grant_permission_handler))
        .route("/agents", post(handlers::create_agent))
        .route("/agents/:id", post(handlers::update_agent))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::rate_limit_middleware,
        ));

    // Public/read endpoints (no rate limiting)
    let api_routes = Router::new()
        .route("/events", get(handlers::sse_handler))
        .route("/events/publish", post(handlers::post_event_handler))
        .route("/history", get(handlers::get_history))
        .route("/metrics", get(handlers::get_metrics))
        .route("/memories", get(handlers::get_memories))
        .route("/chat", post(handlers::chat_handler))
        .route("/plugins", get(handlers::get_plugins))
        .route("/plugins/:id/config", get(handlers::get_plugin_config))
        .route("/agents", get(handlers::get_agents))
        .route("/permissions/pending", get(handlers::get_pending_permissions))
        .route("/permissions/:id/approve", post(handlers::approve_permission))
        .route("/permissions/:id/deny", post(handlers::deny_permission))
        .merge(admin_routes);

    let app = Router::new()
        .nest("/api", api_routes.with_state(app_state.clone()))
        .route("/api/plugin/*path", any(dynamic_proxy_handler))
        .with_state(app_state.clone())
        .fallback(handlers::assets::static_handler)
        .layer(
            CorsLayer::new()
                .allow_origin(config.cors_origins)
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers([axum::http::header::CONTENT_TYPE]),
        );

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    info!(
        "🚀 VERS-SYSTEM Kernel is listening on http://0.0.0.0:{}",
        config.port
    );

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    Ok(())
}

use axum::extract::State;
use axum::http::Request;
use axum::response::IntoResponse;
use tower::ServiceExt;

async fn dynamic_proxy_handler(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
) -> impl IntoResponse {
    let router = {
        let router_lock = state.dynamic_router.router.read().await;
        router_lock.clone()
    };

    let any_state = state.clone() as Arc<dyn std::any::Any + Send + Sync>;
    router
        .with_state(any_state)
        .oneshot(request)
        .await
        .into_response()
}
