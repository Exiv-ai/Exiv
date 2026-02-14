use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use vers_shared::{
    AgentMetadata, CapabilityType, Plugin, PluginConfig, PluginFactory, PluginManifest,
    ReasoningEngine, ServiceType, VersId as PluginId, VersMessage, Permission, PluginRuntimeContext,
    NetworkCapability, HttpRequest
};

pub struct DeepSeekPlugin {
    id: PluginId,
    api_key: String,
    model_id: String,
    allowed_permissions: std::sync::RwLock<Vec<Permission>>,
    http_client: std::sync::RwLock<Option<Arc<dyn NetworkCapability>>>,
}

impl DeepSeekPlugin {
    pub fn new(id: PluginId, api_key: Option<String>, model_id: Option<String>) -> Self {
        Self {
            id,
            api_key: api_key.unwrap_or_default(),
            model_id: model_id.unwrap_or_else(|| "deepseek-chat".to_string()),
            allowed_permissions: std::sync::RwLock::new(vec![]),
            http_client: std::sync::RwLock::new(None),
        }
    }

    pub fn factory() -> Arc<dyn PluginFactory> {
        Arc::new(DeepSeekFactory)
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
                    "engine": "deepseek-reasoner",
                    "capabilities": ["text-generation", "context-recall"]
                }))
            }),
        )
    }
}

#[async_trait]
impl Plugin for DeepSeekPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.id,
            name: "DeepSeek Reasoning".to_string(),
            description: "Advanced reasoning using DeepSeek API.".to_string(),
            version: "0.1.0".to_string(),
            service_type: ServiceType::Reasoning,
            tags: vec!["#LLM".to_string(), "#MIND".to_string()],
            is_active: true,
            is_configured: true,
            required_config_keys: vec!["api_key".to_string()],
            action_icon: Some("Brain".to_string()),
            action_target: None,
            required_permissions: vec![Permission::NetworkAccess],
            provided_capabilities: vec![CapabilityType::Reasoning],
            provided_tools: vec![],
        }
    }

    async fn on_plugin_init(
        &self,
        context: PluginRuntimeContext,
        network: Option<Arc<dyn NetworkCapability>>,
    ) -> anyhow::Result<()> {
        {
            let mut perms = self.allowed_permissions.write().map_err(|_| anyhow::anyhow!("Lock error"))?;
            *perms = context.effective_permissions;
            tracing::info!("🔐 DeepSeek initialized with permissions: {:?}", *perms);
        }
        {
            let mut client = self.http_client.write().map_err(|_| anyhow::anyhow!("Lock error"))?;
            *client = network;
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_web(&self) -> Option<&dyn vers_shared::WebPlugin> {
        Some(self)
    }

    async fn on_event(
        &self,
        event: &vers_shared::VersEvent,
    ) -> anyhow::Result<Option<vers_shared::VersEvent>> {
        if let vers_shared::VersEvent::ThoughtRequested {
            agent,
            message,
            context,
        } = event
        {
            tracing::info!(
                "🧠 DeepSeek received ThoughtRequested for agent: {}",
                agent.name
            );

            // 思考の実行
            match self.think(agent, message, context.clone()).await {
                Ok(content) => {
                    return Ok(Some(vers_shared::VersEvent::ThoughtResponse {
                        agent_id: agent.id,
                        content,
                        source_message_id: message.id,
                    }));
                }
                Err(e) => {
                    tracing::error!("❌ DeepSeek thinking error: {}", e);
                }
            }
        }
        Ok(None)
    }

    fn as_reasoning(&self) -> Option<&dyn ReasoningEngine> {
        Some(self)
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
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "DeepSeek API Key is not configured in System Settings."
            ));
        }

        // 🛡️ Capability Check (Physical Isolation)
        let client = {
            let client_guard = self.http_client.read().map_err(|_| anyhow::anyhow!("Lock error"))?;
            client_guard.clone().ok_or_else(|| anyhow::anyhow!(
                "Security Violation: NetworkCapability not injected. Check permissions."
            ))?
        };

        // Build messages
        let mut messages = Vec::new();

        // System Prompt
        let system_content = format!("You are {}. {}.", agent.name, agent.description);
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_content
        }));

        // History
        for msg in context {
            let role = match msg.source {
                vers_shared::MessageSource::User { .. } => "user",
                vers_shared::MessageSource::Agent(_) => "assistant",
                vers_shared::MessageSource::System => "system",
            };
            messages.push(serde_json::json!({
                "role": role,
                "content": msg.content
            }));
        }

        // Current Message
        messages.push(serde_json::json!({
            "role": "user",
            "content": message.content
        }));

        let payload = serde_json::json!({
            "model": self.model_id,
            "messages": messages,
            "stream": false
        });

        // Use injected capability
        let mut headers = std::collections::HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", self.api_key));
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let req = HttpRequest {
            method: "POST".to_string(),
            url: "https://api.deepseek.com/chat/completions".to_string(),
            headers,
            body: Some(payload.to_string()),
        };

        let resp = client.send_http_request(req).await?;

        if resp.status < 200 || resp.status >= 300 {
            return Err(anyhow::anyhow!("DeepSeek API Error: {} - {}", resp.status, resp.body));
        }

        let json: serde_json::Value = serde_json::from_str(&resp.body)?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format from DeepSeek"))?
            .to_string();

        Ok(content)
    }
}

pub struct DeepSeekFactory;

#[async_trait]
impl PluginFactory for DeepSeekFactory {
    fn name(&self) -> &str {
        "mind.deepseek"
    }
    fn service_type(&self) -> ServiceType {
        ServiceType::Reasoning
    }

    async fn create(&self, config: PluginConfig) -> anyhow::Result<Arc<dyn Plugin>> {
        let api_key = config.config_values.get("api_key").cloned();
        let model_id = config.config_values.get("model_id").cloned();
        let plugin = DeepSeekPlugin::new(config.id, api_key, model_id);
        Ok(Arc::new(plugin))
    }
}
