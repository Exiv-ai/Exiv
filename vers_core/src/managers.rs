use std::sync::Arc;
use std::collections::HashMap;
use sqlx::SqlitePool;
use tracing::{info, error};

use vers_shared::{
    AgentMetadata, MemoryProvider, Plugin, PluginConfig, PluginFactory, PluginManifest,
    ReasoningEngine, VersEvent, VersId, VersMessage, Permission, PluginRuntimeContext
};
use crate::capabilities::SafeHttpClient;

#[derive(sqlx::FromRow, Debug)]
pub struct PluginSetting {
    pub plugin_id: String,
    pub is_active: bool,
    pub allowed_permissions: sqlx::types::Json<Vec<Permission>>,
}

pub struct PluginRegistry {
    pub plugins: HashMap<String, Arc<dyn Plugin>>,
    pub internal_handlers: tokio::sync::RwLock<Vec<Arc<SystemHandler>>>,
    pub effective_permissions: tokio::sync::RwLock<HashMap<VersId, Vec<Permission>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            internal_handlers: tokio::sync::RwLock::new(Vec::new()),
            effective_permissions: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    pub async fn add_internal_handler(&self, handler: Arc<SystemHandler>) {
        let mut handlers = self.internal_handlers.write().await;
        handlers.push(handler);
    }

    pub async fn update_effective_permissions(&self, plugin_id: VersId, permission: Permission) {
        let mut perms_lock = self.effective_permissions.write().await;
        perms_lock.entry(plugin_id).or_default().push(permission);
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

    pub fn find_memory(&self) -> Option<&dyn MemoryProvider> {
        for plugin in self.plugins.values() {
            if let Some(mem) = plugin.as_memory() {
                return Some(mem);
            }
        }
        None
    }

    /// 全てのアクティブなプラグイン（および内部ハンドラ）にイベントを配信する
    pub async fn dispatch_event(
        &self,
        event: &VersEvent,
        event_tx: &tokio::sync::mpsc::Sender<VersEvent>,
    ) {
        // 1. 内部ハンドラの処理 (System Core Logic)
        {
            let handlers = self.internal_handlers.read().await;
            for handler in &*handlers {
                if let VersEvent::MessageReceived(msg) = event {
                    let handler = handler.clone();
                    let msg = msg.clone();
                    let tx = event_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handler.handle_message(msg, &tx).await {
                            error!("❌ SystemHandler Error: {}", e);
                        }
                    });
                }
            }
        }

        // 2. 外部プラグインの処理 (Parallel)
        let mut handles = Vec::new();
        for (id, plugin) in &self.plugins {
            let plugin = plugin.clone();
            let event = event.clone();
            let id = id.clone();
            handles.push((id, tokio::spawn(async move {
                plugin.on_event(&event).await
            })));
        }

        // 結果を待機
        for (id, handle) in handles {
            match handle.await {
                Ok(Ok(Some(new_event))) => {
                    let tx = event_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tx.send(new_event).await {
                            error!("🔌 Failed to re-dispatch plugin event: {}", e);
                        }
                    });
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => {
                    error!("🔌 Plugin {} on_event error: {}", id, e);
                }
                Err(e) => {
                    error!("🔥 Plugin {} PANICKED during event processing: {}", id, e);
                }
            }
        }
    }
}

pub struct PluginManager {
    pub pool: SqlitePool, // Made public for handlers if needed, or keep methods here
    factories: HashMap<String, Arc<dyn PluginFactory>>,
    http_client: Arc<SafeHttpClient>,
    store: Arc<crate::db::SqliteDataStore>,
}

