pub mod system;
pub mod assets;
pub mod update;

use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
    http::HeaderMap,
};
use futures::stream::Stream;
use serde::Deserialize;
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};
use tracing::{info, error};
use exiv_shared::ExivMessage;

use crate::{AppState, AppResult, AppError};

fn check_auth(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    use subtle::ConstantTimeEq;
    match state.config.admin_api_key {
        Some(ref required_key) => {
            let auth_header = headers.get("X-API-Key")
                .and_then(|h| h.to_str().ok());

            let matches: bool = match auth_header {
                Some(provided) => provided.as_bytes().ct_eq(required_key.as_bytes()).into(),
                None => false,
            };
            if !matches {
                return Err(AppError::Vers(exiv_shared::ExivError::PermissionDenied(
                    exiv_shared::Permission::AdminAccess
                )));
            }
        }
        None => {
            // In release builds, require API key to be configured
            if !cfg!(debug_assertions) {
                return Err(AppError::Vers(exiv_shared::ExivError::PermissionDenied(
                    exiv_shared::Permission::AdminAccess
                )));
            }
            // M-09: Warn in debug builds when no API key is set
            tracing::warn!("Admin API access without authentication (debug mode, no API key configured)");
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct PluginToggleRequest {
    pub id: String,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: String,
    pub default_engine: String,
    pub metadata: Option<HashMap<String, String>>,
    pub required_capabilities: Option<Vec<exiv_shared::CapabilityType>>,
}

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct UpdateAgentRequest {
    pub default_engine_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Get list of all agents
pub async fn get_agents(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let agents = state.agent_manager.list_agents().await?;
    Ok(Json(serde_json::json!(agents)))
}

/// Create agent
pub async fn create_agent(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CreateAgentRequest>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;

    // M-07: Input validation
    if payload.name.is_empty() || payload.name.len() > 200 {
        return Err(AppError::Vers(exiv_shared::ExivError::ValidationError(
            "Agent name must be 1-200 characters".to_string(),
        )));
    }
    if payload.description.len() > 1000 {
        return Err(AppError::Vers(exiv_shared::ExivError::ValidationError(
            "Description must be at most 1000 characters".to_string(),
        )));
    }

    let agent_id = state
        .agent_manager
        .create_agent(
            &payload.name,
            &payload.description,
            &payload.default_engine,
            payload.metadata.unwrap_or_default(),
            payload.required_capabilities.unwrap_or_else(|| vec![
                exiv_shared::CapabilityType::Reasoning,
                exiv_shared::CapabilityType::Memory
            ]),
        )
        .await?;
    Ok(Json(serde_json::json!({ "status": "success", "id": agent_id })))
}

/// Update agent settings
pub async fn update_agent(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAgentRequest>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    state.agent_manager.update_agent_config(&id, payload.default_engine_id, payload.metadata).await?;
    Ok(Json(serde_json::json!({ "status": "success" })))
}

/// Get list of all plugins (reflecting DB settings)
pub async fn get_plugins(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let manifests = state.plugin_manager.list_plugins_with_settings(&state.registry).await?;
    Ok(Json(serde_json::json!(manifests)))
}

/// Get plugin configuration
/// K-04: Requires auth since config may contain API keys
pub async fn get_plugin_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    let config = state.plugin_manager.get_config(&id).await?;
    Ok(Json(serde_json::json!(config)))
}

/// Update plugin configuration
pub async fn update_plugin_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<UpdateConfigPayload>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    state
        .plugin_manager
        .update_config(&id, &payload.key, &payload.value)
        .await?;

    info!(plugin_id = %id, key = %payload.key, "⚙️ Config updated for plugin. Broadcasting update...");

    // Get latest settings and notify
    if let Ok(full_config) = state.plugin_manager.get_config(&id).await {
        let envelope = crate::EnvelopedEvent::system(exiv_shared::ExivEventData::ConfigUpdated {
            plugin_id: id.clone(),
            config: full_config,
        });
        let event = envelope.event.clone();
        // H-04: Log send errors instead of silently ignoring
        if let Err(e) = state.event_tx.send(envelope).await {
            error!("Failed to send config update event: {}", e);
        }

        // 監査ログに記録
        crate::db::spawn_audit_log(state.pool.clone(), crate::db::AuditLogEntry {
            timestamp: chrono::Utc::now(),
            event_type: "CONFIG_UPDATED".to_string(),
            actor_id: Some("admin".to_string()),
            target_id: Some(id.clone()),
            permission: None,
            result: "SUCCESS".to_string(),
            reason: format!("Configuration key '{}' updated", payload.key),
            metadata: Some(serde_json::json!({
                "key": payload.key,
                "value_length": payload.value.len()
            })),
            trace_id: Some(event.trace_id.to_string()),
        });
    }

    Ok(Json(serde_json::json!({ "status": "success" })))
}

/// プラグインの有効/無効状態を一括適用
pub async fn apply_plugin_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Vec<PluginToggleRequest>>,
) -> AppResult<Json<bool>> {
    check_auth(&state, &headers)?;
    info!(
        count = payload.len(),
        "📥 Received plugin settings apply request"
    );

    let settings = payload.into_iter().map(|i| (i.id, i.is_active)).collect();

    state.plugin_manager.apply_settings(settings).await?;
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct GrantPermissionRequest {
    pub permission: exiv_shared::Permission,
}

/// プラグインに権限を付与（承認）
pub async fn grant_permission_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<GrantPermissionRequest>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    info!(
        plugin_id = %id,
        permission = ?payload.permission,
        "🔐 Granting permission to plugin"
    );

    state.plugin_manager.grant_permission(&id, payload.permission.clone()).await?;

    // イベントループに通知して Capability を注入させる
    let envelope = crate::EnvelopedEvent::system(exiv_shared::ExivEventData::PermissionGranted {
        plugin_id: id.clone(),
        permission: payload.permission.clone(),
    });
    let event = envelope.event.clone();
    // H-04: Log send errors instead of silently ignoring
    if let Err(e) = state.event_tx.send(envelope).await {
        error!("Failed to send permission grant event: {}", e);
    }

    // 監査ログに記録
    crate::db::spawn_audit_log(state.pool.clone(), crate::db::AuditLogEntry {
        timestamp: chrono::Utc::now(),
        event_type: "PERMISSION_GRANTED".to_string(),
        actor_id: Some("admin".to_string()),
        target_id: Some(id.clone()),
        permission: Some(format!("{:?}", payload.permission)),
        result: "SUCCESS".to_string(),
        reason: "Administrator approved permission request".to_string(),
        metadata: None,
        trace_id: Some(event.trace_id.to_string()),
    });

    Ok(Json(serde_json::json!({ "status": "success" })))
}

/// システムを正常終了させる（ガーディアンによる再起動用）
pub async fn shutdown_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    
    info!("🛑 Shutdown requested. Broadcasting notification...");

    // Send system notification
    let envelope = crate::EnvelopedEvent::system(exiv_shared::ExivEventData::SystemNotification(
        "Kernel is shutting down for maintenance...".to_string()
    ));
    // H-04: Log send errors instead of silently ignoring
    if let Err(e) = state.event_tx.send(envelope).await {
        error!("Failed to send shutdown notification event: {}", e);
    }

    // Execute shutdown asynchronously (after returning response)
    let shutdown = state.shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;

        // 🚧 Signal maintenance mode (exe directory for deployed layout)
        let maint = crate::config::exe_dir().join(".maintenance");
        if let Err(e) = std::fs::write(&maint, "active") {
            error!("❌ Failed to create .maintenance file: {}", e);
        } else {
            info!("🚧 Maintenance mode engaged.");
        }

        info!("👋 Kernel shutting down gracefully.");
        shutdown.notify_one();
    });

    Ok(Json(serde_json::json!({ "status": "shutting_down" })))
}

