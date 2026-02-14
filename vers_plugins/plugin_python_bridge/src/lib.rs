use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{timeout, Duration};
use std::process::Stdio;
use vers_shared::{
    AgentMetadata, Plugin, PluginConfig, ReasoningEngine, VersMessage, PluginRuntimeContext, 
    vers_plugin, NetworkCapability
};
use tracing::info;

#[vers_plugin(
    name = "bridge.python",
    kind = "Reasoning",
    description = "Universal Python Bridge using subprocess communication. Supports PyTorch/TensorFlow natively.",
    version = "0.2.0",
    permissions = ["NetworkAccess", "FileRead"],
    capabilities = ["Reasoning"]
)]
pub struct PythonBridgePlugin {
    id: String,
    script_path: String,
    process: Arc<RwLock<Option<PythonProcess>>>,
    dynamic_manifest: Arc<RwLock<Option<vers_shared::PluginManifest>>>,
    allowed_permissions: Arc<RwLock<Vec<vers_shared::Permission>>>,
    http_client: Arc<RwLock<Option<Arc<dyn NetworkCapability>>>>,
}

struct PythonProcess {
    #[allow(dead_code)]
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl PythonBridgePlugin {
    pub async fn new_plugin(config: PluginConfig) -> anyhow::Result<Self> {
        let script_path = config.config_values.get("script_path")
            .cloned()
            .unwrap_or_else(|| "scripts/bridge_main.py".to_string());
        
        Ok(Self {
            id: config.id,
            script_path,
            process: Arc::new(RwLock::new(None)),
            dynamic_manifest: Arc::new(RwLock::new(None)),
            allowed_permissions: Arc::new(RwLock::new(vec![])),
            http_client: Arc::new(RwLock::new(None)),
        })
    }

    async fn ensure_process(&self) -> anyhow::Result<()> {
        // 1. Check if process exists (Read Lock)
        {
            let lock = self.process.read().await;
            if lock.is_some() {
                return Ok(());
            }
        }

        // 2. Spawn process (Write Lock)
        let mut lock = self.process.write().await;
        if lock.is_none() {
            info!("🐍 Spawning Python subprocess: scripts/bridge_runtime.py with user script: {}", self.script_path);
            
            let mut child = Command::new("python3")
                .arg("scripts/bridge_runtime.py")
                .arg(&self.script_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?;

            let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
            let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
            
            let mut proc = PythonProcess {
                child,
                stdin,
                stdout: BufReader::new(stdout),
            };

            // 3. Load manifest using the RAW process before storing it (to avoid recursion)
            info!("🐍 Loading dynamic manifest from Python...");
            let manifest_json = timeout(
                Duration::from_secs(5),
                Self::communicate_raw(&mut proc.stdin, &mut proc.stdout, "get_manifest", serde_json::Value::Null)
            ).await??;
            
            let mut manifest = self.auto_manifest();
            if let Some(id) = manifest_json.get("id").and_then(|v| v.as_str()) {
                manifest.id = id.to_string();
            }
            if let Some(name) = manifest_json.get("name").and_then(|v| v.as_str()) {
                manifest.name = name.to_string();
            }
            if let Some(desc) = manifest_json.get("description").and_then(|v| v.as_str()) {
                manifest.description = desc.to_string();
            }
            if let Some(ver) = manifest_json.get("version").and_then(|v| v.as_str()) {
                manifest.version = ver.to_string();
            }
            
            info!("✅ Python Manifest loaded: {}", manifest.name);
            let mut dynamic = self.dynamic_manifest.write().await;
            *dynamic = Some(manifest);

            *lock = Some(proc);
        }
        Ok(())
    }

    async fn communicate_raw(
        stdin: &mut tokio::process::ChildStdin,
        stdout: &mut BufReader<tokio::process::ChildStdout>,
        method: &str,
        params: serde_json::Value
    ) -> anyhow::Result<serde_json::Value> {
        let request = serde_json::json!({
            "method": method,
            "params": params
        });

        let mut line = request.to_string();
        line.push('\n');

        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;

        let mut response_line = String::new();
        if stdout.read_line(&mut response_line).await? == 0 {
            return Err(anyhow::anyhow!("Python process disconnected"));
        }

        let response: serde_json::Value = serde_json::from_str(&response_line)?;
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("Python Error: {}", error));
        }

        Ok(response.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }

    async fn call_python(&self, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        self.ensure_process().await?;
        
        let mut lock = self.process.write().await;
        if let Some(proc) = lock.as_mut() {
            let result = timeout(
                Duration::from_secs(10),
                Self::communicate_raw(&mut proc.stdin, &mut proc.stdout, method, params)
            ).await;

            match result {
                Ok(Ok(val)) => Ok(val),
                Ok(Err(e)) => {
                    info!("❌ Python Bridge communication error: {}. Resetting process.", e);
                    *lock = None;
                    Err(e)
                }
                Err(_) => {
                    info!("⏳ Python Bridge call timed out. Resetting process.");
                    let _ = proc.child.kill().await;
                    *lock = None;
                    Err(anyhow::anyhow!("Python Bridge timeout"))
                }
            }
        } else {
            Err(anyhow::anyhow!("Python process not available"))
        }
    }
}

#[async_trait]
impl Plugin for PythonBridgePlugin {
    fn manifest(&self) -> vers_shared::PluginManifest {
        if let Ok(dynamic) = self.dynamic_manifest.try_read() {
            if let Some(m) = &*dynamic {
                return m.clone();
            }
        }
        self.auto_manifest()
    }

