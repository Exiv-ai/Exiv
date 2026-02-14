use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use sqlx::SqlitePool;
use vers_core::{
    managers::{PluginRegistry, PluginManager},
    events::EventProcessor,
    DynamicRouter, EnvelopedEvent
};
use vers_shared::{
    Plugin, PluginCast, PluginManifest, VersEvent, ServiceType
};

// -------------------------------------------------------------------------
// Ping-Pong Plugins
// -------------------------------------------------------------------------

struct PingPlugin {
    id: String,
    target_id: String,
}
impl PluginCast for PingPlugin { fn as_any(&self) -> &dyn std::any::Any { self } }
#[async_trait::async_trait]
impl Plugin for PingPlugin {
    fn manifest(&self) -> PluginManifest {
        let m = PluginManifest {
            id: self.id.clone(),
            name: "Ping".to_string(),
            description: "".to_string(),
            version: "1.0".to_string(),
            category: vers_shared::PluginCategory::Tool,
            service_type: ServiceType::Reasoning,
            tags: vec![],
            is_active: true,
            is_configured: true,
            required_config_keys: vec![],
            action_icon: None,
            action_target: None,
            icon_data: None,
            magic_seal: 0x56455253,
            sdk_version: "1.0".to_string(),
            required_permissions: vec![],
            provided_capabilities: vec![],
            provided_tools: vec![],
        };
        m
    }

    async fn on_event(&self, event: &VersEvent) -> anyhow::Result<Option<vers_shared::VersEventData>> {
        if let vers_shared::VersEventData::SystemNotification(msg) = &event.data {
            if msg == &format!("TO_{}", self.id) {
                return Ok(Some(vers_shared::VersEventData::SystemNotification(format!("TO_{}", self.target_id))));
            }
        }
        Ok(None)
    }
}

// -------------------------------------------------------------------------
// Test Case
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_event_cascading_protection() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE plugin_settings (plugin_id TEXT PRIMARY KEY, is_active BOOLEAN, allowed_permissions TEXT)").execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE plugin_configs (plugin_id TEXT, config_key TEXT, config_value TEXT, PRIMARY KEY(plugin_id, config_key))").execute(&pool).await.unwrap();

    let plugin_manager = Arc::new(PluginManager::new(pool.clone(), vec![], 1, 10)); // 1 sec timeout
    let registry = Arc::new(PluginRegistry::new(1, 10));
    
    let id_a = "plugin.a".to_string();
    let id_b = "plugin.b".to_string();

    {
        let mut plugins = registry.plugins.write().await;
        plugins.insert(id_a.clone(), Arc::new(PingPlugin { id: id_a.clone(), target_id: id_b.clone() }));
        plugins.insert(id_b.clone(), Arc::new(PingPlugin { id: id_b.clone(), target_id: id_a.clone() }));
    }

    let (tx_broadcast, mut rx_broadcast) = broadcast::channel::<Arc<VersEvent>>(1000);
    let (tx_internal, rx_internal) = mpsc::channel::<EnvelopedEvent>(1000);
    
    let dynamic_router = Arc::new(DynamicRouter {
        router: tokio::sync::RwLock::new(axum::Router::new()),
    });

    let processor = EventProcessor::new(
        registry.clone(),
        plugin_manager.clone(),
        tx_broadcast.clone(),
        dynamic_router,
    );

    let tx_internal_for_loop = tx_internal.clone();
    tokio::spawn(async move {
        processor.process_loop(rx_internal, tx_internal_for_loop).await;
    });

    // Start the ping-pong
    let trigger = EnvelopedEvent {
        event: Arc::new(VersEvent::new(vers_shared::VersEventData::SystemNotification(format!("TO_{}", id_a)))),
        issuer: None,
        correlation_id: None,
        depth: 0,
    };
    
    // 手動で dispatch を呼ぶ
    registry.dispatch_event(trigger, &tx_internal).await;

    // Count events in broadcast
    let mut count = 0;
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(3));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => break,
            msg = rx_broadcast.recv() => {
                if msg.is_ok() {
                    count += 1;
                    if count > 100 { break; } // Safety break for test if protection fails
                }
            }
        }
    }

    println!("Total events broadcasted: {}", count);
    // If protection is working (limit e.g. 10), count should be around 10-20.
    // If NOT working, count will be 100 (due to safety break) or very high.
    assert!(count < 50, "Event cascading protection failed! Count: {}", count);
}
