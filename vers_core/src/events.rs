use crate::managers::{PluginRegistry, PluginManager};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use vers_shared::{Permission, VersEvent};

pub struct EventProcessor {
    registry: Arc<PluginRegistry>,
    plugin_manager: Arc<PluginManager>,
    tx_internal: broadcast::Sender<VersEvent>,
}

impl EventProcessor {
    pub fn new(
        registry: Arc<PluginRegistry>,
        plugin_manager: Arc<PluginManager>,
        tx_internal: broadcast::Sender<VersEvent>,
    ) -> Self {
        Self {
            registry,
            plugin_manager,
            tx_internal,
        }
    }

    pub async fn process_loop(
        &self,
        mut event_rx: mpsc::Receiver<VersEvent>,
        event_tx: mpsc::Sender<VersEvent>,
    ) {
        info!("🧠 Kernel Event Processor Loop started.");

        while let Some(event) = event_rx.recv().await {
            // 1. 全プラグイン（および内部システムハンドラ）に配信
            self.registry.dispatch_event(&event, &event_tx).await;

            // 2. 内部イベント分岐処理
            match event.clone() {
                VersEvent::ThoughtResponse {
                    agent_id,
                    content,
                    source_message_id: _,
                } => {
                    info!("🧠 Received ThoughtResponse from Agent: {}", agent_id);
                    let msg = vers_shared::VersMessage::new(
                        vers_shared::MessageSource::Agent { id: agent_id },
                        content,
                    );
                    let _ = self.tx_internal.send(VersEvent::MessageReceived(msg.clone()));
                    let _ = event_tx.send(VersEvent::MessageReceived(msg)).await;
                }
                VersEvent::ActionRequested { requester, action } => {
                    if self.authorize(&requester, Permission::InputControl).await {
                        info!("✅ Action authorized for plugin: {}", requester);
                        let _ = self.tx_internal.send(VersEvent::ActionRequested { requester, action });
                    } else {
                        error!("🚫 SECURITY VIOLATION: Plugin {} attempted Action without InputControl permission.", requester);
                    }
                }
                VersEvent::PermissionGranted { plugin_id, permission } => {
                    info!("🔐 Permission {:?} GRANTED to plugin: {}", permission, plugin_id);
                    
                    // 1. 権限リストの更新 (In-memory)
                    let vers_id = vers_shared::VersId::from_name(&plugin_id);
                    self.registry.update_effective_permissions(vers_id, permission.clone()).await;
                    
                    // 2. Capability の注入
                    if let Some(plugin) = self.registry.plugins.get(&plugin_id) {
                        if let Some(cap) = self.plugin_manager.get_capability_for_permission(&permission) {
                            info!("💉 Injecting capability into {}", plugin_id);
                            if let Err(e) = plugin.on_capability_injected(cap).await {
                                error!("❌ Failed to inject capability into {}: {}", plugin_id, e);
                            }
                        }
                    }
                }
                _ => {
                    // SSE等への通知
                    let _ = self.tx_internal.send(event);
                }
            }
        }
    }

    async fn authorize(&self, requester_id: &vers_shared::VersId, required: Permission) -> bool {
        let perms_lock = self.registry.effective_permissions.read().await;
        if let Some(perms) = perms_lock.get(requester_id) {
            return perms.contains(&required);
        }
        false
    }
}