impl PluginManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: pool.clone(),
            factories: HashMap::new(),
            http_client: Arc::new(SafeHttpClient::new()),
            store: Arc::new(crate::db::SqliteDataStore::new(pool)),
        }
    }

    pub fn register_factory(&mut self, factory: Arc<dyn PluginFactory>) {
        self.factories.insert(factory.name().to_string(), factory);
    }

    /// Register all built-in plugins (Principle #1: Core Minimalism / Dynamic Bootstrap)
    pub fn register_builtins(&mut self) {
        info!("🔍 Scanning for plugins via inventory...");
        for registrar in vers_shared::inventory::iter::<vers_shared::PluginRegistrar> {
            let factory = (registrar.factory)();
            info!("📦 Discovered plugin factory: {}", factory.name());
            self.register_factory(factory);
        }
    }

    pub async fn initialize_all(&self) -> anyhow::Result<PluginRegistry> {
        let mut registry = PluginRegistry::new();
        let (settings, mut config_map) = self.fetch_plugin_configs().await?;

        for setting in settings {
            let pid = setting.plugin_id.clone();
            let config_values = config_map.remove(&pid).unwrap_or_default();
            
            if let Err(e) = self.bootstrap_plugin(setting, config_values, &mut registry).await {
                error!("❌ Failed to bootstrap plugin {}: {}", pid, e);
            }
        }

        Ok(registry)
    }

    async fn fetch_plugin_configs(&self) -> anyhow::Result<(Vec<PluginSetting>, HashMap<String, HashMap<String, String>>)> {
        let settings: Vec<PluginSetting> =
            sqlx::query_as("SELECT plugin_id, is_active, allowed_permissions FROM plugin_settings WHERE is_active = 1")
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

        Ok((settings, config_map))
    }

    async fn bootstrap_plugin(
        &self,
        setting: PluginSetting,
        config_values: HashMap<String, String>,
        registry: &mut PluginRegistry,
    ) -> anyhow::Result<()> {
        let plugin_id_str = &setting.plugin_id;
        let factory = self.factories.get(plugin_id_str)
            .ok_or_else(|| anyhow::anyhow!("No factory found for enabled plugin: {}", plugin_id_str))?;

        let config = PluginConfig {
            id: plugin_id_str.clone(),
            config_values,
        };

        info!("🔌 Initializing plugin: {}", plugin_id_str);
        let plugin = factory.create(config).await?;
        let manifest = plugin.manifest();
        
        // 🛂 入国審査 (Magic Seal Validation)
        if manifest.magic_seal != 0x56455253 {
            return Err(anyhow::anyhow!("Access Denied: Plugin '{}' is not compiled with official SDK", manifest.name));
        }
        
        info!("✅ Plugin '{}' (SDK v{}) verified.", manifest.name, manifest.sdk_version);

        // 🔐 権限の注入 (Principles #5)
        let permissions = setting.allowed_permissions.0;
        let context = PluginRuntimeContext {
            effective_permissions: permissions.clone(),
            store: self.store.clone(),
        };
        
        // 💉 Capability Injection
        let network_capability = if permissions.contains(&Permission::NetworkAccess) {
            self.get_capability_for_permission(&Permission::NetworkAccess)
                .and_then(|c| {
                    if let vers_shared::PluginCapability::Network(net) = c {
                        Some(net)
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        plugin.on_plugin_init(context, network_capability).await?;
        
        registry.plugins.insert(plugin_id_str.clone(), plugin.clone());
        {
            let mut perms_lock = registry.effective_permissions.write().await;
            perms_lock.insert(VersId::from_name(&manifest.id), permissions);
        }

        Ok(())
    }

    pub fn get_capability_for_permission(&self, permission: &Permission) -> Option<vers_shared::PluginCapability> {
        match permission {
            Permission::NetworkAccess => Some(vers_shared::PluginCapability::Network(self.http_client.clone())),
            _ => None,
        }
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

    pub async fn list_plugins_with_settings(&self, registry: &PluginRegistry) -> anyhow::Result<Vec<PluginManifest>> {
        let rows: Vec<PluginSetting> = sqlx::query_as("SELECT * FROM plugin_settings")
            .fetch_all(&self.pool)
            .await?;
        
        let settings: HashMap<String, bool> = rows.into_iter()
            .map(|s| (s.plugin_id, s.is_active))
            .collect();
            
        let mut manifests = registry.list_plugins();
        for m in &mut manifests {
            // Note: In a real system, we'd use a better mapping than ID string
            if let Some(&active) = settings.get(&m.name.to_lowercase().replace(" ", ".")) {
                m.is_active = active;
            }
        }
        Ok(manifests)
    }

    pub async fn apply_settings(&self, settings: Vec<(String, bool)>) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        for (id, active) in settings {
             sqlx::query("UPDATE plugin_settings SET is_active = ? WHERE plugin_id = ?")
                .bind(active)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn grant_permission(&self, plugin_id: &str, permission: vers_shared::Permission) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;

        let row: (sqlx::types::Json<Vec<vers_shared::Permission>>,) = sqlx::query_as("SELECT allowed_permissions FROM plugin_settings WHERE plugin_id = ?")
            .bind(plugin_id)
            .fetch_one(&mut *tx)
            .await?;
        
        let mut perms = row.0.0;
        if !perms.contains(&permission) {
            perms.push(permission);
            let perms_json = sqlx::types::Json(perms);
            sqlx::query("UPDATE plugin_settings SET allowed_permissions = ? WHERE plugin_id = ?")
                .bind(perms_json)
                .bind(plugin_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct AgentRow {
    id: String,
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
            "SELECT id, name, description, status, default_engine_id, metadata FROM agents WHERE id = ?"
        )
        .bind(agent_id)
        .fetch_one(&self.pool)
        .await?;
        
        let metadata = AgentMetadata {
            id: row.id,
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
            "SELECT id, name, description, status, default_engine_id, metadata FROM agents"
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|row| {
            AgentMetadata {
                id: row.id,
                name: row.name,
                description: row.description,
                status: row.status,
                required_capabilities: vec![vers_shared::CapabilityType::Reasoning, vers_shared::CapabilityType::Memory],
                plugin_bindings: vec![],
                metadata: row.metadata.0,
            }
        }).collect())
    }

    pub async fn create_agent(
        &self,
        name: &str,
        description: &str,
        default_engine: &str,
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<VersId> {
        let id_str = format!("agent.{}", name.to_lowercase().replace(" ", "_"));
        let id = VersId::from_name(name);
        let metadata_json = serde_json::to_string(&metadata)?;

        sqlx::query("INSERT INTO agents (id, name, description, default_engine_id, status, metadata) VALUES (?, ?, ?, ?, 'offline', ?)")
            .bind(&id_str)
            .bind(name)
            .bind(description)
            .bind(default_engine)
            .bind(metadata_json)
            .execute(&self.pool)
            .await?;

        Ok(id)
    }

    pub async fn update_agent_config(
        &self,
        agent_id: &str,
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let metadata_json = serde_json::to_string(&metadata)?;
        sqlx::query("UPDATE agents SET metadata = ? WHERE id = ?")
            .bind(metadata_json)
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub struct SystemHandler {
    registry: Arc<PluginRegistry>,
    agent_manager: AgentManager,
    default_agent_id: String,
}

impl SystemHandler {
    pub fn new(
        registry: Arc<PluginRegistry>,
        agent_manager: AgentManager,
        default_agent_id: String,
    ) -> Self {
        Self { registry, agent_manager, default_agent_id }
    }

    pub async fn handle_message(&self, msg: VersMessage, event_tx: &tokio::sync::mpsc::Sender<VersEvent>) -> anyhow::Result<()> {
        let target_agent_id = msg.metadata.get("target_agent_id")
            .cloned()
            .unwrap_or_else(|| self.default_agent_id.clone());

        // 1. エージェント情報の取得
        let (agent, _engine_id) = self.agent_manager.get_agent_config(&target_agent_id).await?;

        // 2. メモリからのコンテキスト取得
        let memory = if let Some(preferred_id) = agent.metadata.get("preferred_memory") {
            self.registry.get_memory(preferred_id)
        } else {
            self.registry.find_memory()
        };

        let context = if let Some(mem) = memory {
            mem.recall(agent.id.clone(), &msg.content, 10)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        // 3. 【核心】思考要求イベントを発行
        info!(
            "📢 SystemHandler: Dispatching ThoughtRequested for agent '{}' with engine '{}'...",
            agent.name, _engine_id
        );
        let thought_event = VersEvent::ThoughtRequested {
            agent: agent.clone(),
            engine_id: _engine_id,
            message: msg.clone(),
            context,
        };
        
        // バスに再投入
        if let Err(e) = event_tx.send(thought_event).await {
            error!("❌ Failed to dispatch ThoughtRequested: {}", e);
        }

        // メモリへの保存
        if let Some(mem) = memory {
            let _ = mem.store(agent.id.clone(), msg).await;
        }

        Ok(())
    }
}

impl vers_shared::PluginCast for SystemHandler {
    fn as_any(&self) -> &dyn std::any::Any { self }
}

#[async_trait::async_trait]
impl Plugin for SystemHandler {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "core.system".to_string(),
            name: "Kernel System Handler".to_string(),
            description: "Internal core logic handler".to_string(),
            version: "1.0.0".to_string(),
            service_type: vers_shared::ServiceType::Reasoning,
            tags: vec![],
            is_active: true,
            is_configured: true,
            required_config_keys: vec![],
            action_icon: None,
            action_target: None,
            icon_data: None,
            magic_seal: 0x56455253,
            sdk_version: "internal".to_string(),
            required_permissions: vec![],
            provided_capabilities: vec![],
            provided_tools: vec![],
        }
    }

    async fn on_event(&self, _event: &VersEvent) -> anyhow::Result<Option<VersEvent>> {
        Ok(None)
    }
}
