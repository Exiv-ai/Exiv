use std::sync::Arc;
use std::collections::HashMap;
use sqlx::SqlitePool;
use tracing::{info, error};

use vers_shared::{
    AgentMetadata, MemoryProvider, Plugin, PluginConfig, PluginFactory, PluginManifest,
    ReasoningEngine, VersEvent, VersId, VersMessage,
};

pub struct PluginRegistry {
    pub plugins: HashMap<String, Arc<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn list_plugins(&self) -> Vec<PluginManifest> {
        self.plugins.values().map(|p| p.manifest()).collect()
    }

    pub fn get_engine(&self, id: &str) -> Option<&dyn ReasoningEngine> {
        self.plugins.get(id).and_then(|p| p.as_reasoning())
    }

    pub fn get_memory(&self, id: &str) -> Option<&dyn MemoryProvider> {
        self.plugins.get(id).and_then(|p| p.as_memory())
    }

    /// 全てのアクティブなプラグインにイベントを配信する
    pub async fn dispatch_event(
        &self,
        event: &VersEvent,
        event_tx: &tokio::sync::mpsc::Sender<VersEvent>,
    ) {
        // 全プラグインの on_event を並列実行
        let mut futures = Vec::new();
        for plugin in self.plugins.values() {
            futures.push(plugin.on_event(event));
        }

        // 結果は待機するが、個別のエラーで全体を止めない
        let results = futures::future::join_all(futures).await;
        for res in results {
            match res {
                Ok(Some(new_event)) => {
                    // プラグインが新しいイベントを返した場合、バスに再投入する
                    // デッドロック防止のため、送信処理は非同期化する
                    let tx = event_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tx.send(new_event).await {
                            error!("🔌 Failed to re-dispatch plugin event: {}", e);
                        }
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    error!("🔌 Plugin event dispatch error: {}", e);
                }
            }
        }
    }
}

pub struct PluginManager {
    pub pool: SqlitePool, // Made public for handlers if needed, or keep methods here
    factories: HashMap<String, Arc<dyn PluginFactory>>,
}

