use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{RwLock, oneshot};
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{timeout, Duration};
use std::process::Stdio;
use std::collections::HashMap;
use exiv_shared::{
    AgentMetadata, Plugin, PluginConfig, ReasoningEngine, ExivMessage, PluginRuntimeContext, 
    exiv_plugin, NetworkCapability, Tool
};
use tracing::info;

/// Resolve a script path: try exe-relative first (deployed), fall back to CWD (development).
fn resolve_script_path(relative: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(relative);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    std::path::PathBuf::from(relative)
}

#[exiv_plugin(
    name = "bridge.python",
    kind = "Reasoning",
    description = "Universal Python Bridge with asynchronous event streaming. Supports real-time capabilities like Gaze Tracking.",
    version = "0.3.0",
    category = "Tool",
    permissions = ["NetworkAccess", "FileRead", "ProcessExecution", "VisionRead"],
    tags = ["#TOOL", "#ADAPTER"],
    capabilities = ["Reasoning", "Tool", "Web"]
)]
#[derive(Clone)]
pub struct PythonBridgePlugin {
    instance_id: String,
    script_path: String,
    state: Arc<RwLock<PythonBridgeState>>,
}

struct PythonBridgeState {
    process: Option<PythonProcessHandle>,
    dynamic_manifest: Option<exiv_shared::PluginManifest>,
    allowed_permissions: Vec<exiv_shared::Permission>,
    http_client: Option<Arc<dyn NetworkCapability>>,
    pending_calls: HashMap<i64, oneshot::Sender<anyhow::Result<serde_json::Value>>>,
    next_call_id: i64,
    event_tx: Option<tokio::sync::mpsc::Sender<exiv_shared::ExivEventData>>,
    restart_count: u32,
    last_restart: Option<std::time::Instant>,
}

struct PythonProcessHandle {
    #[allow(dead_code)]
    child: Child,
    stdin: tokio::process::ChildStdin,
    #[allow(dead_code)]
    reader_handle: tokio::task::JoinHandle<()>,
}

impl PythonBridgePlugin {
    pub async fn new_plugin(config: PluginConfig) -> anyhow::Result<Self> {
        let script_path = config.config_values.get("script_path")
            .cloned()
            .unwrap_or_else(|| "scripts/bridge_main.py".to_string());

        // Security: prevent path traversal attacks
        if script_path.contains("..") {
            return Err(anyhow::anyhow!("Script path must not contain '..': {}", script_path));
        }
        let path = std::path::Path::new(&script_path);
        if path.is_absolute() || !path.starts_with("scripts/") {
            return Err(anyhow::anyhow!("Script path must be relative and within 'scripts/' directory: {}", script_path));
        }

        Ok(Self {
            instance_id: config.id,
            script_path,
            state: Arc::new(RwLock::new(PythonBridgeState {
                process: None,
                dynamic_manifest: None,
                allowed_permissions: vec![],
                http_client: None,
                pending_calls: HashMap::new(),
                next_call_id: 1,
                event_tx: None,
                restart_count: 0,
                last_restart: None,
            })),
        })
    }

