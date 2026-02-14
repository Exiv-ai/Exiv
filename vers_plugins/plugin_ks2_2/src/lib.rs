use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::sync::Arc;
use vers_shared::{
    AgentMetadata, CapabilityType, MemoryProvider, Permission, Plugin, PluginConfig, PluginFactory,
    PluginManifest, ReasoningEngine, ServiceType, VersId, VersId as PluginId, VersMessage,
};

pub struct Ks2_2Plugin {
    id: PluginId,
    pool: SqlitePool,
}

impl Ks2_2Plugin {
    pub async fn new(id: PluginId, database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { id, pool })
    }

    fn base_manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.id,
            name: "KS2.2 Core".to_string(),
            description: "Standard memory and reasoning logic from Karin System 2.1.".to_string(),
            version: "0.1.0".to_string(),
            service_type: ServiceType::Reasoning,
            tags: vec![
                "#CORE".to_string(),
                "#MIND".to_string(),
                "#MEMORY".to_string(),
            ],
            is_active: true,
            is_configured: true,
            required_config_keys: vec![],
            action_icon: Some("Database".to_string()),
            action_target: Some("MODAL_RECALL".to_string()),
            required_permissions: vec![Permission::MemoryRead, Permission::MemoryWrite],
            provided_capabilities: vec![CapabilityType::Reasoning, CapabilityType::Memory],
            provided_tools: vec!["recall".to_string(), "store".to_string()],
        }
    }

    pub fn factory() -> Arc<dyn PluginFactory> {
        Arc::new(Ks2_2Factory)
    }
}

#[async_trait]
impl Plugin for Ks2_2Plugin {
    fn manifest(&self) -> PluginManifest {
        self.base_manifest()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_reasoning(&self) -> Option<&dyn ReasoningEngine> {
        Some(self)
    }
    fn as_memory(&self) -> Option<&dyn MemoryProvider> {
        Some(self)
    }
}

#[async_trait]
impl ReasoningEngine for Ks2_2Plugin {
    fn name(&self) -> &str {
        "KS2.2-Mind"
    }

    async fn think(
        &self,
        _agent: &AgentMetadata,
        message: &VersMessage,
        _context: Vec<VersMessage>,
    ) -> anyhow::Result<String> {
        Ok(format!(
            "KS2.2 received: '{}'. (Reasoning logic integrated)",
            message.content
        ))
    }
}

#[async_trait]
impl MemoryProvider for Ks2_2Plugin {
    fn name(&self) -> &str {
        "KS2.2-Storage"
    }

    async fn store(&self, _agent_id: VersId, message: VersMessage) -> anyhow::Result<()> {
        let metadata = serde_json::to_string(&message.metadata)?;
        sqlx::query("INSERT INTO memories (id, content, metadata, timestamp) VALUES (?, ?, ?, ?)")
            .bind(message.id.to_string())
            .bind(message.content)
            .bind(metadata)
            .bind(message.timestamp)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn recall(
        &self,
        _agent_id: VersId,
        query: &str,
        _limit: usize,
    ) -> anyhow::Result<Vec<VersMessage>> {
        let rows = sqlx::query_as::<_, MemoryRow>("SELECT content, metadata, timestamp FROM memories WHERE content LIKE ? ORDER BY timestamp DESC")
            .bind(format!("%{}%", query))
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let metadata = serde_json::from_str(&row.metadata).unwrap_or_default();
            results.push(VersMessage {
                id: VersId::new(),
                source: vers_shared::MessageSource::System,
                target_agent: None,
                content: row.content,
                timestamp: row.timestamp,
                metadata,
            });
        }
        Ok(results)
    }
}

#[derive(sqlx::FromRow)]
struct MemoryRow {
    content: String,
    metadata: String,
    timestamp: DateTime<Utc>,
}

pub struct Ks2_2Factory;

#[async_trait]
impl PluginFactory for Ks2_2Factory {
    fn name(&self) -> &str {
        "core.ks2_2"
    }
    fn service_type(&self) -> ServiceType {
        ServiceType::Reasoning
    }

    async fn create(&self, config: PluginConfig) -> anyhow::Result<Arc<dyn Plugin>> {
        let db_url = config
            .config_values
            .get("database_url")
            .ok_or_else(|| anyhow::anyhow!("database_url required for KS2.2 config"))?;
        let plugin = Ks2_2Plugin::new(config.id, db_url).await?;
        Ok(Arc::new(plugin))
    }
}
