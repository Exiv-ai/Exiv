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
}

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub key: String,
    pub value: String,
}

/// プラグインのマニフェストにDBの設定を適用する共通ロジック
fn apply_plugin_settings_to_manifest(
    mut manifest: PluginManifest,
    settings: &HashMap<String, bool>,
) -> PluginManifest {
    if let Some(&active) = settings.get(&manifest.id.to_string()) {
        manifest.is_active = active;
    }
    manifest
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
        .create_agent(&payload.name, &payload.description, &payload.default_engine)
        .await
    {
        Ok(_) => Json(serde_json::json!({ "status": "success" })),
        Err(e) => {
            error!("❌ Failed to create agent: {}", e);
            Json(serde_json::json!({ "status": "error", "message": e.to_string() }))
        }
    }
}

/// 全プラグインのリストを取得（DB設定を反映）
pub async fn get_plugins(State(state): State<Arc<AppState>>) -> Json<Vec<PluginManifest>> {
    let mut manifests = Vec::new();

    // DBから設定をロード
    let settings_result = sqlx::query_as::<_, crate::managers::PluginSetting>("SELECT * FROM plugin_settings")
        .fetch_all(&state.pool)
        .await;

    let settings: HashMap<String, bool> = match settings_result {
        Ok(list) => list
            .into_iter()
            .map(|s| (s.plugin_id, s.is_active))
            .collect(),
        Err(e) => {
            error!("❌ Failed to load plugin settings from DB: {}", e);
            HashMap::new()
        }
    };

    // レジストリから全プラグインを取得
    for manifest in state.registry.list_plugins() {
        manifests.push(apply_plugin_settings_to_manifest(manifest, &settings));
    }

    Json(manifests)
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
        Ok(_) => Json(serde_json::json!({ "status": "success" })),
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

    let mut tx = match state.pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!("❌ Failed to start transaction: {}", e);
            return Json(false);
        }
    };

    for item in payload {
        let res = sqlx::query(
            "UPDATE plugin_settings SET is_active = ? WHERE plugin_id = ?",
        )
        .bind(item.is_active)
        .bind(&item.id)
        .execute(&mut *tx)
        .await;

        if let Err(e) = res {
            error!("❌ Failed to save plugin setting for {}: {}", item.id, e);
            return Json(false);
        }
    }

    match tx.commit().await {
        Ok(_) => Json(true),
        Err(e) => {
            error!("❌ Failed to commit transaction: {}", e);
            Json(false)
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
