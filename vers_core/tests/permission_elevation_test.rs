use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use vers_shared::{
    Plugin, PluginManifest, ServiceType, VersId, VersEvent, Permission, 
    PluginCapability, PluginCast
};
use vers_core::managers::{PluginRegistry, PluginManager};
use vers_core::events::EventProcessor;
use sqlx::SqlitePool;

struct MockPlugin {
    id: String,
    injected: Arc<RwLock<bool>>,
}

impl MockPlugin {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            injected: Arc::new(RwLock::new(false)),
        }
    }
}

impl PluginCast for MockPlugin {
    fn as_any(&self) -> &dyn std::any::Any { self }
}

#[async_trait::async_trait]
impl Plugin for MockPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.id.clone(),
            name: "Mock".to_string(),
            description: "".to_string(),
            version: "1.0.0".to_string(),
            service_type: ServiceType::Skill,
            tags: vec![],
            is_active: true,
            is_configured: true,
            required_config_keys: vec![],
            action_icon: None,
            action_target: None,
            icon_data: None,
            magic_seal: 0x56455253,
            sdk_version: "1.0.0".to_string(),
            required_permissions: vec![],
            provided_capabilities: vec![],
            provided_tools: vec![],
        }
    }

    async fn on_event(&self, _event: &VersEvent) -> anyhow::Result<Option<VersEvent>> {
        Ok(None)
    }

    async fn on_capability_injected(&self, capability: PluginCapability) -> anyhow::Result<()> {
        let PluginCapability::Network(_) = capability;
        let mut lock = self.injected.write().await;
        *lock = true;
        Ok(())
    }
}

#[tokio::test]
async fn test_dynamic_permission_elevation_flow() {
    // 1. Setup Kernel Components
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut registry_raw = PluginRegistry::new();
    let plugin_manager = Arc::new(PluginManager::new(pool.clone()));
    let (tx_internal, _rx_internal) = tokio::sync::broadcast::channel(10);

    // 2. Register Mock Plugin
    let plugin_id = "test.mock";
    let mock_plugin = Arc::new(MockPlugin::new(plugin_id));
    let injected_flag = mock_plugin.injected.clone();
    
    registry_raw.plugins.insert(plugin_id.to_string(), mock_plugin.clone());
    
    let registry = Arc::new(registry_raw);
    let processor = EventProcessor::new(registry.clone(), plugin_manager.clone(), tx_internal);
    let (event_tx, event_rx) = mpsc::channel(10);

    // 3. Verify initial state (no permission)
    let vers_id = VersId::from_name(plugin_id);
    {
        let perms = registry.effective_permissions.read().await;
        assert!(!perms.contains_key(&vers_id));
    }

    // 4. Simulate PermissionGranted Event
    let grant_event = VersEvent::PermissionGranted {
        plugin_id: plugin_id.to_string(),
        permission: Permission::NetworkAccess,
    };

    // Start processor in background
    let event_tx_clone = event_tx.clone();
    tokio::spawn(async move {
        processor.process_loop(event_rx, event_tx_clone).await;
    });

    // Send event
    event_tx.send(grant_event).await.unwrap();

    // 5. Assert: Registry is updated
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    {
        let perms = registry.effective_permissions.read().await;
        assert!(perms.contains_key(&vers_id));
        assert!(perms.get(&vers_id).unwrap().contains(&Permission::NetworkAccess));
    }

    // 6. Assert: Plugin received the capability
    {
        let is_injected = injected_flag.read().await;
        assert!(*is_injected, "Capability should have been injected into the plugin");
    }
}

