pub mod config;
pub mod db;
pub mod events;
pub mod handlers;
pub mod managers;
pub mod capabilities;

use vers_shared::VersEvent;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub struct AppState {
    pub tx: broadcast::Sender<VersEvent>,
    pub registry: Arc<managers::PluginRegistry>,
    pub event_tx: mpsc::Sender<VersEvent>,
    pub pool: SqlitePool,
    pub agent_manager: managers::AgentManager,
    pub plugin_manager: Arc<managers::PluginManager>,
}
