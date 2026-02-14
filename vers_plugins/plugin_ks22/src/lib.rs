use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use vers_shared::{
    MemoryProvider, Plugin, PluginConfig, ReasoningEngine, VersMessage,
    vers_plugin, PluginRuntimeContext, PluginDataStore, SALExt
};

#[vers_plugin(
    name = "core.ks22", 
    kind = "Reasoning",
    description = "Standard memory and reasoning logic from Karin System 2.1.",
    version = "0.1.0",
    category = "Memory",
    permissions = ["MemoryRead", "MemoryWrite"],
    capabilities = ["Reasoning", "Memory"]
)]
pub struct Ks22Plugin {
    state: Arc<RwLock<Ks22State>>,
}

struct Ks22State {
    store: Option<Arc<dyn PluginDataStore>>,
}

impl Ks22Plugin {
    pub async fn new_plugin(_config: PluginConfig) -> anyhow::Result<Self> {
        Ok(Self { 
            state: Arc::new(RwLock::new(Ks22State { store: None })) 
        })
    }
}

#[async_trait]
impl Plugin for Ks22Plugin {
    fn manifest(&self) -> vers_shared::PluginManifest {
        self.auto_manifest()
    }

    async fn on_plugin_init(
        &self,
        context: PluginRuntimeContext,
        _network: Option<Arc<dyn vers_shared::NetworkCapability>>,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.store = Some(context.store);
        Ok(())
    }

    async fn on_event(&self, event: &vers_shared::VersEvent) -> anyhow::Result<Option<vers_shared::VersEventData>> {
        if let vers_shared::VersEventData::ThoughtRequested { agent, engine_id, message, context } = &event.data {
            if engine_id == "core.ks22" {
                let content = self.think(agent, message, context.clone()).await?;
                return Ok(Some(vers_shared::VersEventData::ThoughtResponse {
                    agent_id: agent.id.clone(),
                    engine_id: "core.ks22".to_string(),
                    content,
                    source_message_id: message.id.clone(),
                }));
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl ReasoningEngine for Ks22Plugin {
    fn name(&self) -> &str { "KS2.2-Mind" }

    async fn think(
        &self,
        _agent: &vers_shared::AgentMetadata,
        message: &VersMessage,
        _context: Vec<VersMessage>,
    ) -> anyhow::Result<String> {
        Ok(format!("KS2.2 received: '{}'.", message.content))
    }
}

#[async_trait]
impl MemoryProvider for Ks22Plugin {
    fn name(&self) -> &str { "KS2.2-Storage" }

    async fn store(&self, agent_id: String, message: VersMessage) -> anyhow::Result<()> {
        let state = self.state.read().await;
        let store = state.store.as_ref().ok_or_else(|| anyhow::anyhow!("Store not initialized"))?;
        
        let key = store.generate_mem_key(&agent_id, &message);
        store.save("core.ks22", &key, &message).await?;
        Ok(())
    }

    async fn recall(
        &self,
        agent_id: String,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<VersMessage>> {
        let state = self.state.read().await;
        let store = state.store.as_ref().ok_or_else(|| anyhow::anyhow!("Store not initialized"))?;
        
        let prefix = format!("mem:{}:", agent_id);
        // Kernel 側で DESC ソートされているため、最新のものが最初に来る
        let items = store.get_all_json("core.ks22", &prefix).await?;
        
        let mut messages = Vec::new();
        let query_lower = query.to_lowercase();

        for (_, value) in items {
            if let Ok(msg) = serde_json::from_value::<VersMessage>(value) {
                // 🔍 シンプルなキーワードマッチング (もしクエリがあれば)
                if query.is_empty() || msg.content.to_lowercase().contains(&query_lower) {
                    messages.push(msg);
                }
            }
            if messages.len() >= limit {
                break;
            }
        }
        
        // 🔄 LLM に渡すために時系列順（昇順）に戻す
        messages.reverse();
        Ok(messages)
    }
}