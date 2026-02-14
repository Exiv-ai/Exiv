use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use vers_shared::{
    AgentMetadata, CapabilityType, Plugin, PluginConfig, PluginFactory, PluginManifest,
    ReasoningEngine, ServiceType, VersId as PluginId, VersMessage,
};

pub struct DeepSeekPlugin {
    id: PluginId,
    api_key: String,
    model_id: String,
}

impl DeepSeekPlugin {
    pub fn new(id: PluginId, api_key: Option<String>, model_id: Option<String>) -> Self {
        Self {
            id,
            api_key: api_key.unwrap_or_default(),
            model_id: model_id.unwrap_or_else(|| "deepseek-chat".to_string()),
        }
    }

    pub fn factory() -> Arc<dyn PluginFactory> {
        Arc::new(DeepSeekFactory)
    }

    pub fn register_routes<S>(&self, router: axum::Router<S>) -> axum::Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
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
            required_permissions: vec![],
            provided_capabilities: vec![CapabilityType::Reasoning],
            provided_tools: vec![],
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
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

        let client = reqwest::Client::new();

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

        let resp = client
            .post("https://api.deepseek.com/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !resp.status().is_success() {
            let error_text = resp.text().await?;
            return Err(anyhow::anyhow!("DeepSeek API Error: {}", error_text));
        }

        let json: serde_json::Value = resp.json().await?;

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
