use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub use vers_macros::vers_plugin;
pub use inventory;

/// VERSプラットフォーム内での一意の識別子（Agent, Plugin, Session等）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersId(Uuid);

impl std::fmt::Display for VersId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl VersId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_name(name: &str) -> Self {
        let namespace = Uuid::NAMESPACE_DNS;
        Self(Uuid::new_v5(&namespace, name.as_bytes()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityType {
    /// 思考・推論能力 (ReasoningEngine)
    Reasoning,
    /// 記憶・永続化能力 (MemoryProvider)
    Memory,
    /// 外部通信・入出力能力 (CommunicationAdapter)
    Communication,
    /// 特定タスク実行能力 (Tool)
    Tool,
    /// 視覚・画像処理能力
    Vision,
    /// 物理/ハードウェア操作能力
    HAL,
    /// Webサーバー拡張能力 (APIエンドポイント提供)
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    VisionRead,
    InputControl,
    FileRead,
    FileWrite,
    NetworkAccess,
    ProcessExecution,
    MemoryRead,
    MemoryWrite,
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Kernelによって認可され、実行時に提供されるプラグインの権限・環境情報
#[derive(Clone)]
pub struct PluginRuntimeContext {
    pub effective_permissions: Vec<Permission>,
    pub store: Arc<dyn PluginDataStore>,
}

/// プラグインがデータを保存するための抽象ストレージインターフェース (Principle #4: Data Sovereignty / Principle #6: SAL)
#[async_trait]
pub trait PluginDataStore: Send + Sync {
    /// JSON形式でデータを保存
    async fn set_json(&self, plugin_id: &str, key: &str, value: serde_json::Value) -> anyhow::Result<()>;
    /// JSON形式でデータを取得
    async fn get_json(&self, plugin_id: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>>;
}

/// SALを型安全に利用するための拡張トレイト
#[async_trait]
pub trait SALExt: PluginDataStore {
    async fn save<T: Serialize + Sync>(&self, plugin_id: &str, key: &str, value: &T) -> anyhow::Result<()> {
        self.set_json(plugin_id, key, serde_json::to_value(value)?).await
    }

    async fn load<T: for<'de> Deserialize<'de>>(&self, plugin_id: &str, key: &str) -> anyhow::Result<Option<T>> {
        if let Some(json) = self.get_json(plugin_id, key).await? {
            Ok(Some(serde_json::from_value(json)?))
        } else {
            Ok(None)
        }
    }
}

impl<T: PluginDataStore + ?Sized> SALExt for T {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[async_trait]
pub trait NetworkCapability: Send + Sync {
    async fn send_http_request(&self, request: HttpRequest) -> anyhow::Result<HttpResponse>;
}

/// 実行時に注入される具体的な能力のラッパー
#[derive(Clone)]
pub enum PluginCapability {
    Network(Arc<dyn NetworkCapability>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceType {
    Communication,
    Reasoning,
    Skill,
    Vision,
    Action,
    Memory,
    HAL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub service_type: ServiceType,
    pub tags: Vec<String>,
    pub is_active: bool,
    pub is_configured: bool,
    pub required_config_keys: Vec<String>,
    pub action_icon: Option<String>,
    pub action_target: Option<String>,
    pub icon_data: Option<String>,
    pub magic_seal: u32,
    pub sdk_version: String,
    pub required_permissions: Vec<Permission>,
    pub provided_capabilities: Vec<CapabilityType>,
    pub provided_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageSource {
    User { id: String, name: String },
    Agent { id: String },
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersMessage {
    pub id: String,
    pub source: MessageSource,
    pub target_agent: Option<String>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl VersMessage {
    pub fn new(source: MessageSource, content: String) -> Self {
        Self {
            id: VersId::new().to_string(),
            source,
            target_agent: None,
            content,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandAction {
    MouseMove { x: i32, y: i32 },
    MouseClick { button: String },
    KeyPress { key: String },
    Wait { ms: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorVisionData {
    pub captured_at: DateTime<Utc>,
    pub detected_elements: Vec<DetectedElement>,
    pub image_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedElement {
    pub label: String,
    pub bounds: (i32, i32, i32, i32), // x, y, w, h
    pub confidence: f32,
    pub attributes: HashMap<String, String>,
}

/// プラグインのダウンキャストを補助するためのトレイト
pub trait PluginCast {
    fn as_any(&self) -> &dyn Any;
    fn as_tool(&self) -> Option<&dyn Tool> { None }
    fn as_communication(&self) -> Option<&dyn CommunicationAdapter> { None }
    fn as_reasoning(&self) -> Option<&dyn ReasoningEngine> { None }
    fn as_memory(&self) -> Option<&dyn MemoryProvider> { None }
    fn as_web(&self) -> Option<&dyn WebPlugin> { None }
}

/// 全てのプラグインが実装するベースとなるマーカートレイト
#[async_trait]
pub trait Plugin: Any + Send + Sync + PluginCast {
    fn manifest(&self) -> PluginManifest;

    /// プラグイン自体の初期化（権限の割り当てなど）
    async fn on_plugin_init(
        &self,
        _context: PluginRuntimeContext,
        _network: Option<Arc<dyn NetworkCapability>>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// システムイベントの購読（デフォルトは何もしない）
    /// 戻り値としてイベントを返すと、Kernelによって再配信される
    async fn on_event(&self, _event: &VersEvent) -> anyhow::Result<Option<VersEvent>> {
        Ok(None)
    }

    /// エージェント初期化時のフック（メタデータの注入など）
    async fn on_agent_init(&self, _agent: &mut AgentMetadata) -> anyhow::Result<()> {
        Ok(())
    }

    /// 実行中に権限が承認され、Capabilityが注入された際のフック
    async fn on_capability_injected(
        &self,
        _capability: PluginCapability,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

pub trait WebPlugin: Plugin {
    fn register_routes(&self, router: axum::Router<Arc<dyn Any + Send + Sync>>) -> axum::Router<Arc<dyn Any + Send + Sync>>;
}

#[async_trait]
pub trait Tool: Plugin {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

#[async_trait]
pub trait CommunicationAdapter: Plugin {
    fn name(&self) -> &str;
    async fn start(&self, event_sender: tokio::sync::mpsc::Sender<VersEvent>)
        -> anyhow::Result<()>;
    async fn send(&self, target_user_id: &str, content: &str) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ReasoningEngine: Plugin {
    fn name(&self) -> &str;
    async fn think(
        &self,
        agent: &AgentMetadata,
        message: &VersMessage,
        context: Vec<VersMessage>,
    ) -> anyhow::Result<String>;
}

#[async_trait]
pub trait MemoryProvider: Plugin {
    fn name(&self) -> &str;
    async fn store(&self, agent_id: String, message: VersMessage) -> anyhow::Result<()>;
    async fn recall(
        &self,
        agent_id: String,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<VersMessage>>;
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &VersEvent) -> anyhow::Result<Option<VersEvent>>;
}

pub struct EventRouter {
    pub handlers: HashMap<String, Box<dyn EventHandler>>,
}

impl EventRouter {
    pub fn new() -> Self {
        Self { handlers: HashMap::new() }
    }

    pub fn on<F, Fut>(&mut self, event_type: &str, handler: F)
    where
        F: Fn(VersEvent) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = anyhow::Result<Option<VersEvent>>> + Send + 'static,
    {
        struct FuncHandler<F>(F);
        #[async_trait]
        impl<F, Fut> EventHandler for FuncHandler<F>
        where
            F: Fn(VersEvent) -> Fut + Send + Sync + 'static,
            Fut: std::future::Future<Output = anyhow::Result<Option<VersEvent>>> + Send + 'static,
        {
            async fn handle(&self, event: &VersEvent) -> anyhow::Result<Option<VersEvent>> {
                (self.0)(event.clone()).await
            }
        }
        self.handlers.insert(event_type.to_string(), Box::new(FuncHandler(handler)));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VersEvent {
    MessageReceived(VersMessage),
    VisionUpdated(ColorVisionData),
    /// プラグインからのアクション要求（権限チェック対象）
    ActionRequested {
        requester: VersId,
        action: HandAction,
    },
    SystemNotification(String),
    /// プラグインに対して思考（推論）を要求する
    ThoughtRequested {
        agent: AgentMetadata,
        engine_id: String,
        message: VersMessage,
        context: Vec<VersMessage>,
    },
    /// プラグインからの思考結果
    ThoughtResponse {
        agent_id: String,
        content: String,
        source_message_id: String,
    },
    /// プラグインの設定が更新された通知
    ConfigUpdated {
        plugin_id: String,
        config: std::collections::HashMap<String, String>,
    },
    /// プラグインからの権限要求
    PermissionRequested {
        plugin_id: String,
        permission: Permission,
        reason: String,
    },
    /// 権限が承認された通知
    PermissionGranted {
        plugin_id: String,
        permission: Permission,
    },
    /// マニフェストが更新された通知
    ManifestUpdated {
        plugin_id: String,
        new_manifest: PluginManifest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub required_capabilities: Vec<CapabilityType>,
    pub plugin_bindings: Vec<VersId>,
    pub metadata: HashMap<String, String>,
}

pub struct PluginConfig {
    pub id: String,
    pub config_values: std::collections::HashMap<String, String>,
}

#[async_trait]
pub trait PluginFactory: Send + Sync {
    fn name(&self) -> &str;
    fn service_type(&self) -> ServiceType;
    async fn create(&self, config: PluginConfig) -> anyhow::Result<Arc<dyn Plugin>>;
}

pub struct PluginRegistrar {
    pub factory: fn() -> Arc<dyn PluginFactory>,
}

inventory::collect!(PluginRegistrar);
