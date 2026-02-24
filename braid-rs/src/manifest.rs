use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameManifest {
    pub game_title: String,
    pub rom_hash: String,
    pub emulator_core: String,
    #[serde(default = "default_sync_method")]
    pub sync_method: String,
    #[serde(default = "default_frame_delay")]
    pub frame_delay: i32,
}

fn default_sync_method() -> String {
    "rollback".to_string()
}

fn default_frame_delay() -> i32 {
    2
}

impl GameManifest {
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(data: &str) -> serde_json::Result<Self> {
        serde_json::from_str(data)
    }
}
