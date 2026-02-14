use axum::{
    routing::{get, post},
    Router,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::info;

use plugin_cerebras::CerebrasPlugin;
use plugin_cursor::CursorPlugin;
use plugin_deepseek::DeepSeekPlugin;
use plugin_ks2_2::Ks2_2Plugin;
use vers_shared::VersEvent;

mod config;
mod db;
mod events;
mod handlers;
mod managers;
mod capabilities;

use config::AppConfig;
use events::EventProcessor;
use managers::{AgentManager, MessageRouter, PluginManager};

pub struct AppState {
    pub tx: broadcast::Sender<VersEvent>,
    pub registry: Arc<managers::PluginRegistry>,
    pub event_tx: mpsc::Sender<VersEvent>,
    pub pool: SqlitePool,
    pub agent_manager: AgentManager,
    pub plugin_manager: Arc<PluginManager>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    info!("+---------------------------------------+");
    info!("|            VERS-SYSTEM Kernel         |");
    info!("|             Version 0.3.3             |");
    info!("+---------------------------------------+");

    let config = AppConfig::load()?;
    info!(
        "📍 Loaded Config: DB_URL={}, DASHBOARD={:?}",
        config.database_url, config.dashboard_path
    );

    // 1. データベースの初期化
    let pool = SqlitePool::connect(&config.database_url).await?;
    db::init_db(&pool, &config.database_url).await?;

    // 2. Plugin Manager Setup (Bootstrap)
    let mut plugin_manager_mut = PluginManager::new(pool.clone());

    // Register Factories
    plugin_manager_mut.register_factory(Ks2_2Plugin::factory());
    plugin_manager_mut.register_factory(DeepSeekPlugin::factory());
    plugin_manager_mut.register_factory(CerebrasPlugin::factory());
    plugin_manager_mut.register_factory(CursorPlugin::factory());

    let plugin_manager = Arc::new(plugin_manager_mut);

    // Initialize Plugins
    let registry = Arc::new(plugin_manager.initialize_all().await?);

    // 3. Managers
    let agent_manager = AgentManager::new(pool.clone());
    let (tx, _rx) = broadcast::channel(100);
    let (event_tx, event_rx) = mpsc::channel::<VersEvent>(100);
    let router = MessageRouter::new(registry.clone(), agent_manager.clone(), event_tx.clone());

    // 4. App State
    let app_state = Arc::new(AppState {
        tx: tx.clone(),
        registry: registry.clone(),
        event_tx: event_tx.clone(),
        pool: pool.clone(),
        agent_manager,
        plugin_manager,
    });

    // 5. Event Loop
    let processor = EventProcessor::new(registry.clone(), Arc::new(router), tx.clone());

    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        processor.process_loop(event_rx, event_tx_clone).await;
    });

    // 6. Web Server
    let api_routes = Router::new()
        .route("/events", get(handlers::sse_handler))
        .route("/chat", post(handlers::chat_handler))
        .route("/plugins", get(handlers::get_plugins))
        .route("/plugins/apply", post(handlers::apply_plugin_settings))
        .route(
            "/plugins/:id/config",
            get(handlers::get_plugin_config).post(handlers::update_plugin_config),
        )
        .route(
            "/agents",
            get(handlers::get_agents).post(handlers::create_agent),
        );

    // 🔌 プラグイン固有のルートを登録 (Capability-Driven)
    let mut dynamic_routes = Router::new();
    for (id, plugin) in registry.plugins.iter() {
        if let Some(web) = plugin.as_web() {
            dynamic_routes = web.register_routes(dynamic_routes);
            info!("🔌 Registered dynamic routes for web-enabled plugin: {}", id);
        }
    }

    // AppState を Any にキャストしてプラグイン用ルートに提供し、コアAPIとマージ
    let dynamic_routes_with_state = dynamic_routes.with_state(app_state.clone() as Arc<dyn std::any::Any + Send + Sync>);
    let app = Router::new()
        .nest("/api", api_routes.with_state(app_state.clone()).merge(dynamic_routes_with_state))
        .nest_service("/", ServeDir::new(config.dashboard_path))
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
    axum::serve(listener, app).await?;
    Ok(())
}