    async fn on_plugin_init(
        &self,
        context: PluginRuntimeContext,
        network: Option<Arc<dyn NetworkCapability>>,
    ) -> anyhow::Result<()> {
        {
            let mut perms = self.allowed_permissions.write().await;
            *perms = context.effective_permissions;
        }
        {
            let mut client = self.http_client.write().await;
            *client = network;
        }
        Ok(())
    }

    async fn on_event(
        &self,
        event: &vers_shared::VersEvent,
    ) -> anyhow::Result<Option<vers_shared::VersEvent>> {
        match event {
            vers_shared::VersEvent::ThoughtRequested {
                agent,
                engine_id,
                message,
                context,
            } => {
                if engine_id != "bridge.python" {
                    return Ok(None);
                }

                // セキュリティチェック：NetworkAccessが必要なスクリプトで権限がない場合
                let has_network = {
                    let client = self.http_client.read().await;
                    client.is_some()
                };

                // マニフェストでNetworkAccessを要求しているか確認
                let needs_network = if let Ok(dynamic) = self.dynamic_manifest.try_read() {
                    dynamic.as_ref().map(|m| m.required_permissions.contains(&vers_shared::Permission::NetworkAccess)).unwrap_or(false)
                } else {
                    false
                };

                if needs_network && !has_network {
                    info!("🛡️ Python Bridge: Permission NetworkAccess is required but not granted. Requesting...");
                    return Ok(Some(vers_shared::VersEvent::PermissionRequested {
                        plugin_id: "bridge.python".to_string(),
                        permission: vers_shared::Permission::NetworkAccess,
                        reason: "Python script 'analyst' needs network to fetch external data.".to_string(),
                    }));
                }

                let content = self.think(agent, message, context.clone()).await?;
                return Ok(Some(vers_shared::VersEvent::ThoughtResponse {
                    agent_id: agent.id.clone(),
                    content,
                    source_message_id: message.id.clone(),
                }));
            }
            _ => {}
        }
        Ok(None)
    }

    async fn on_capability_injected(
        &self,
        capability: vers_shared::PluginCapability,
    ) -> anyhow::Result<()> {
        match capability {
            vers_shared::PluginCapability::Network(net) => {
                let mut client = self.http_client.write().await;
                *client = Some(net);
                info!("💉 Python Bridge: NetworkCapability injected live.");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl ReasoningEngine for PythonBridgePlugin {
    fn name(&self) -> &str {
        "PythonSubprocessBridge"
    }

    async fn think(
        &self,
        agent: &AgentMetadata,
        message: &VersMessage,
        context: Vec<VersMessage>,
    ) -> anyhow::Result<String> {
        let params = serde_json::json!({
            "agent": agent,
            "message": message,
            "context": context
        });

        let result = self.call_python("think", params).await?;
        Ok(result.as_str().unwrap_or_default().to_string())
    }
}