impl PluginManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            factories: HashMap::new(),
        }
    }

    pub fn register_factory(&mut self, factory: Arc<dyn PluginFactory>) {
        self.factories.insert(factory.name().to_string(), factory);
    }

    pub async fn initialize_all(&self) -> anyhow::Result<PluginRegistry> {
        let mut registry = PluginRegistry::new();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM plugin_settings")
            .fetch_one(&self.pool)
            .await?;

        if count.0 == 0 {
            info!("🌱 Seeding default plugin settings...");
            let defaults = vec!["core.ks2_2", "mind.deepseek", "mind.cerebras", "hal.cursor"];
            for id in defaults {
                sqlx::query(
                    "INSERT OR IGNORE INTO plugin_settings (plugin_id, is_active) VALUES (?, 1)",
                )
                .bind(id)
                .execute(&self.pool)
                .await?;
            }
        }

        let settings: Vec<(String, bool)> =
            sqlx::query_as("SELECT plugin_id, is_active FROM plugin_settings WHERE is_active = 1")
                .fetch_all(&self.pool)
                .await?;

        let config_rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT plugin_id, config_key, config_value FROM plugin_configs")
                .fetch_all(&self.pool)
                .await?;

        let mut config_map: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (pid, k, v) in config_rows {
            config_map.entry(pid).or_default().insert(k, v);
        }

        for (plugin_id_str, _) in settings {
            if let Some(factory) = self.factories.get(&plugin_id_str) {
                let config = PluginConfig {
                    id: VersId::from_name(&plugin_id_str),
                    config_values: config_map.remove(&plugin_id_str).unwrap_or_default(),
                };

                info!("🔌 Initializing plugin: {}", plugin_id_str);
                match factory.create(config).await {
                    Ok(plugin) => {
                        registry.plugins.insert(plugin_id_str.clone(), plugin);
                    }
                    Err(e) => {
                        error!("❌ Failed to initialize plugin {}: {}", plugin_id_str, e);
                    }
                }
            } else {
                error!("⚠️ No factory found for enabled plugin: {}", plugin_id_str);
            }
        }

        Ok(registry)
    }

    pub async fn get_config(&self, plugin_id: &str) -> anyhow::Result<HashMap<String, String>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT config_key, config_value FROM plugin_configs WHERE plugin_id = ?",
        )
        .bind(plugin_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn update_config(
        &self,
        plugin_id: &str,
        key: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO plugin_configs (plugin_id, config_key, config_value) VALUES (?, ?, ?)")
            .bind(plugin_id)
            .bind(key)
            .bind(value)
            .execute(&self.pool).await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct AgentRow {
    name: String,
    description: String,
    status: String,
    default_engine_id: String,
    metadata: sqlx::types::Json<HashMap<String, String>>,
}

#[derive(Clone)]
pub struct AgentManager {
    pool: SqlitePool,
}

impl AgentManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_agent_config(&self, agent_id: &str) -> anyhow::Result<(AgentMetadata, String)> {
        let row: AgentRow = sqlx::query_as(
            "SELECT name, description, status, default_engine_id, metadata FROM agents WHERE id = ?"
        )
        .bind(agent_id)
        .fetch_one(&self.pool)
        .await?;
        
        let metadata = AgentMetadata {
            id: VersId::from_name(&row.name),
            name: row.name,
            description: row.description,
            status: row.status,
            required_capabilities: vec![vers_shared::CapabilityType::Reasoning, vers_shared::CapabilityType::Memory],
            plugin_bindings: vec![],
            metadata: row.metadata.0,
        };
        
        Ok((metadata, row.default_engine_id))
    }
    
    pub async fn list_agents(&self) -> anyhow::Result<Vec<AgentMetadata>> {
         let rows: Vec<AgentRow> = sqlx::query_as(
            "SELECT name, description, status, default_engine_id, metadata FROM agents"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|row| {
            AgentMetadata {
                id: VersId::from_name(&row.name),
                name: row.name,
                description: row.description,
                status: row.status,
                required_capabilities: vec![vers_shared::CapabilityType::Reasoning, vers_shared::CapabilityType::Memory],
                plugin_bindings: vec![],
                metadata: row.metadata.0,
            }
        }).collect())
    }

    pub async fn create_agent(&self, name: &str, description: &str, default_engine: &str) -> anyhow::Result<VersId> {
        let id_str = format!("agent.{}", name.to_lowercase().replace(" ", "_"));
        let id = VersId::from_name(name); // or use uuid
        
        sqlx::query("INSERT INTO agents (id, name, description, default_engine_id, status, metadata) VALUES (?, ?, ?, ?, 'offline', '{}')")
            .bind(&id_str)
            .bind(name)
            .bind(description)
            .bind(default_engine)
            .execute(&self.pool)
            .await?;
            
        Ok(id)
    }
}

pub struct MessageRouter {
    registry: Arc<PluginRegistry>,
    agent_manager: AgentManager,
    event_tx: tokio::sync::mpsc::Sender<VersEvent>,
}

impl MessageRouter {
    pub fn new(
        registry: Arc<PluginRegistry>,
        agent_manager: AgentManager,
        event_tx: tokio::sync::mpsc::Sender<VersEvent>,
    ) -> Self {
        Self { registry, agent_manager, event_tx }
    }

    pub async fn route(&self, msg: VersMessage) -> anyhow::Result<()> {
        let target_agent_id = "agent.karin"; // Default for now

        // 1. エージェント情報の取得
        let (agent, _engine_id) = self.agent_manager.get_agent_config(target_agent_id).await?;

        // 2. メモリからのコンテキスト取得
        let memory_id = "core.ks2_2";
        let memory = self.registry.get_memory(memory_id);
        let context = if let Some(mem) = memory {
            mem.recall(agent.id, &msg.content, 10)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        // 3. メッセージ受信を全プラグインに通知
        self.registry
            .dispatch_event(&VersEvent::MessageReceived(msg.clone()), &self.event_tx)
            .await;

        // 4. 【核心】思考要求イベントを発行
        // Coreは誰が答えるかを知らず、バスに「誰かこの条件で考えてくれ」と投げる
        info!(
            "📢 Dispatching ThoughtRequested for agent '{}'...",
            agent.name
        );
        self.registry
            .dispatch_event(
                &VersEvent::ThoughtRequested {
                    agent: agent.clone(),
                    message: msg.clone(),
                    context,
                },
                &self.event_tx,
            )
            .await;

        // メモリへの保存（リクエスト分）
        if let Some(mem) = memory {
            let _ = mem.store(agent.id, msg).await;
        }

        Ok(())
    }
}
