use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "flow-dynamics.config.json";

/// Persistent application configuration stored in `flow-dynamics.config.json`
/// alongside the layout / glyph files in the working directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Glyph library files loaded automatically at startup (in order).
    /// Later entries override earlier ones for the same component+material+diameter.
    #[serde(default)]
    pub glyph_files: Vec<PathBuf>,
}

impl AppConfig {
    pub fn load() -> Self {
        std::fs::read_to_string(CONFIG_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(CONFIG_FILE, json);
        }
    }
}
