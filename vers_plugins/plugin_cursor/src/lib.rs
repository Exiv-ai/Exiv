use async_trait::async_trait;
use std::sync::Arc;
use vers_shared::{
    CapabilityType, Permission, Plugin, PluginConfig, PluginFactory, PluginManifest, ServiceType,
    VersId as PluginId,
};

pub struct CursorPlugin {
    id: PluginId,
}

impl CursorPlugin {
    pub fn new(id: PluginId) -> Self {
        Self { id }
    }

    pub fn factory() -> Arc<dyn PluginFactory> {
        Arc::new(CursorFactory)
    }

    fn base_manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.id,
            name: "1.6.x Neural Cursor".to_string(),
            description:
                "High-precision dot cursor with fluid motion trails from Karin System 1.6.12."
                    .to_string(),
            version: "1.6.12".to_string(),
            service_type: ServiceType::HAL,
            tags: vec!["#HAL".to_string(), "#UI".to_string(), "#CORE".to_string()],
            is_active: true,
            is_configured: true,
            required_config_keys: vec![],
            action_icon: Some("MousePointer2".to_string()),
            action_target: Some("MODAL_UI_SETTINGS".to_string()),
            required_permissions: vec![Permission::InputControl],
            provided_capabilities: vec![CapabilityType::HAL],
            provided_tools: vec![],
        }
    }
}

#[async_trait]
impl Plugin for CursorPlugin {
    fn manifest(&self) -> PluginManifest {
        self.base_manifest()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct CursorFactory;

#[async_trait]
impl PluginFactory for CursorFactory {
    fn name(&self) -> &str {
        "hal.cursor"
    }
    fn service_type(&self) -> ServiceType {
        ServiceType::HAL
    }

    async fn create(&self, config: PluginConfig) -> anyhow::Result<Arc<dyn Plugin>> {
        let plugin = CursorPlugin::new(config.id);
        Ok(Arc::new(plugin))
    }
}
