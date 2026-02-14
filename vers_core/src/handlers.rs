use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::Stream;
use serde::Deserialize;
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};
use tracing::{error, info};
use vers_shared::{AgentMetadata, PluginManifest, VersEvent, VersMessage};

use crate::AppState;

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
}

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct UpdateAgentRequest {
    pub metadata: HashMap<String, String>,
}

/// 全エージェントのリストを取得
pub async fn get_agents(State(state): State<Arc<AppState>>) -> Json<Vec<AgentMetadata>> {
    match state.agent_manager.list_agents().await {
        Ok(agents) => Json(agents),
        Err(e) => {
            error!("❌ Failed to list agents: {}", e);
            Json(vec![])
        }
    }
}

/// エージェント作成
pub async fn create_agent(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateAgentRequest>,
) -> Json<serde_json::Value> {
    match state
        .agent_manager
        .create_agent(
            &payload.name,
            &payload.description,
            &payload.default_engine,
            payload.metadata.unwrap_or_default(),
        )
        .await
    {
        Ok(_) => Json(serde_json::json!({ "status": "success" })),
        Err(e) => {
            error!("❌ Failed to create agent: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

/// エージェント設定の更新
pub async fn update_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAgentRequest>,
) -> Json<serde_json::Value> {
    match state.agent_manager.update_agent_config(&id, payload.metadata).await {
        Ok(_) => Json(serde_json::json!({ "status": "success" })),
        Err(e) => {
            error!("❌ Failed to update agent {}: {}", id, e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

/// 全プラグインのリストを取得（DB設定を反映）
pub async fn get_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<PluginManifest>> {
    match state.plugin_manager.list_plugins_with_settings(&state.registry).await {
        Ok(manifests) => Json(manifests),
        Err(e) => {
            error!("❌ Failed to list plugins: {}", e);
            Json(vec![])
        }
    }
}

/// プラグイン設定の取得
pub async fn get_plugin_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<HashMap<String, String>> {
    match state.plugin_manager.get_config(&id).await {
        Ok(config) => Json(config),
        Err(e) => {
            error!("❌ Failed to get config for {}: {}", id, e);
            Json(HashMap::new())
        }
    }
}

/// プラグイン設定の更新
pub async fn update_plugin_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateConfigPayload>,
) -> Json<serde_json::Value> {
    match state
        .plugin_manager
        .update_config(&id, &payload.key, &payload.value)
        .await
    {
        Ok(_) => {
            info!("⚙️ Config updated for plugin: {}. Broadcasting update...", id);
            
            // 最新の全設定を取得して通知
            if let Ok(full_config) = state.plugin_manager.get_config(&id).await {
                let _ = state.event_tx.send(VersEvent::ConfigUpdated {
                    plugin_id: id,
                    config: full_config,
                }).await;
            }
            
            Json(serde_json::json!({ "status": "success" }))
        },
        Err(e) => {
            error!("❌ Failed to update config for {}: {}", id, e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

/// プラグインの有効/無効状態を一括適用
pub async fn apply_plugin_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Vec<PluginToggleRequest>>,
) -> Json<bool> {
    info!(
        "📥 Received plugin settings apply request: {} items",
        payload.len()
    );

    let settings = payload.into_iter().map(|i| (i.id, i.is_active)).collect();

    match state.plugin_manager.apply_settings(settings).await {
        Ok(_) => Json(true),
        Err(e) => {
            error!("❌ Failed to apply plugin settings: {}", e);
            Json(false)
        }
    }
}

#[derive(Deserialize)]
pub struct GrantPermissionRequest {
    pub permission: vers_shared::Permission,
}

/// プラグインに権限を付与（承認）
pub async fn grant_permission_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<GrantPermissionRequest>,
) -> Json<serde_json::Value> {
    info!("🔐 Granting permission {:?} to plugin: {}", payload.permission, id);
    
    match state.plugin_manager.grant_permission(&id, payload.permission.clone()).await {
        Ok(_) => {
            // イベントループに通知して Capability を注入させる
            let _ = state.event_tx.send(VersEvent::PermissionGranted {
                plugin_id: id,
                permission: payload.permission,
            }).await;
            
            Json(serde_json::json!({ "status": "success" }))
        },
        Err(e) => {
            error!("❌ Failed to grant permission to {}: {}", id, e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

/// メッセージ送信ハンドラ
pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(msg): Json<VersMessage>,
) -> Json<serde_json::Value> {
    if let Err(e) = state.event_tx.send(VersEvent::MessageReceived(msg)).await {
        error!("❌ Failed to send message to event loop: {}", e);
        return Json(serde_json::json!({ "status": "error", "message": e.to_string() }));
    }
    Json(serde_json::json!({ "status": "accepted" }))
}

/// SSEイベントストリームハンドラ
pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx.subscribe();
    let stream = async_stream::stream! {
        yield Ok(Event::default().event("handshake").data("connected"));
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                yield Ok(Event::default().data(json));
            }
        }
    };
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
