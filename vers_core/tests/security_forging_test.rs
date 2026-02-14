use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use sqlx::SqlitePool;
use vers_core::{
    managers::{PluginRegistry, PluginManager},
    events::EventProcessor,
    DynamicRouter,
};
use vers_shared::{
    Plugin, PluginCast, PluginManifest, VersEvent, VersId, Permission, ServiceType, HandAction
};

// -------------------------------------------------------------------------
// Mock Plugins
// -------------------------------------------------------------------------

// 1. 権限を持つ正規の管理者プラグイン (ただし今回はIDを使われるだけなので中身は空でも良い)
struct AdminPlugin(VersId);
impl PluginCast for AdminPlugin { fn as_any(&self) -> &dyn std::any::Any { self } }
#[async_trait::async_trait]
impl Plugin for AdminPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.0.to_string(),
            name: "Admin".to_string(),
            description: "Authorized plugin".to_string(),
            version: "1.0".to_string(),
            category: vers_shared::PluginCategory::Agent,
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
            required_permissions: vec![Permission::InputControl],
            provided_capabilities: vec![],
            provided_tools: vec![],
        }
    }
}

// 2. 権限を持たない悪意あるプラグイン
struct MaliciousPlugin {
    self_id: VersId,
    target_id: VersId, // 偽装対象
}
impl PluginCast for MaliciousPlugin { fn as_any(&self) -> &dyn std::any::Any { self } }
#[async_trait::async_trait]
impl Plugin for MaliciousPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.self_id.to_string(),
            name: "Malice".to_string(),
            description: "Trying to forge events".to_string(),
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
            required_permissions: vec![], // 権限なし！
            provided_capabilities: vec![],
            provided_tools: vec![],
        }
    }

    async fn on_event(&self, event: &VersEvent) -> anyhow::Result<Option<vers_shared::VersEventData>> {
        // トリガーイベントを受け取ったら、偽装イベントを発行する
        if let vers_shared::VersEventData::SystemNotification(msg) = &event.data {
            if msg == "TRIGGER_ATTACK" {
                // ここで AdminPlugin の ID を騙って ActionRequested を生成
                return Ok(Some(vers_shared::VersEventData::ActionRequested {
                    requester: self.target_id.clone(), // <--- FORGING HERE
                    action: HandAction::Wait { ms: 100 },
                }));
            }
        }
        Ok(None)
    }
}

// -------------------------------------------------------------------------
// Test Case
// -------------------------------------------------------------------------

#[tokio::test]
async fn test_vulnerability_event_forging() {
    // 1. Setup DB & Managers
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    // 必要なテーブルを作成
    sqlx::query("CREATE TABLE plugin_settings (plugin_id TEXT PRIMARY KEY, is_active BOOLEAN, allowed_permissions TEXT)")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE plugin_configs (plugin_id TEXT, config_key TEXT, config_value TEXT, PRIMARY KEY(plugin_id, config_key))")
        .execute(&pool).await.unwrap();

    let plugin_manager = Arc::new(PluginManager::new(pool.clone(), vec![], 5, 10));
    let registry = Arc::new(PluginRegistry::new(5, 10));
    
    // 2. Setup IDs
    let admin_id = VersId::new();
    let malice_id = VersId::new();

    // 3. Register Plugins
    {
        let mut plugins = registry.plugins.write().await;
        plugins.insert(admin_id.to_string(), Arc::new(AdminPlugin(admin_id)));
        plugins.insert(malice_id.to_string(), Arc::new(MaliciousPlugin { self_id: malice_id, target_id: admin_id }));
    }

    // 4. Grant Permissions (Admin only)
    registry.update_effective_permissions(admin_id, Permission::InputControl).await;
    // Maliceには権限を与えない

    // 5. Setup Event Loop
    let (tx_broadcast, mut rx_broadcast) = broadcast::channel::<Arc<VersEvent>>(100);
    let (tx_internal, rx_internal) = mpsc::channel::<vers_core::EnvelopedEvent>(100);
    
    let dynamic_router = Arc::new(DynamicRouter {
        router: tokio::sync::RwLock::new(axum::Router::new()),
    });

    let metrics = Arc::new(vers_core::managers::SystemMetrics::new());
    let event_history = Arc::new(tokio::sync::RwLock::new(VecDeque::new()));

    let processor = EventProcessor::new(
        registry.clone(),
        plugin_manager.clone(),
        tx_broadcast.clone(),
        dynamic_router,
        event_history,
        metrics,
        1000, // max_history_size
    );

    // Run Processor in background
    let tx_internal_clone = tx_internal.clone();
    tokio::spawn(async move {
        processor.process_loop(rx_internal, tx_internal_clone).await;
    });

    // 6. Trigger Attack
    // Maliceプラグインに "TRIGGER_ATTACK" を送る。
    // Malice は on_event で偽装イベント(ActionRequested from Admin)を返す。
    // Registry.dispatch_event -> tx_internal -> EventProcessor -> authorize(requester) -> Pass?
    
    // Start the ping-pong (or in this case, the attack trigger)
    let trigger = vers_core::EnvelopedEvent {
        event: Arc::new(VersEvent::new(vers_shared::VersEventData::SystemNotification("TRIGGER_ATTACK".to_string()))),
        issuer: None,
        correlation_id: None,
        depth: 0,
    };
    
    // 手動で dispatch を呼ぶ
    registry.dispatch_event(trigger, &tx_internal).await;

    // 7. Verify Result
    // Security Fix後は、偽装イベントはドロップされるはず。
    
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), rx_broadcast.recv()).await;

    match result {
        Ok(Ok(event)) => {
            match &event.data {
                vers_shared::VersEventData::ActionRequested { requester, .. } => {
                    // 偽装イベントが来たらテスト失敗！
                    panic!("❌ SECURITY FAIL: Forged event was NOT blocked! Requester: {}", requester);
                }
                _ => {
                     println!("Received unrelated event: {:?}", event);
                }
            }
        }
        Ok(Err(e)) => panic!("Broadcast error: {}", e),
        Err(_) => {
            // タイムアウト = イベントが来なかった = ブロックされた = 成功！
            println!("✅ SUCCESS: Forged event was blocked (timeout).");
        }
    }
}
