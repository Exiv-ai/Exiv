use crate::managers::{MessageRouter, PluginRegistry};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};
use vers_shared::{Permission, VersEvent};

pub struct EventProcessor {
    registry: Arc<PluginRegistry>,
    router: Arc<MessageRouter>,
    tx_internal: broadcast::Sender<VersEvent>,
}

impl EventProcessor {
    pub fn new(
        registry: Arc<PluginRegistry>,
        router: Arc<MessageRouter>,
        tx_internal: broadcast::Sender<VersEvent>,
    ) -> Self {
        Self {
            registry,
            router,
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
            match event.clone() {
                VersEvent::MessageReceived(msg) => {
                    // 全体に通知
                    let _ = self
                        .tx_internal
                        .send(VersEvent::MessageReceived(msg.clone()));

                    // ルーティング
                    let router_clone = self.router.clone();
                    tokio::spawn(async move {
                        if let Err(e) = router_clone.route(msg).await {
                            error!("❌ Routing Error: {}", e);
                        }
                    });
                }
                VersEvent::ThoughtResponse {
                    agent_id,
                    content,
                    source_message_id: _,
                } => {
                    info!("🧠 Received ThoughtResponse from Agent: {}", agent_id);
                    // 思考結果をメッセージとして配信
                    let msg = vers_shared::VersMessage::new(
                        vers_shared::MessageSource::Agent(agent_id),
                        content,
                    );

                    // 全体に配信 (SSE用)
                    let _ = self
                        .tx_internal
                        .send(VersEvent::MessageReceived(msg.clone()));

                    // メモリ保存等のために再度バスに投入 (MessageReceivedとして)
                    // これにより、将来的にプラグインがこの応答にさらに反応できる
                    let _ = event_tx.send(VersEvent::MessageReceived(msg)).await;
                }
                VersEvent::ActionRequested { requester, action } => {
                    // 🛡️ Permission Enforcement Layer
                    if self.authorize(&requester, Permission::InputControl) {
                        info!("✅ Action authorized for plugin: {}", requester);
                        let _ = self
                            .tx_internal
                            .send(VersEvent::ActionRequested { requester, action });
                    } else {
                        error!("🚫 SECURITY VIOLATION: Plugin {} attempted Action without InputControl permission.", requester);
                    }
                }
                _ => {
                    // その他のイベントは全プラグインに配信
                    self.registry.dispatch_event(&event, &event_tx).await;
                    // SSE等への内部通知
                    let _ = self.tx_internal.send(event);
                }
            }
        }
    }

    fn authorize(&self, requester_id: &vers_shared::VersId, required: Permission) -> bool {
        for plugin in self.registry.plugins.values() {
            let manifest = plugin.manifest();
            if manifest.id == *requester_id {
                return manifest.required_permissions.contains(&required);
            }
        }
        false
    }
}
