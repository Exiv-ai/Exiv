use std::sync::Arc;
use tokio::sync::mpsc;
use vers_shared::{VersEvent, VersMessage, MessageSource, Plugin};
use vers_core::handlers::system::SystemHandler;
use vers_core::managers::{PluginRegistry, AgentManager};
use sqlx::SqlitePool;

#[tokio::test]
async fn test_system_handler_loop_prevention() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    // Setup minimal DB for AgentManager
    sqlx::query("CREATE TABLE agents (id TEXT PRIMARY KEY, name TEXT, description TEXT, status TEXT, default_engine_id TEXT, required_capabilities TEXT, metadata TEXT)").execute(&pool).await.unwrap();
    
    let agent_id = "agent.test";
    sqlx::query("INSERT INTO agents (id, name, description, status, default_engine_id, required_capabilities, metadata) VALUES (?, 'Test Agent', 'Desc', 'online', 'engine.test', '[\"Reasoning\", \"Memory\"]', '{}')")
        .bind(agent_id)
        .execute(&pool).await.unwrap();

    let registry = Arc::new(PluginRegistry::new(5, 10));
    let agent_manager = AgentManager::new(pool);
    let (event_tx, mut event_rx) = mpsc::channel(10);

    let metrics = Arc::new(vers_core::managers::SystemMetrics::new());
    let handler = SystemHandler::new(
        registry.clone(),
        agent_manager,
        agent_id.to_string(),
        event_tx,
        10, // memory_context_limit
        metrics,
        vec!["mind.deepseek".to_string(), "mind.cerebras".to_string()],
    );

    // 1. Test User Message (Should trigger ThoughtRequested)
    let user_msg = VersMessage::new(
        MessageSource::User { id: "user1".into(), name: "User".into() },
        "Hello".into()
    );
    let user_event = VersEvent::new(vers_shared::VersEventData::MessageReceived(user_msg));
    
    let _ = handler.on_event(&user_event).await.unwrap();
    
    let received_envelope = event_rx.try_recv().expect("Should have received ThoughtRequested");
    if let vers_shared::VersEventData::ThoughtRequested { .. } = &received_envelope.event.data {
        // OK
    } else {
        panic!("Expected ThoughtRequested, got {:?}", received_envelope.event);
    }

    // 2. Test Agent Message (Should NOT trigger ThoughtRequested)
    let agent_msg = VersMessage::new(
        MessageSource::Agent { id: agent_id.into() },
        "Response".into()
    );
    let agent_event = VersEvent::new(vers_shared::VersEventData::MessageReceived(agent_msg));
    
    let _ = handler.on_event(&agent_event).await.unwrap();
    
    // Check that NO event was sent to the channel
    let result = event_rx.try_recv();
    assert!(result.is_err(), "Should NOT have received any event for agent message");
}