/// 外部（フロントエンド等）からイベントをバスに注入するハンドラ
pub async fn post_event_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(event_data): Json<exiv_shared::ExivEventData>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    // 🛡️ Security Check: 外部からの重要なシステムイベントの注入を禁止
    match &event_data {
        // H-15: Only allow safe event types from external sources
        // SystemNotification removed - external callers should not inject system notifications
        exiv_shared::ExivEventData::MessageReceived(_) |
        exiv_shared::ExivEventData::VisionUpdated(_) |
        exiv_shared::ExivEventData::GazeUpdated(_) => {
            // これらは許可
        },
        _ => {
            error!("🚫 SECURITY ALERT: External attempt to inject restricted event: {:?}", event_data);
            return Err(AppError::Vers(exiv_shared::ExivError::PermissionDenied(exiv_shared::Permission::AdminAccess)));
        }
    }

    let envelope = crate::EnvelopedEvent::system(event_data);
    if let Err(e) = state.event_tx.send(envelope).await {
        error!("Failed to send external event: {}", e);
        return Err(AppError::Internal(anyhow::anyhow!("Failed to publish event")));
    }
    Ok(Json(serde_json::json!({ "status": "published" })))
}

/// メッセージ送信ハンドラ
/// K-07: Requires auth since it injects events into the bus
pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(msg): Json<ExivMessage>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    let envelope = crate::EnvelopedEvent::system(exiv_shared::ExivEventData::MessageReceived(msg));
    if let Err(e) = state.event_tx.send(envelope).await {
        error!("Failed to send chat message event: {}", e);
        return Err(AppError::Internal(anyhow::anyhow!("Failed to accept message")));
    }
    Ok(Json(serde_json::json!({ "status": "accepted" })))
}

