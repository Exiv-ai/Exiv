use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use std::collections::HashMap;

/// VERSプラットフォーム内での一意の識別子（Agent, Plugin, Session等）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}

/// エージェントやプラグインが要求・提供する「権限/能力」
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// 視覚情報へのアクセス (Color)
    VisionRead,
    /// 入力デバイスの操作 (Hand)
    InputControl,
    /// ファイルシステムへのアクセス
    FileRead,
    FileWrite,
    /// ネットワーク通信
    NetworkAccess,
    /// サブプロセスの実行（コンパイラ等）
    ProcessExecution,
    /// 長期記憶へのアクセス (Karin)
    MemoryRead,
    MemoryWrite,
}

/// プラグインが提供する「サービス」の分類
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceType {
    /// 通信アダプター (Discord, Slack等)
    Communication,
    /// 思考エンジン (Chat, Reasoning, Research)
    Reasoning,
    /// ツール/技能 (Search, Calculation, Scraping)
    Skill,
    /// 視覚フレームワーク (Color)
    Vision,
    /// 操作フレームワーク (Hand)
    Action,
    /// 記憶システム (Karin)
    Memory,
}

/// プラグインの自己紹介書
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: VersId,
    pub name: String,
    pub version: String,
    pub service_type: ServiceType,
    /// このプラグインが動作するために必要な権限
    pub required_capabilities: Vec<Capability>,
    /// このプラグインが提供する具体的な機能（Tool）のリスト
    pub provided_tools: Vec<String>,
}

/// メッセージの送信元
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageSource {
    User { id: String, name: String },
    Agent(VersId),
    System,
}

/// プラットフォーム内を流れる標準メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersMessage {
    pub id: VersId,
    pub source: MessageSource,
    pub target_agent: Option<VersId>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    /// メタデータ（感情状態、信頼スコアなど）
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

/// 汎用操作フレームワーク「Hand」のための抽象アクション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandAction {
    MouseMove { x: i32, y: i32 },
    MouseClick { button: String },
    KeyPress { key: String },
    Wait { ms: u32 },
}

/// 汎用視覚フレームワーク「Color」のための抽象データ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorVisionData {
    /// タイムスタンプ
    pub captured_at: DateTime<Utc>,
    /// 認識されたオブジェクトやUI要素のリスト（セマンティック情報）
    pub detected_elements: Vec<DetectedElement>,
    /// 画像データの参照（必要に応じて）
    pub image_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedElement {
    pub label: String,
    pub bounds: (i32, i32, i32, i32), // x, y, w, h
    pub confidence: f32,
    pub attributes: HashMap<String, String>,
}

/// 全てのプラグインツールが実装すべきインターフェース
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value>;
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

/// 外部通信アダプターのトレイト
#[async_trait]
pub trait CommunicationAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, event_sender: tokio::sync::mpsc::Sender<VersEvent>) -> anyhow::Result<()>;
    async fn send(&self, target_user_id: &str, content: &str) -> anyhow::Result<()>;
}

/// 思考エンジン（LLM連携）のトレイト
#[async_trait]
pub trait ReasoningEngine: Send + Sync {
    fn name(&self) -> &str;
    /// メッセージと文脈を受け取り、回答を生成する
    async fn think(&self, agent: &AgentMetadata, message: &VersMessage, context: Vec<VersMessage>) -> anyhow::Result<String>;
}

/// 記憶システム（Karin KS2.5等）のトレイト
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;
    /// 記憶の保存
    async fn store(&self, agent_id: VersId, message: VersMessage) -> anyhow::Result<()>;
    /// 関連する記憶の検索
    async fn recall(&self, agent_id: VersId, query: &str, limit: usize) -> anyhow::Result<Vec<VersMessage>>;
}

/// システム内を流れるイベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersEvent {
    /// メッセージ受信
    MessageReceived(VersMessage),
    /// 視覚情報の更新 (Colorからのイベント)
    VisionUpdated(ColorVisionData),
    /// 操作命令の発生 (Handへのリクエスト)
    ActionRequested(HandAction),
    /// システム通知
    SystemNotification(String),
}

/// エージェント（人格）の定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub id: VersId,
    pub name: String,
    pub description: String,
    /// このエージェントが持つ権限
    pub capabilities: Vec<Capability>,
    pub plugin_bindings: Vec<VersId>,
}