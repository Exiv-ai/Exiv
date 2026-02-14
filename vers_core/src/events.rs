use crate::managers::{PluginRegistry, PluginManager};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use vers_shared::{Permission, VersEvent};

pub struct EventProcessor {
    registry: Arc<PluginRegistry>,
    plugin_manager: Arc<PluginManager>,
    tx_internal: broadcast::Sender<Arc<VersEvent>>,
    refresh_tx: mpsc::Sender<()>, // 🔄 ルート更新用チャンネル
}

impl EventProcessor {
    pub fn new(
        registry: Arc<PluginRegistry>,
        plugin_manager: Arc<PluginManager>,
        tx_internal: broadcast::Sender<Arc<VersEvent>>,
        dynamic_router: Arc<crate::DynamicRouter>,
    ) -> Self {
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);
        let registry_clone = registry.clone();
        let dynamic_router_clone = dynamic_router.clone();

        // 🔄 デバウンスされたルート更新タスク
        tokio::spawn(async move {
            while let Some(_) = refresh_rx.recv().await {
                // 連続した要求を待機してまとめる (デバウンス)
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                // チャンネルに溜まった残りのメッセージを空にする
                while let Ok(_) = refresh_rx.try_recv() {}

                info!("🔄 Refreshing dynamic routes (debounced)...");
                let mut dynamic_routes = axum::Router::new();
                let plugins_snapshot = registry_clone.plugins.read().await;
                for (id, plugin) in plugins_snapshot.iter() {
                    if let Some(web) = plugin.as_web() {
                        dynamic_routes = web.register_routes(dynamic_routes);
                        info!("🔌 Re-registered dynamic routes for plugin: {}", id);
                    }
                }
                drop(plugins_snapshot);

                let mut router_lock = dynamic_router_clone.router.write().await;
                *router_lock = dynamic_routes;
            }
        });

        Self {
            registry,
            plugin_manager,
            tx_internal,
            refresh_tx,
        }
    }

    async fn request_refresh(&self) {
        let _ = self.refresh_tx.try_send(());
    }

    pub async fn process_loop(
        &self,
        mut event_rx: mpsc::Receiver<crate::EnvelopedEvent>,
        event_tx: mpsc::Sender<crate::EnvelopedEvent>,
    ) {
        info!("🧠 Kernel Event Processor Loop started.");

        while let Some(envelope) = event_rx.recv().await {
            let event = envelope.event.clone();
            let trace_id = event.trace_id;
            
            // 1. 全プラグイン（および内部システムハンドラ）に配信
            self.registry.dispatch_event(envelope.clone(), &event_tx).await;

            // 2. 内部イベント分岐処理
            match &event.data {
                vers_shared::VersEventData::ThoughtResponse {
                    agent_id,
                    content,
                    source_message_id: _,
                } => {
                    info!(trace_id = %trace_id, agent_id = %agent_id, "🧠 Received ThoughtResponse");
                    let msg = vers_shared::VersMessage::new(
                        vers_shared::MessageSource::Agent { id: agent_id.clone() },
                        content.clone(),
                    );
                    let msg_received = Arc::new(vers_shared::VersEvent::with_trace(
                        trace_id, 
                        vers_shared::VersEventData::MessageReceived(msg.clone())
                    ));
                    let _ = self.tx_internal.send(msg_received.clone());
                    
                    let system_envelope = crate::EnvelopedEvent {
                        event: msg_received,
                        issuer: None,
                        correlation_id: Some(trace_id),
                        depth: envelope.depth + 1,
                    };
                    let _ = event_tx.send(system_envelope).await;
                }
                vers_shared::VersEventData::ActionRequested { requester, action: _action } => {
                    // Security Check: Verify that the issuer matches the requester
                    let is_valid_issuer = match &envelope.issuer {
                        Some(issuer_id) => issuer_id == requester,
                        None => true, // System/Kernel can act on behalf of anyone
                    };

                    if !is_valid_issuer {
                        error!(
                            trace_id = %trace_id,
                            requester_id = %requester,
                            issuer_id = ?envelope.issuer,
                            "🚫 FORGERY DETECTED: Plugin attempted to impersonate another ID in ActionRequested"
                        );
                        continue; // Drop the event
                    }

                    if self.authorize(requester, Permission::InputControl).await {
                        info!(trace_id = %trace_id, requester_id = %requester, "✅ Action authorized");
                        let _ = self.tx_internal.send(event.clone());
                    } else {
                        error!(
                            trace_id = %trace_id,
                            requester_id = %requester,
                            "🚫 SECURITY VIOLATION: Plugin attempted Action without InputControl permission"
                        );
                    }
                }
                vers_shared::VersEventData::PermissionGranted { plugin_id, permission } => {
                    info!(
                        trace_id = %trace_id,
                        plugin_id = %plugin_id,
                        permission = ?permission,
                        "🔐 Permission GRANTED to plugin"
                    );
                    
                    // 1. 権限リストの更新 (In-memory)
                    let vers_id = vers_shared::VersId::from_name(plugin_id);
                    self.registry.update_effective_permissions(vers_id, permission.clone()).await;
                    
                    // 2. Capability の注入
                    let plugins = self.registry.plugins.read().await;
                    if let Some(plugin) = plugins.get(plugin_id) {
                        if let Some(cap) = self.plugin_manager.get_capability_for_permission(permission) {
                            let plugin_id = plugin_id.clone(); // Clone for spawn
                            info!(trace_id = %trace_id, plugin_id = %plugin_id, "💉 Injecting capability");
                            let plugin = plugin.clone();
                            tokio::spawn(async move {
                                if let Err(e) = plugin.on_capability_injected(cap).await {
                                    error!(trace_id = %trace_id, plugin_id = %plugin_id, error = %e, "❌ Failed to inject capability");
                                }
                            });
                        }
                    }
                    drop(plugins);

                    // 3. ルーティングの更新をリクエスト
                    self.request_refresh().await;
                }
                vers_shared::VersEventData::ConfigUpdated { .. } => {
                    // 設定変更によってルートが変わる可能性があるため更新をリクエスト
                    self.request_refresh().await;
                    let _ = self.tx_internal.send(event);
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