/// SSEイベントストリームハンドラ
pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx.subscribe();
    let stream = async_stream::stream! {
        yield Ok(Event::default().event("handshake").data("connected"));
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Ok(json) = serde_json::to_string(&event) {
                        yield Ok(Event::default().data(json));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SSE stream lagged by {} messages", n);
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// 最近のイベント履歴を取得
pub async fn get_history(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let history = state.event_history.read().await;
    let history_vec: Vec<_> = history.iter().collect();
    Ok(Json(serde_json::json!(history_vec)))
}

/// システムメトリクスを取得
pub async fn get_metrics(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let history_len = state.event_history.read().await.len();
    let max_size = state.config.event_history_size;

    Ok(Json(serde_json::json!({
        "total_requests": state.metrics.total_requests.load(std::sync::atomic::Ordering::Relaxed),
        "total_memories": state.metrics.total_memories.load(std::sync::atomic::Ordering::Relaxed),
        "total_episodes": state.metrics.total_episodes.load(std::sync::atomic::Ordering::Relaxed),
        "ram_usage": "Unknown", // Future implementation
        "event_history": {
            "current_size": history_len,
            "max_size": max_size,
            "memory_estimate_bytes": history_len * std::mem::size_of::<std::sync::Arc<exiv_shared::ExivEvent>>(),
        }
    })))
}

/// 保存されたメモリ（履歴）を取得
pub async fn get_memories(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, value FROM plugin_data WHERE key LIKE 'mem:%' ORDER BY key DESC LIMIT 100"
    )
    .fetch_all(&state.pool)
    .await?;

    let memories: Vec<serde_json::Value> = rows.into_iter()
        .filter_map(|(_k, v)| serde_json::from_str(&v).ok())
        .collect();

    Ok(Json(serde_json::json!(memories)))
}

/// Get pending permission requests (Human-in-the-Loop)
pub async fn get_pending_permissions(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<crate::PermissionRequest>>> {
    let requests = crate::get_pending_permission_requests(&state.pool).await?;
    Ok(Json(requests))
}

/// Approve a permission request
#[derive(Deserialize)]
pub struct PermissionDecisionPayload {
    // Accepted for backwards compatibility but not used for audit trail
    // (actor identity determined from auth, not user-supplied value)
    #[allow(dead_code)]
    approved_by: String,
}

pub async fn approve_permission(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(_payload): Json<PermissionDecisionPayload>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    // Use fixed "admin" actor since auth is via single API key (not user-supplied value)
    let actor_id = "admin".to_string();
    crate::update_permission_request(&state.pool, &request_id, "approved", &actor_id).await?;

    // Write audit log
    crate::db::spawn_audit_log(state.pool.clone(), crate::AuditLogEntry {
        timestamp: chrono::Utc::now(),
        event_type: "PERMISSION_REQUEST_APPROVED".to_string(),
        actor_id: Some(actor_id),
        target_id: Some(request_id.clone()),
        permission: None,
        result: "SUCCESS".to_string(),
        reason: "Human administrator approved permission request".to_string(),
        metadata: None,
        trace_id: None,
    });

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Permission request approved"
    })))
}

