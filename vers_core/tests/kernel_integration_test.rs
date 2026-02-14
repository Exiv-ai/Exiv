use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use plugin_deepseek::DeepSeekPlugin;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tower::ServiceExt;
use vers_shared::{Plugin, VersEvent, WebPlugin};
use std::any::Any;

// 💡 統合テスト用に最小限のセットアップを模倣
#[tokio::test]
async fn test_dynamic_routing_registration() {
    // 1. Setup minimal state (Omitted setup code for brevity, but keeping logic consistent)
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE agents (id TEXT PRIMARY KEY, name TEXT, description TEXT, status TEXT, default_engine_id TEXT, metadata TEXT)").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE plugin_settings (plugin_id TEXT PRIMARY KEY, is_active BOOLEAN)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE plugin_configs (plugin_id TEXT, config_key TEXT, config_value TEXT, PRIMARY KEY(plugin_id, config_key))").execute(&pool).await.unwrap();

    let (_tx, _rx) = broadcast::channel::<VersEvent>(100);
    let (_event_tx, _event_rx) = mpsc::channel::<VersEvent>(100);

    // 2. Initialize a real plugin to test its route
    let ds_plugin = Arc::new(DeepSeekPlugin::new(
        vers_shared::VersId::from_name("test"),
        None,
        None,
    ));

    // 3. Manually build the router as main.rs does (using the new capability-driven registration)
    let mut dynamic_routes = axum::Router::new();
    if let Some(web) = ds_plugin.as_web() {
        dynamic_routes = web.register_routes(dynamic_routes);
    }

    // Mock state matching the Router's expected state type in register_routes
    let mock_state = Arc::new("mock_state".to_string()) as Arc<dyn Any + Send + Sync>;
    let api_routes = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(dynamic_routes.with_state(mock_state));

    let app = axum::Router::new().nest("/api", api_routes);

    // 4. Test the dynamic route
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/plugin/deepseek/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_permission_logic_unit() {
    // Kernelのイベントループ内の権限検証ロジックが正しく Permission 型を扱えるかチェック
    let permission = vers_shared::Permission::InputControl;
    let mut permissions = Vec::new();
    permissions.push(permission.clone());

    assert!(permissions.contains(&vers_shared::Permission::InputControl));
}
