use async_trait::async_trait;
use std::sync::Arc;
use vers_shared::{
    AgentMetadata, CapabilityType, Plugin, PluginConfig, PluginFactory, PluginManifest,
    ReasoningEngine, ServiceType, VersId as PluginId, VersMessage,
};

pub struct CerebrasPlugin {
    id: PluginId,
    api_key: String,
    model_id: String,
}

impl CerebrasPlugin {
    pub fn new(id: PluginId, api_key: Option<String>, model_id: Option<String>) -> Self {
        Self {
            id,
            api_key: api_key.unwrap_or_default(),
            model_id: model_id.unwrap_or_else(|| "llama3.1-70b".to_string()),
        }
    }

    pub fn factory() -> Arc<dyn PluginFactory> {
        Arc::new(CerebrasFactory)
    }
}

#[async_trait]
impl Plugin for CerebrasPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.id,
            name: "Cerebras Reasoning".to_string(),
            description: "High-speed reasoning via Cerebras API.".to_string(),
            version: "0.1.0".to_string(),
            service_type: ServiceType::Reasoning,
            tags: vec!["#LLM".to_string(), "#FAST".to_string()],
            is_active: true,
            is_configured: true,
            required_config_keys: vec!["api_key".to_string()],
            action_icon: Some("Zap".to_string()),
            action_target: None,
            required_permissions: vec![],
            provided_capabilities: vec![CapabilityType::Reasoning],
            provided_tools: vec![],
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_reasoning(&self) -> Option<&dyn ReasoningEngine> {
        Some(self)
    }
}

#[async_trait]
impl ReasoningEngine for CerebrasPlugin {
    fn name(&self) -> &str {
        "Cerebras"
    }

    async fn think(
        &self,
        _agent: &AgentMetadata,
        _message: &VersMessage,
        _context: Vec<VersMessage>,
    ) -> anyhow::Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "Cerebras API Key is not configured in System Settings."
            ));
        }
        Ok(format!(
            "Cerebras [{}] is processing at lightning speed (mock).",
            self.model_id
        ))
    }
}

pub struct CerebrasFactory;

#[async_trait]
impl PluginFactory for CerebrasFactory {
    fn name(&self) -> &str {
        "mind.cerebras"
    }
    fn service_type(&self) -> ServiceType {
        ServiceType::Reasoning
    }

    async fn create(&self, config: PluginConfig) -> anyhow::Result<Arc<dyn Plugin>> {
        let api_key = config.config_values.get("api_key").cloned();
        let model_id = config.config_values.get("model_id").cloned();
        let plugin = CerebrasPlugin::new(config.id, api_key, model_id);
        Ok(Arc::new(plugin))
    }
}
