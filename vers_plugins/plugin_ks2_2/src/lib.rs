use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use vers_shared::{
    MemoryProvider, Plugin, PluginConfig, ReasoningEngine, VersMessage,
    vers_plugin, PluginRuntimeContext, PluginDataStore, SALExt
};

#[vers_plugin(
    name = "core.ks2_2", 
    kind = "Reasoning",
    description = "Standard memory and reasoning logic from Karin System 2.1.",
    version = "0.1.0",
    permissions = ["MemoryRead", "MemoryWrite"],
    capabilities = ["Reasoning", "Memory"]
)]
pub struct Ks2_2Plugin {
    id: String,
    store: Arc<RwLock<Option<Arc<dyn PluginDataStore>>>>,
}

impl Ks2_2Plugin {
    pub async fn new_plugin(config: PluginConfig) -> anyhow::Result<Self> {
        Ok(Self { 
            id: config.id, 
            store: Arc::new(RwLock::new(None)) 
        })
    }
}

#[async_trait]
impl Plugin for Ks2_2Plugin {
    fn manifest(&self) -> vers_shared::PluginManifest {
        self.auto_manifest()
    }

    async fn on_plugin_init(
        &self,
        context: PluginRuntimeContext,
        _network: Option<Arc<dyn vers_shared::NetworkCapability>>,
    ) -> anyhow::Result<()> {
        let mut store = self.store.write().await;
        *store = Some(context.store);
        Ok(())
    }
}

#[async_trait]
impl ReasoningEngine for Ks2_2Plugin {
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
impl MemoryProvider for Ks2_2Plugin {
    fn name(&self) -> &str { "KS2.2-Storage" }

    async fn store(&self, agent_id: String, message: VersMessage) -> anyhow::Result<()> {
        let store_guard = self.store.read().await;
        let store = store_guard.as_ref().ok_or_else(|| anyhow::anyhow!("Store not initialized"))?;
        
        let key = format!("mem:{}:{}", agent_id, message.id);
        // 型安全な save を使用
        store.save("core.ks2_2", &key, &message).await?;
        Ok(())
    }

    async fn recall(
        &self,
        _agent_id: String,
        _query: &str,
        _limit: usize,
    ) -> anyhow::Result<Vec<VersMessage>> {
        Ok(vec![])
    }
}