/// Deny a permission request
pub async fn deny_permission(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(request_id): axum::extract::Path<String>,
    Json(_payload): Json<PermissionDecisionPayload>,
) -> AppResult<Json<serde_json::Value>> {
    check_auth(&state, &headers)?;
    // Use fixed "admin" actor since auth is via single API key (not user-supplied value)
    let actor_id = "admin".to_string();
    crate::update_permission_request(&state.pool, &request_id, "denied", &actor_id).await?;

    // Write audit log
    crate::db::spawn_audit_log(state.pool.clone(), crate::AuditLogEntry {
        timestamp: chrono::Utc::now(),
        event_type: "PERMISSION_REQUEST_DENIED".to_string(),
        actor_id: Some(actor_id),
        target_id: Some(request_id.clone()),
        permission: None,
        result: "SUCCESS".to_string(),
        reason: "Human administrator denied permission request".to_string(),
        metadata: None,
        trace_id: None,
    });

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Permission request denied"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use crate::config::AppConfig;
    use crate::managers::{PluginRegistry, AgentManager, PluginManager, SystemMetrics};
    use crate::DynamicRouter;
    use std::collections::VecDeque;
    use tokio::sync::{broadcast, mpsc, Notify, RwLock};
    use sqlx::SqlitePool;

    async fn create_test_app_state(admin_api_key: Option<String>) -> Arc<AppState> {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::db::init_db(&pool, "sqlite::memory:").await.unwrap();

        let (event_tx, _event_rx) = mpsc::channel(100);
        let (tx, _rx) = broadcast::channel(100);

        let registry = Arc::new(PluginRegistry::new(5, 10));
        let agent_manager = AgentManager::new(pool.clone());
        let plugin_manager = Arc::new(PluginManager::new(
            pool.clone(),
            vec![],
            30,
            10,
        ));

        let dynamic_router = Arc::new(DynamicRouter {
            router: RwLock::new(axum::Router::new()),
        });

        let metrics = Arc::new(SystemMetrics::new());
        let event_history = Arc::new(RwLock::new(VecDeque::new()));

        let mut config = AppConfig::load().unwrap();
        config.admin_api_key = admin_api_key;

        let rate_limiter = Arc::new(crate::middleware::RateLimiter::new(10, 20));

        Arc::new(AppState {
            tx,
            registry,
            event_tx,
            pool,
            agent_manager,
            plugin_manager,
            dynamic_router,
            config,
            event_history,
            metrics,
            rate_limiter,
            shutdown: Arc::new(Notify::new()),
        })
    }

    #[tokio::test]
    async fn test_check_auth_with_valid_api_key() {
        let state = create_test_app_state(Some("test-secret-key".to_string())).await;
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("test-secret-key"));

        let result = check_auth(&state, &headers);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_auth_with_invalid_api_key() {
        let state = create_test_app_state(Some("test-secret-key".to_string())).await;
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("wrong-key"));

        let result = check_auth(&state, &headers);
        assert!(result.is_err());

        if let Err(AppError::Vers(exiv_shared::ExivError::PermissionDenied(perm))) = result {
            assert_eq!(perm, exiv_shared::Permission::AdminAccess);
        } else {
            panic!("Expected PermissionDenied error");
        }
    }

    #[tokio::test]
    async fn test_check_auth_with_missing_header() {
        let state = create_test_app_state(Some("test-secret-key".to_string())).await;
        let headers = HeaderMap::new();

        let result = check_auth(&state, &headers);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_auth_no_key_configured_debug_mode() {
        // In debug mode (cfg!(debug_assertions) = true), no API key allows access
        let state = create_test_app_state(None).await;
        let headers = HeaderMap::new();

        let result = check_auth(&state, &headers);

        #[cfg(debug_assertions)]
        assert!(result.is_ok());

        #[cfg(not(debug_assertions))]
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_auth_empty_api_key_header() {
        let state = create_test_app_state(Some("test-secret-key".to_string())).await;
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static(""));

        let result = check_auth(&state, &headers);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_auth_case_sensitive() {
        let state = create_test_app_state(Some("test-secret-key".to_string())).await;
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", HeaderValue::from_static("TEST-SECRET-KEY"));

        let result = check_auth(&state, &headers);
        assert!(result.is_err(), "API key should be case-sensitive");
    }
}
