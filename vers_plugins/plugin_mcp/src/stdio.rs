use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct StdioTransport {
    child: Child,
    request_tx: mpsc::Sender<String>,
    response_rx: mpsc::Receiver<String>,
}

impl StdioTransport {
    pub async fn start(command: &str, args: &[String]) -> Result<Self> {
        info!("🔌 Starting MCP Server: {} {:?}", command, args);

        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true) // Guardrail: Clean up process on drop
            .spawn()
            .context(format!("Failed to spawn MCP server: {}", command))?;

        let stdin = child.stdin.take().context("Failed to open stdin")?;
        let stdout = child.stdout.take().context("Failed to open stdout")?;
        let stderr = child.stderr.take().context("Failed to open stderr")?;

        let (req_tx, mut req_rx) = mpsc::channel::<String>(100);
        let (res_tx, res_rx) = mpsc::channel::<String>(100);

        // Writer Task
        tokio::spawn(async move {
            let mut writer = stdin;
            while let Some(msg) = req_rx.recv().await {
                // MCP requires messages to be line-delimited
                let line = format!("{}
", msg);
                if let Err(e) = writer.write_all(line.as_bytes()).await {
                    error!("❌ Failed to write to MCP server stdin: {}", e);
                    break;
                }
                if let Err(e) = writer.flush().await {
                    error!("❌ Failed to flush MCP server stdin: {}", e);
                    break;
                }
            }
        });

        // Reader Task (Stdout)
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if !line.trim().is_empty() {
                    if let Err(_) = res_tx.send(line).await {
                        break; // Channel closed
                    }
                }
            }
            warn!("🔌 MCP Server stdout closed.");
        });

        // Logger Task (Stderr)
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                warn!("[MCP Stderr] {}", line);
            }
        });

        Ok(Self {
            child,
            request_tx: req_tx,
            response_rx: res_rx,
        })
    }

    pub async fn send(&self, msg: String) -> Result<()> {
        self.request_tx.send(msg).await.context("Failed to send message to transport task")
    }

    pub async fn recv(&mut self) -> Option<String> {
        self.response_rx.recv().await
    }
}
