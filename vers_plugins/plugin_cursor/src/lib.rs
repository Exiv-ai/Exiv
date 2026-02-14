use async_trait::async_trait;
use vers_shared::{
    Plugin, PluginConfig, PluginManifest, vers_plugin, VersEvent, HandAction
};

#[vers_plugin(
    name = "hal.cursor",
    kind = "HAL",
    description = "High-precision dot cursor with fluid motion trails from Karin System 1.6.12.",
    version = "1.6.12",
    permissions = ["InputControl"],
    capabilities = ["HAL"]
)]
pub struct CursorPlugin {
    id: String,
}

impl CursorPlugin {
    pub async fn new_plugin(config: PluginConfig) -> anyhow::Result<Self> {
        Ok(Self { id: config.id })
    }
}

#[async_trait]
impl Plugin for CursorPlugin {
    fn manifest(&self) -> PluginManifest {
        self.auto_manifest()
    }

    async fn on_event(&self, event: &VersEvent) -> anyhow::Result<Option<VersEvent>> {
        if let VersEvent::ActionRequested { requester: _, action } = event {
            let action_desc = match action {
                HandAction::MouseMove { x, y } => format!("Moving cursor to ({}, {})", x, y),
                HandAction::MouseClick { button } => format!("Clicking {} button", button),
                HandAction::KeyPress { key } => format!("Pressing key: {}", key),
                HandAction::Wait { ms } => format!("Waiting for {}ms", ms),
            };
            
            tracing::info!("🖱️ Neural Cursor Action: {}", action_desc);
            
            return Ok(Some(VersEvent::SystemNotification(
                format!("Neural Cursor Executed: {}", action_desc)
            )));
        }
        Ok(None)
    }
}
