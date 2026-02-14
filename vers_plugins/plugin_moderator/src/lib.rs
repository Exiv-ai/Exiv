use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use vers_shared::{
    Plugin, PluginConfig, vers_plugin, VersEvent, VersEventData, VersId,
    VersMessage, MessageSource, AgentMetadata
};

#[vers_plugin(
    name = "core.moderator",
    kind = "Reasoning",
    description = "Consensus Moderator for collective intelligence.",
    version = "0.2.0",
    category = "Tool",
    tags = ["#TOOL"],
    capabilities = ["Reasoning"]
)]
pub struct ModeratorPlugin {
    /// trace_id -> SessionState
    sessions: Arc<RwLock<HashMap<VersId, SessionState>>>,
}

enum SessionState {
    Collecting(Vec<Proposal>),
    Synthesizing,
}

struct Proposal {
    #[allow(dead_code)]
    engine_id: String,
    content: String,
}

impl ModeratorPlugin {
    pub async fn new_plugin(_config: PluginConfig) -> anyhow::Result<Self> {
        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl vers_shared::ReasoningEngine for ModeratorPlugin {
    fn name(&self) -> &str { "ConsensusModerator" }
    async fn think(&self, _agent: &AgentMetadata, _msg: &VersMessage, _ctx: Vec<VersMessage>) -> anyhow::Result<String> {
        Ok("I am observing the consensus process.".to_string())
    }
}

#[async_trait]
impl Plugin for ModeratorPlugin {
    fn manifest(&self) -> vers_shared::PluginManifest {
        self.auto_manifest()
    }

    async fn on_event(&self, event: &VersEvent) -> anyhow::Result<Option<VersEventData>> {
        match &event.data {
            VersEventData::ConsensusRequested { task: _, engine_ids } => {
                tracing::info!(trace_id = %event.trace_id, "🤝 Consensus process started for {} engines", engine_ids.len());
                
                // セッションの初期化 (Collecting Phase)
                {
                    let mut sessions = self.sessions.write().await;
                    sessions.insert(event.trace_id, SessionState::Collecting(Vec::new()));
                }
                
                return Ok(None);
            }

            VersEventData::ThoughtResponse { agent_id, content, source_message_id: _ } => {
                // 自分自身(Moderator)や、統合エンジン(Synthesizer)からの回答を区別する必要がある
                if agent_id == "system.consensus" {
                    return Ok(None);
                }

                let mut sessions = self.sessions.write().await;
                
                // 状態遷移のためのフラグ
                let mut start_synthesis = false;
                let mut synthesis_prompt = String::new();

                if let Some(state) = sessions.get_mut(&event.trace_id) {
                    match state {
                        SessionState::Collecting(proposals) => {
                            // 1. 提案の収集
                            proposals.push(Proposal {
                                engine_id: agent_id.clone(),
                                content: content.clone(),
                            });

                            tracing::info!(trace_id = %event.trace_id, "📥 Collected proposal from {} ({}/{})", agent_id, proposals.len(), 2);

                            if proposals.len() >= 2 {
                                // プロンプト作成用データを先に退避
                                let combined_views = proposals.iter().enumerate()
                                    .map(|(i, p)| format!("## Opinion {}:\n{}", i + 1, p.content))
                                    .collect::<Vec<_>>().join("\n\n");
                                
                                // 2. 統合フェーズへ移行
                                *state = SessionState::Synthesizing;
                                start_synthesis = true;
                                    
                                synthesis_prompt = format!(
                                    "You are a wise moderator. Synthesize the following opinions into a single, coherent conclusion.\n\n{}",
                                    combined_views
                                );
                            }
                        }
                        SessionState::Synthesizing => {
                            // 3. 統合回答の受信 (Final Phase)
                            tracing::info!(trace_id = %event.trace_id, "🏁 Synthesis complete via {}", agent_id);
                            
                            // セッション終了
                            sessions.remove(&event.trace_id);
                            
                            return Ok(Some(VersEvent::with_trace(
                                event.trace_id,
                                VersEventData::ThoughtResponse {
                                    agent_id: "system.consensus".to_string(),
                                    content: content.clone(),
                                    source_message_id: "consensus".to_string(),
                                }
                            ).data));
                        }
                    }
                }

                // ロックを解放してからイベント発行
                if start_synthesis {
                    tracing::info!(trace_id = %event.trace_id, "⚗️ Starting synthesis phase...");
                    
                    // ダミーのエージェントメタデータ (統合用)
                    let synthesizer_agent = AgentMetadata {
                        id: "agent.synthesizer".to_string(),
                        name: "Synthesizer".to_string(),
                        description: "AI Moderator".to_string(),
                        status: "online".to_string(),
                        default_engine_id: Some("mind.deepseek".to_string()),
                        required_capabilities: vec![],
                        plugin_bindings: vec![],
                        metadata: HashMap::new(),
                    };

                    return Ok(Some(VersEvent::with_trace(
                        event.trace_id,
                        VersEventData::ThoughtRequested {
                            agent: synthesizer_agent,
                            engine_id: "mind.deepseek".to_string(), // DeepSeekに統合を依頼
                            message: VersMessage::new(MessageSource::System, synthesis_prompt),
                            context: vec![],
                        }
                    ).data));
                }
            }
            _ => {}
        }
        Ok(None)
    }
}
