use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tokio::sync::RwLock;
use vers_shared::{
    AgentMetadata, Plugin, PluginConfig,
    ReasoningEngine, VersMessage, Permission, PluginRuntimeContext,
    NetworkCapability, HttpRequest, vers_plugin
};

#[vers_plugin(
    name = "mind.deepseek",
    kind = "Reasoning",
    description = "Advanced reasoning using DeepSeek API.",
    version = "0.1.0",
    icon = "assets/icon.svg",
    action_icon = "Settings",
    config_keys = ["api_key", "model_id"],
    permissions = ["NetworkAccess"],
    capabilities = ["Reasoning", "Web"]
)]
pub struct DeepSeekPlugin {
    id: String,
    api_key: Arc<RwLock<String>>,
    model_id: Arc<RwLock<String>>,
    allowed_permissions: Arc<RwLock<Vec<Permission>>>,
    http_client: Arc<RwLock<Option<Arc<dyn NetworkCapability>>>>,
}

impl DeepSeekPlugin {
    pub async fn new_plugin(config: PluginConfig) -> anyhow::Result<Self> {
        let api_key = config.config_values.get("api_key").cloned().unwrap_or_default();
        let model_id = config.config_values.get("model_id").cloned().unwrap_or_else(|| "deepseek-chat".to_string());
        
        Ok(Self {
            id: config.id,
            api_key: Arc::new(RwLock::new(api_key)),
            model_id: Arc::new(RwLock::new(model_id)),
            allowed_permissions: Arc::new(RwLock::new(vec![])),
            http_client: Arc::new(RwLock::new(None)),
        })
    }
}

#[async_trait]
impl Plugin for DeepSeekPlugin {
    fn manifest(&self) -> vers_shared::PluginManifest {
        self.auto_manifest()
    }

    async fn on_plugin_init(
        &self,
        context: PluginRuntimeContext,
        network: Option<Arc<dyn NetworkCapability>>,
    ) -> anyhow::Result<()> {
        {
            let mut perms = self.allowed_permissions.write().await;
            *perms = context.effective_permissions;
        }
        {
            let mut client = self.http_client.write().await;
            *client = network;
        }
        Ok(())
    }

    async fn on_event(
        &self,
        event: &vers_shared::VersEvent,
    ) -> anyhow::Result<Option<vers_shared::VersEvent>> {
        match event {
            vers_shared::VersEvent::ThoughtRequested {
                agent,
                engine_id,
                message,
                context,
            } => {
                if engine_id != "mind.deepseek" {
                    return Ok(None);
                }
                let content = self.think(agent, message, context.clone()).await?;
                return Ok(Some(vers_shared::VersEvent::ThoughtResponse {
                    agent_id: agent.id.clone(),
                    content,
                    source_message_id: message.id.clone(),
                }));
            }
            vers_shared::VersEvent::ConfigUpdated { plugin_id, config } => {
                if plugin_id == "mind.deepseek" {
                    if let Some(key) = config.get("api_key") {
                        let mut api_key = self.api_key.write().await;
                        *api_key = key.clone();
                    }
                    if let Some(model) = config.get("model_id") {
                        let mut model_id = self.model_id.write().await;
                        *model_id = model.clone();
                    }
                    println!("🔌 DeepSeek Plugin configuration hot-reloaded.");
                }
            }
            _ => {}
        }
        Ok(None)
    }

    async fn on_capability_injected(
        &self,
        capability: vers_shared::PluginCapability,
    ) -> anyhow::Result<()> {
        match capability {
            vers_shared::PluginCapability::Network(net) => {
                let mut client = self.http_client.write().await;
                *client = Some(net);
                println!("💉 DeepSeek Plugin: NetworkCapability injected live.");
            }
        }
        Ok(())
    }
}

impl vers_shared::WebPlugin for DeepSeekPlugin {
    fn register_routes(
        &self,
        router: axum::Router<Arc<dyn Any + Send + Sync>>,
    ) -> axum::Router<Arc<dyn Any + Send + Sync>> {
        router.route(
            "/plugin/deepseek/status",
            axum::routing::get(|| async {
                axum::Json(serde_json::json!({
                    "status": "active",
                    "engine": "deepseek-reasoner"
                }))
            }),
        )
    }
}

#[async_trait]
impl ReasoningEngine for DeepSeekPlugin {
    fn name(&self) -> &str {
        "DeepSeek"
    }

    async fn think(
        &self,
        agent: &AgentMetadata,
        message: &VersMessage,
        context: Vec<VersMessage>,
    ) -> anyhow::Result<String> {
        let (api_key, model_id) = {
            let key = self.api_key.read().await;
            let model = self.model_id.read().await;
            (key.clone(), model.clone())
        };

        if api_key.is_empty() {
            return Err(anyhow::anyhow!("DeepSeek API Key not configured."));
        }

        let client = {
            let client_guard = self.http_client.read().await;
            let client_opt: &Option<Arc<dyn NetworkCapability>> = &*client_guard;
            client_opt.clone().ok_or_else(|| anyhow::anyhow!("NetworkCapability not injected."))?
        };

        let mut messages = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("You are {}. {}.", agent.name, agent.description)
        }));

        for msg in context {
            let role = match msg.source {
                vers_shared::MessageSource::User { .. } => "user",
                vers_shared::MessageSource::Agent { .. } => "assistant",
                vers_shared::MessageSource::System => "system",
            };
            messages.push(serde_json::json!({ "role": role, "content": msg.content }));
        }

        messages.push(serde_json::json!({ "role": "user", "content": message.content }));

        let req = HttpRequest {
            method: "POST".to_string(),
            url: "https://api.deepseek.com/chat/completions".to_string(),
            headers: [
                ("Authorization".to_string(), format!("Bearer {}", api_key)),
                ("Content-Type".to_string(), "application/json".to_string())
            ].into_iter().collect(),
            body: Some(serde_json::json!({
                "model": model_id,
                "messages": messages,
                "stream": false
            }).to_string()),
        };

        let resp = client.send_http_request(req).await?;
        let json: serde_json::Value = serde_json::from_str(&resp.body)?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid API response"))?
            .to_string();

        Ok(content)
    }
}