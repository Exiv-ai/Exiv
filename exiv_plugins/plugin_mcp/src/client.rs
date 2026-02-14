use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use tracing::{debug, error, info};

use crate::protocol::{
    CallToolParams, CallToolResult, ClientCapabilities, ClientInfo, InitializeParams, 
    JsonRpcRequest, JsonRpcResponse, ListToolsResult
};
use crate::stdio::StdioTransport;

pub struct McpClient {
    transport: Arc<Mutex<StdioTransport>>,
    pending_requests: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>,
    next_id: Arc<Mutex<i64>>,
}

impl McpClient {
    pub async fn connect(command: &str, args: &[String]) -> Result<Self> {
        let transport = StdioTransport::start(command, args).await?;
        let client = Self {
            transport: Arc::new(Mutex::new(transport)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        };

        // Start response loop
        client.start_response_loop();

        // Perform MCP Handshake
        client.initialize().await?;

        Ok(client)
    }

    fn start_response_loop(&self) {
        let transport = self.transport.clone();
        let pending = self.pending_requests.clone();

        tokio::spawn(async move {
            loop {
                // Must lock transport to recv
                let msg_opt = {
                    let mut tp = transport.lock().await;
                    tp.recv().await
                };

                match msg_opt {
                    Some(line) => {
                        // Try parsing as response
                        if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                            if let Some(id_val) = response.id {
                                if let Some(id) = id_val.as_i64() {
                                    let mut map = pending.lock().await;
                                    if let Some(tx) = map.remove(&id) {
                                        if let Some(error) = response.error {
                                            let _ = tx.send(Err(anyhow::anyhow!("RPC Error {}: {}", error.code, error.message)));
                                        } else {
                                            let _ = tx.send(Ok(response.result.unwrap_or(Value::Null)));
                                        }
                                    }
                                }
                            }
                        } else {
                            // Could be a notification or invalid JSON
                            debug!("Received non-response message: {}", line);
                        }
                    }
                    None => {
                        error!("🔌 MCP Connection closed.");
                        break;
                    }
                }
            }
        });
    }

    async fn call(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = {
            let mut lock = self.next_id.lock().await;
            let id = *lock;
            *lock += 1;
            id
        };

        let request = JsonRpcRequest::new(id, method, params);
        let req_str = serde_json::to_string(&request)?;

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending_requests.lock().await;
            map.insert(id, tx);
        }

        {
            let tp = self.transport.lock().await;
            tp.send(req_str).await?;
        }

        // Wait for response with timeout (Guardrail #7)
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(res) => res.context("Response channel closed")?,
            Err(_) => {
                let mut map = self.pending_requests.lock().await;
                map.remove(&id);
                Err(anyhow::anyhow!("MCP Request timed out"))
            }
        }
    }

    async fn initialize(&self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(), // Latest draft
            capabilities: ClientCapabilities {},
            client_info: ClientInfo {
                name: "EXIV-SYSTEM".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let result = self.call("initialize", Some(serde_json::to_value(params)?)).await?;
        info!("✅ MCP Initialized: {:?}", result);

        // Send initialized notification
        let notify = JsonRpcRequest::notification("notifications/initialized", None);
        let notify_str = serde_json::to_string(&notify)?;
        {
            let tp = self.transport.lock().await;
            tp.send(notify_str).await?;
        }

        Ok(())
    }

    pub async fn list_tools(&self) -> Result<ListToolsResult> {
        let val = self.call("tools/list", None).await?;
        let result: ListToolsResult = serde_json::from_value(val)?;
        Ok(result)
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult> {
        let params = CallToolParams {
            name: name.to_string(),
            arguments: args,
        };
        let val = self.call("tools/call", Some(serde_json::to_value(params)?)).await?;
        let result: CallToolResult = serde_json::from_value(val)?;
        Ok(result)
    }
}
