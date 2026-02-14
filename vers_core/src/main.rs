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

use vers_shared::VersEvent;

use vers_core::{
    config::AppConfig,
    db,
    events::EventProcessor,
    handlers,
    managers::{AgentManager, PluginManager, SystemHandler},
    AppState,
};

mod plugins;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    info!("+---------------------------------------+");
    info!("|            VERS-SYSTEM Kernel         |");
    info!("|             Version 0.3.3-ASC         |");
    info!("+---------------------------------------+");

    let config = AppConfig::load()?;
    info!(
        "📍 Loaded Config: DB_URL={}, DEFAULT_AGENT={}",
        config.database_url, config.default_agent_id
    );

    // 1. データベースの初期化
    let pool = SqlitePool::connect(&config.database_url).await?;
    db::init_db(&pool, &config.database_url).await?;

    // 2. Plugin Manager Setup
    let mut plugin_manager_mut = PluginManager::new(pool.clone());
    plugin_manager_mut.register_builtins();
    let plugin_manager = Arc::new(plugin_manager_mut);

    // 3. Initialize External Plugins
    let registry = plugin_manager.initialize_all().await?;
    let registry_arc = Arc::new(registry);

    // 4. Managers & Internal Handlers
    let agent_manager = AgentManager::new(pool.clone());
    let (tx, _rx) = broadcast::channel(100);
    let (event_tx, event_rx) = mpsc::channel::<VersEvent>(100);

    // 🔌 System Handler の登録 (Principle #3: Everything is a Handler)
    let system_handler = Arc::new(SystemHandler::new(
        registry_arc.clone(),
        agent_manager.clone(),
        config.default_agent_id.clone(),
    ));
    
    // Registry に内部ハンドラを追加
    registry_arc.add_internal_handler(system_handler).await;

    // 5. App State
    let app_state = Arc::new(AppState {
        tx: tx.clone(),
        registry: registry_arc.clone(),
        event_tx: event_tx.clone(),
        pool: pool.clone(),
        agent_manager: agent_manager.clone(),
        plugin_manager: plugin_manager.clone(),
    });

    // 6. Event Loop
    let processor = EventProcessor::new(registry_arc.clone(), plugin_manager.clone(), tx.clone());

    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        processor.process_loop(event_rx, event_tx_clone).await;
    });

    // 7. Web Server
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
            "/plugins/:id/permissions/grant",
            post(handlers::grant_permission_handler),
        )
        .route(
            "/agents",
            get(handlers::get_agents).post(handlers::create_agent),
        )
        .route(
            "/agents/:id",
            post(handlers::update_agent),
        );

    let mut dynamic_routes = Router::new();
    for (id, plugin) in registry_arc.plugins.iter() {
        if let Some(web) = plugin.as_web() {
            dynamic_routes = web.register_routes(dynamic_routes);
            info!("🔌 Registered dynamic routes for web-enabled plugin: {}", id);
        }
    }

    let dynamic_routes_with_state = dynamic_routes.with_state(app_state.clone() as Arc<dyn std::any::Any + Send + Sync>);
    let app = Router::new()
        .nest("/api", api_routes.with_state(app_state.clone()).merge(dynamic_routes_with_state))
        .nest_service("/", ServeDir::new(config.dashboard_path))
        .layer(
            CorsLayer::new()
                .allow_origin(config.cors_origins)
                .allow_methods([axum::http::Method::GET, ax_http_Method::POST])
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

// Helper to fix potential typo in code above
use axum::http::Method as ax_http_Method;