use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc, time::Duration, collections::HashMap};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn, error};
use vers_shared::{
    VersEvent, VersMessage, MessageSource, AgentMetadata, VersId,
    CommunicationAdapter, ReasoningEngine, MemoryProvider, Tool, Capability
};
use plugin_ks2_2::Ks2_2_Plugin;

/// カーネルが管理するプラグイン・レジストリ
struct PluginRegistry {
    adapters: Vec<Arc<dyn CommunicationAdapter>>,
    engines: HashMap<String, Arc<dyn ReasoningEngine>>,
    memories: HashMap<String, Arc<dyn MemoryProvider>>,
    tools: HashMap<String, Arc<dyn Tool>>,
}

struct AppState {
    tx: broadcast::Sender<VersEvent>,
    registry: Arc<PluginRegistry>,
    event_tx: mpsc::Sender<VersEvent>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    info!("+---------------------------------------+");
    info!("|            VERS-OS Kernel             |");
    info!("|             Version 0.1.0             |");
    info!("+---------------------------------------+");

    println!("[BOOT] Initializing KS2.2 Plugin...");
    // 1. KS2.2 プラグインの初期化
    let database_url = "sqlite:/home/botuser/vers_project/vers_memories.db";
    info!("Using database: {}", database_url);
    
    let ks2_2 = Arc::new(Ks2_2_Plugin::new(database_url).await?);
    println!("[BOOT] Plugin KS2.2 ready.");
    
    // 2. レジストリの構築
    let mut engines = HashMap::new();
    engines.insert("ks2_2_mind".to_string(), ks2_2.clone() as Arc<dyn ReasoningEngine>);
    
    let mut memories = HashMap::new();
    memories.insert("ks2_2_storage".to_string(), ks2_2.clone() as Arc<dyn MemoryProvider>);

    let registry = Arc::new(PluginRegistry {
        adapters: Vec::new(),
        engines,
        memories,
        tools: HashMap::new(),
    });

    // 3. イベントバスの構築
    let (tx, _rx) = broadcast::channel(100);
    let (event_tx, mut event_rx) = mpsc::channel::<VersEvent>(100);

    let app_state = Arc::new(AppState { 
        tx: tx.clone(),
        registry: registry.clone(),
        event_tx: event_tx.clone(),
    });

    // 4. モックエージェント設定 (Karin)
    let karin_metadata = AgentMetadata {
        id: VersId::new(),
        name: "Karin".to_string(),
        description: "Vers-native Karin Agent".to_string(),
        capabilities: vec![Capability::MemoryRead, Capability::MemoryWrite],
        plugin_bindings: vec![],
    };

    // 5. カーネルのメイン・イベントループ
    let tx_internal = tx.clone();
    let registry_internal = registry.clone();
    tokio::spawn(async move {
        info!("🧠 Kernel Event Loop started.");
        while let Some(event) = event_rx.recv().await {
            match event {
                VersEvent::MessageReceived(msg) => {
                    info!("📩 Message received: {}", msg.content);
                    let _ = tx_internal.send(VersEvent::MessageReceived(msg.clone()));
                    
                    // エージェント思考プロセス (KS2.2 Mindを使用)
                    if let Some(engine) = registry_internal.engines.get("ks2_2_mind") {
                        let engine = engine.clone();
                        let tx = tx_internal.clone();
                        let registry = registry_internal.clone();
                        let agent = karin_metadata.clone();
                        let msg_clone = msg.clone();
                        
                        tokio::spawn(async move {
                            match engine.think(&agent, &msg_clone, vec![]).await {
                                Ok(response_text) => {
                                    let response = VersMessage::new(
                                        MessageSource::Agent(agent.id),
                                        response_text
                                    );
                                    let _ = tx.send(VersEvent::MessageReceived(response.clone()));
                                    
                                    // 記憶への保存 (KS2.2 Storageを使用)
                                    if let Some(memory) = registry.memories.get("ks2_2_storage") {
                                        let _ = memory.store(agent.id, msg_clone).await;
                                        let _ = memory.store(agent.id, response).await;
                                    }
                                }
                                Err(e) => error!("Engine error: {}", e),
                            }
                        });
                    }
                }
                VersEvent::SystemNotification(note) => {
                    info!("🔔 System: {}", note);
                    let _ = tx_internal.send(VersEvent::SystemNotification(note));
                }
                _ => {
                    let _ = tx_internal.send(event);
                }
            }
        }
    });

    // 6. Webサーバーの設定
    let dashboard_path = std::env::var("VERS_DASHBOARD_PATH")
        .unwrap_or_else(|_| "../vers_dashboard/dist".to_string());

    let app = Router::new()
        .route("/events", get(sse_handler))
        .route("/chat", post(chat_handler))
        .nest_service("/", ServeDir::new(&dashboard_path))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let port = 8081;
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("🚀 VERS-OS Kernel is listening on http://0.0.0.0:{}", port);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.tx.subscribe();
    let stream = async_stream::stream! {
        yield Ok(Event::default().event("handshake").data("connected"));
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                yield Ok(Event::default().data(json));
            }
        }
    };
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive")
    )
}

async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(msg): Json<VersMessage>,
) -> Json<serde_json::Value> {
    // ユーザーからのメッセージをイベントチャネルに送る
    let _ = state.event_tx.send(VersEvent::MessageReceived(msg)).await;
    Json(serde_json::json!({ "status": "accepted" }))
}