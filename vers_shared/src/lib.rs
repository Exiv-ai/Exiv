use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

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
    pub id: VersId,
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
    pub required_permissions: Vec<Permission>,
    pub provided_capabilities: Vec<CapabilityType>,
    pub provided_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageSource {
    User { id: String, name: String },
    Agent(VersId),
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersMessage {
    pub id: VersId,
    pub source: MessageSource,
    pub target_agent: Option<VersId>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl VersMessage {
    pub fn new(source: MessageSource, content: String) -> Self {
        Self {
            id: VersId::new(),
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

/// 全てのプラグインが実装するベースとなるマーカートレイト
#[async_trait]
pub trait Plugin: Any + Send + Sync {
    fn manifest(&self) -> PluginManifest;

    /// システムイベントの購読（デフォルトは何もしない）
    /// 戻り値としてイベントを返すと、Kernelによって再配信される
    async fn on_event(&self, _event: &VersEvent) -> anyhow::Result<Option<VersEvent>> {
        Ok(None)
    }

    /// エージェント初期化時のフック（メタデータの注入など）
    async fn on_agent_init(&self, _agent: &mut AgentMetadata) -> anyhow::Result<()> {
        Ok(())
    }

    // Cast methods for safe trait object usage
    fn as_any(&self) -> &dyn Any;

    fn as_tool(&self) -> Option<&dyn Tool> {
        None
    }
    fn as_communication(&self) -> Option<&dyn CommunicationAdapter> {
        None
    }
    fn as_reasoning(&self) -> Option<&dyn ReasoningEngine> {
        None
    }
    fn as_memory(&self) -> Option<&dyn MemoryProvider> {
        None
    }
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
    async fn store(&self, agent_id: VersId, message: VersMessage) -> anyhow::Result<()>;
    async fn recall(
        &self,
        agent_id: VersId,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<VersMessage>>;
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
        message: VersMessage,
        context: Vec<VersMessage>,
    },
    /// プラグインからの思考結果
    ThoughtResponse {
        agent_id: VersId,
        content: String,
        source_message_id: VersId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub id: VersId,
    pub name: String,
    pub description: String,
    pub status: String,
    pub required_capabilities: Vec<CapabilityType>,
    pub plugin_bindings: Vec<VersId>,
    pub metadata: HashMap<String, String>,
}

pub struct PluginConfig {
    pub id: VersId,
    pub config_values: HashMap<String, String>,
}

#[async_trait]
pub trait PluginFactory: Send + Sync {
    fn name(&self) -> &str;
    fn service_type(&self) -> ServiceType;
    async fn create(&self, config: PluginConfig) -> anyhow::Result<Arc<dyn Plugin>>;
}
