use async_trait::async_trait;
use vers_shared::{
    Plugin, PluginConfig, vers_plugin, VersEvent, VersEventData, 
    HandAction, ColorVisionData, DetectedElement
};
use chrono::Utc;

#[vers_plugin(
    name = "vision.screen",
    kind = "Vision",
    description = "Screen capture and analysis module.",
    version = "0.1.0",
    category = "Tool",
    permissions = ["VisionRead"],
    tags = ["#TOOL", "#VISION"],
    capabilities = ["Vision"]
)]
pub struct VisionPlugin {}

impl VisionPlugin {
    pub async fn new_plugin(_config: PluginConfig) -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl Plugin for VisionPlugin {
    fn manifest(&self) -> vers_shared::PluginManifest {
        self.auto_manifest()
    }

    async fn on_event(&self, event: &VersEvent) -> anyhow::Result<Option<VersEventData>> {
        if let VersEventData::ActionRequested { requester: _, action } = &event.data {
            if let HandAction::CaptureScreen = action {
                tracing::info!("📷 Vision Plugin: Capturing screen...");
                
                // ここで実際にスクリーンショットを撮る
                // let buffer = screenshots::Screen::all()?[0].capture()?;
                
                // Mock response
                let vision_data = ColorVisionData {
                    captured_at: Utc::now(),
                    detected_elements: vec![
                        DetectedElement {
                            label: "Submit Button".to_string(),
                            bounds: (100, 200, 50, 20),
                            confidence: 0.99,
                            attributes: std::collections::HashMap::new(),
                        }
                    ],
                    image_ref: Some("memory://mock-image-id".to_string()),
                };

                return Ok(Some(VersEvent::with_trace(
                    event.trace_id,
                    VersEventData::VisionUpdated(vision_data)
                ).data));
            }
        }
        Ok(None)
    }
}