    /// Low-level send without checking process (avoids recursion)
    async fn send_raw(stdin: &mut tokio::process::ChildStdin, id: i64, method: &str, params: serde_json::Value) -> anyhow::Result<()> {
        let request = serde_json::json!({
            "id": id,
            "method": method,
            "params": params
        });
        let mut line = request.to_string();
        line.push('\n');
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    const MAX_RESTART_ATTEMPTS: u32 = 3;
    const RESTART_COOLDOWN_SECS: u64 = 5;

    async fn ensure_process(&self) -> anyhow::Result<()> {
        {
            let lock = self.state.read().await;
            if lock.process.is_some() {
                return Ok(());
            }
        }

        let mut state = self.state.write().await;
        if state.process.is_none() {
            // Check restart limits
            if state.restart_count >= Self::MAX_RESTART_ATTEMPTS {
                return Err(anyhow::anyhow!("Max restart attempts ({}) reached", Self::MAX_RESTART_ATTEMPTS));
            }
            if let Some(last) = state.last_restart {
                if last.elapsed().as_secs() < Self::RESTART_COOLDOWN_SECS {
                    return Err(anyhow::anyhow!("Restart cooldown active ({}s remaining)",
                        Self::RESTART_COOLDOWN_SECS - last.elapsed().as_secs()));
                }
            }

            // Update restart tracking
            if state.restart_count > 0 {
                info!("🔄 Restarting Python bridge (attempt {}/{})", state.restart_count + 1, Self::MAX_RESTART_ATTEMPTS);
            }
            state.restart_count += 1;
            state.last_restart = Some(std::time::Instant::now());

            let event_tx = state.event_tx.clone();
            let runtime_path = resolve_script_path("scripts/bridge_runtime.py");
            let user_script_path = resolve_script_path(&self.script_path);
            info!("🐍 Spawning Python subprocess: {} with user script: {}", runtime_path.display(), user_script_path.display());

            let python = if cfg!(windows) { "python" } else { "python3" };
            let mut child = Command::new(python)
                .arg(&runtime_path)
                .arg(&user_script_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()?;

            let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
            let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
            
            // Start background reader with enhanced error handling
            let state_weak = self.state.clone();
            let reader_handle = tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();

                loop {
                    match reader.next_line().await {
                        Ok(Some(line)) => {
                            // Process line (existing event/RPC handling)
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                // Handle event messages
                                if val.get("type").and_then(|v| v.as_str()) == Some("event") {
                                    if let (Some(ev_type), Some(ev_data)) = (val.get("event_type").and_then(|v| v.as_str()), val.get("data")) {
                                        if let Some(tx) = &event_tx {
                                            let data = match ev_type {
                                                "GazeUpdated" => {
                                                    if let Ok(gaze) = serde_json::from_value::<exiv_shared::GazeData>(ev_data.clone()) {
                                                        Some(exiv_shared::ExivEventData::GazeUpdated(gaze))
                                                    } else { None }
                                                }
                                                "SystemNotification" => Some(exiv_shared::ExivEventData::SystemNotification(ev_data.as_str().unwrap_or_default().to_string())),
                                                _ => None
                                            };
                                            if let Some(d) = data {
                                                let _ = tx.send(d).await;
                                            }
                                        }
                                    }
                                    continue;
                                }

                                // Handle RPC response messages
                                if let Some(id) = val.get("id").and_then(|v| v.as_i64()) {
                                    let mut lock = state_weak.write().await;
                                    if let Some(tx) = lock.pending_calls.remove(&id) {
                                        if let Some(err) = val.get("error") {
                                            let _ = tx.send(Err(anyhow::anyhow!("Python Error: {}", err)));
                                        } else {
                                            let _ = tx.send(Ok(val.get("result").cloned().unwrap_or(serde_json::Value::Null)));
                                        }
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            tracing::warn!("🔥 Python bridge reader received EOF - process terminated");
                            break;
                        }
                        Err(e) => {
                            tracing::error!("🔥 Python bridge reader error: {} - terminating", e);
                            break;
                        }
                    }
                }

                // Reader exited - cleanup and mark for restart
                tracing::error!("🔥 Python bridge reader task exited, cleaning up");
                let mut state = state_weak.write().await;

                // Mark process as dead (will auto-restart on next call via ensure_process)
                state.process = None;

                // Fail all pending calls
                for (_, tx) in state.pending_calls.drain() {
                    let _ = tx.send(Err(anyhow::anyhow!("Python process crashed")));
                }

                tracing::info!("🔄 Python bridge will auto-restart on next operation");
            });

            // Initial Handshake (Get Manifest) without using call_python (recursive)
            let id = state.next_call_id;
            state.next_call_id += 1;
            let (tx, rx) = oneshot::channel();
            state.pending_calls.insert(id, tx);
            
            Self::send_raw(&mut stdin, id, "get_manifest", serde_json::Value::Null).await?;
            
            drop(state); // Release lock for receiver task to work
            
            let manifest_res = timeout(Duration::from_secs(5), rx).await?;
            let manifest_json: serde_json::Value = manifest_res??;
            
            let mut state = self.state.write().await;
            let mut manifest = self.auto_manifest();
            
            // 📝 Python 側のマニフェストで上書き
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
            
            // 📂 カテゴリとサービスタイプの動的パース
            if let Some(cat_val) = manifest_json.get("category") {
                if let Ok(cat) = serde_json::from_value(cat_val.clone()) {
                    manifest.category = cat;
                }
            }
            if let Some(st_val) = manifest_json.get("service_type") {
                if let Ok(st) = serde_json::from_value(st_val.clone()) {
                    manifest.service_type = st;
                }
            }

            // 🛠️ 能力と権限の継承
            if let Some(caps_val) = manifest_json.get("capabilities").and_then(|v| v.as_array()) {
                let mut caps = Vec::new();
                for c in caps_val {
                    if let Ok(cap) = serde_json::from_value(c.clone()) {
                        caps.push(cap);
                    }
                }
                if !caps.is_empty() {
                    manifest.provided_capabilities = caps;
                }
            }

            if let Some(perms_val) = manifest_json.get("required_permissions").and_then(|v| v.as_array()) {
                let mut perms = Vec::new();
                for p in perms_val {
                    if let Ok(perm) = serde_json::from_value(p.clone()) {
                        perms.push(perm);
                    }
                }
                if !perms.is_empty() {
                    manifest.required_permissions = perms;
                }
            }

            // 🧰 ツールとアクション情報の継承
            if let Some(tools_val) = manifest_json.get("provided_tools").and_then(|v| v.as_array()) {
                manifest.provided_tools = tools_val.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            }
            if let Some(icon) = manifest_json.get("action_icon").and_then(|v| v.as_str()) {
                manifest.action_icon = Some(icon.to_string());
            }
            if let Some(target) = manifest_json.get("action_target").and_then(|v| v.as_str()) {
                manifest.action_target = Some(target.to_string());
            }

            // 🏷️ タグの統合と #PYTHON の強制付与
            if let Some(tags_val) = manifest_json.get("tags").and_then(|v| v.as_array()) {
                for t in tags_val {
                    if let Some(t_str) = t.as_str() {
                        let t_str = if t_str.starts_with('#') { t_str.to_string() } else { format!("#{}", t_str) };
                        if !manifest.tags.contains(&t_str) {
                            manifest.tags.push(t_str);
                        }
                    }
                }
            }
            if !manifest.tags.contains(&"#PYTHON".to_string()) {
                manifest.tags.push("#PYTHON".to_string());
            }
            
            state.dynamic_manifest = Some(manifest);
            state.process = Some(PythonProcessHandle { child, stdin, reader_handle });
        }
        Ok(())
    }

    pub async fn call_python(&self, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        self.ensure_process().await?; 
        
        let (id, tx) = {
            let mut state = self.state.write().await;
            let id = state.next_call_id;
            state.next_call_id += 1;
            let (tx, rx) = oneshot::channel();
            state.pending_calls.insert(id, tx);
            (id, rx)
        };

        {
            let mut state = self.state.write().await;
            if let Some(proc) = state.process.as_mut() {
                Self::send_raw(&mut proc.stdin, id, method, params).await?;
            } else {
                return Err(anyhow::anyhow!("Python process not running"));
            }
        }

        match timeout(Duration::from_secs(10), tx).await {
            Ok(res) => res?,
            Err(_) => {
                let mut state = self.state.write().await;
                state.pending_calls.remove(&id);
                Err(anyhow::anyhow!("Python call timed out"))
            }
        }
    }
}

#[async_trait]
impl Plugin for PythonBridgePlugin {
    fn manifest(&self) -> exiv_shared::PluginManifest {
        if let Ok(state) = self.state.try_read() {
            if let Some(m) = &state.dynamic_manifest {
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
            let mut state = self.state.write().await;
            state.allowed_permissions = context.effective_permissions;
            state.http_client = network;
            state.event_tx = Some(context.event_tx);
        }

        // 🐍 Perform handshake immediately to load dynamic manifest
        if let Err(e) = self.ensure_process().await {
            tracing::error!("❌ Python Bridge: Failed to initialize subprocess for {}: {}", self.instance_id, e);
        } else {
            info!("🐍 Python Bridge: Subprocess handshake complete for {}", self.instance_id);
        }
        
        Ok(())
    }

    async fn on_event(&self, event: &exiv_shared::ExivEvent) -> anyhow::Result<Option<exiv_shared::ExivEventData>> {
        if let exiv_shared::ExivEventData::ThoughtRequested { agent, engine_id, message, context } = &event.data {
            let manifest = self.manifest();
            if engine_id != &self.instance_id && engine_id != "bridge.python" && engine_id != &manifest.id {
                return Ok(None);
            }
            let content = self.think(agent, message, context.clone()).await?;
            return Ok(Some(exiv_shared::ExivEventData::ThoughtResponse {
                agent_id: agent.id.clone(),
                engine_id: manifest.id.clone(),
                content,
                source_message_id: message.id.clone(),
            }));
        }
        Ok(None)
    }
}

impl exiv_shared::WebPlugin for PythonBridgePlugin {
    fn register_routes(
        &self,
        router: axum::Router<Arc<dyn std::any::Any + Send + Sync>>,
    ) -> axum::Router<Arc<dyn std::any::Any + Send + Sync>> {
        let instance_id = self.instance_id.clone();
        let plugin = self.clone();
        
        router.route(
            &format!("/api/plugin/{}/action/:command", instance_id),
            axum::routing::post(move |
                uri: axum::http::Uri,
                body: Option<axum::Json<serde_json::Value>>
            | {
                let plugin = plugin.clone();
                let body_val = body.map(|b| b.0).unwrap_or(serde_json::Value::Null);
                async move {
                    // Extract command from URI to avoid Path extractor conflict
                    // with outer router's wildcard parameter
                    let command = uri.path()
                        .rsplit('/')
                        .next()
                        .unwrap_or("unknown")
                        .to_string();
                    match plugin.call_python(&format!("on_action_{}", command), body_val).await {
                        Ok(res) => axum::Json(res),
                        Err(e) => {
                            tracing::error!("❌ Python Bridge Action Error: {}", e);
                            axum::Json(serde_json::json!({ "error": e.to_string() }))
                        }
                    }
                }
            }),
        )
    }
}

#[async_trait]
impl ReasoningEngine for PythonBridgePlugin {
    fn name(&self) -> &str { "PythonSubprocessBridge" }
    async fn think(&self, agent: &AgentMetadata, message: &ExivMessage, context: Vec<ExivMessage>) -> anyhow::Result<String> {
        let params = serde_json::json!({ "agent": agent, "message": message, "context": context });
        let result = self.call_python("think", params).await?;
        Ok(result.as_str().unwrap_or_default().to_string())
    }
}

#[async_trait]
impl Tool for PythonBridgePlugin {
    fn name(&self) -> &str { "PythonBridgeTool" }
    fn description(&self) -> &str { "Delegates tool execution to Python script." }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        self.call_python("execute", args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_restart_rate_limiting() {
        let mut config_values = HashMap::new();
        config_values.insert("script_path".to_string(), "scripts/test.py".to_string());

        let config = PluginConfig {
            id: "test.bridge".to_string(),
            config_values,
        };

        let plugin = PythonBridgePlugin::new_plugin(config).await.unwrap();

        // Simulate max restart attempts reached
        {
            let mut state = plugin.state.write().await;
            state.restart_count = PythonBridgePlugin::MAX_RESTART_ATTEMPTS;
        }

        // Next ensure_process should fail due to max attempts
        let result = plugin.ensure_process().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max restart attempts"));
    }

    #[tokio::test]
    async fn test_restart_cooldown() {
        let mut config_values = HashMap::new();
        config_values.insert("script_path".to_string(), "scripts/test.py".to_string());

        let config = PluginConfig {
            id: "test.bridge2".to_string(),
            config_values,
        };

        let plugin = PythonBridgePlugin::new_plugin(config).await.unwrap();

        // Simulate recent restart
        {
            let mut state = plugin.state.write().await;
            state.restart_count = 1;
            state.last_restart = Some(std::time::Instant::now());
        }

        // Immediate restart should fail due to cooldown
        let result = plugin.ensure_process().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cooldown"));
    }

    #[tokio::test]
    async fn test_initial_startup_allowed() {
        let mut config_values = HashMap::new();
        config_values.insert("script_path".to_string(), "scripts/test.py".to_string());

        let config = PluginConfig {
            id: "test.bridge3".to_string(),
            config_values,
        };

        let plugin = PythonBridgePlugin::new_plugin(config).await.unwrap();

        // Initial startup (restart_count = 0) should not be blocked
        let state = plugin.state.read().await;
        assert_eq!(state.restart_count, 0);
        assert!(state.last_restart.is_none());
    }
}