use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cost_config::CostConfig;

const CONFIG_FILE: &str = "flow-dynamics.config.json";

fn default_grid_scale() -> u8 { 12 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupConfig {
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
}

fn default_groups() -> Vec<GroupConfig> {
    vec![GroupConfig { name: "General".to_string(), collapsed: false }]
}

/// Persistent application configuration stored in `flow-dynamics.config.json`
/// alongside the layout / glyph files in the working directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Glyph library files loaded automatically at startup (in order).
    /// Later entries override earlier ones for the same component+material+diameter.
    #[serde(default)]
    pub glyph_files: Vec<PathBuf>,

    /// How many inches one canvas grid cell represents.
    /// Affects default pipe lengths and BOM scale display.
    /// Valid values: 6, 12, 18, 24.  Default 12 (1 cell = 1 foot).
    #[serde(default = "default_grid_scale")]
    pub grid_scale_inches: u8,

    /// Per-unit prices for the cost estimator.
    #[serde(default)]
    pub costs: CostConfig,

    /// Named groups for organizing palette components.
    #[serde(default = "default_groups")]
    pub groups: Vec<GroupConfig>,

    /// Maps a component key (kind_key or custom id) to a group name.
    #[serde(default)]
    pub component_groups: HashMap<String, String>,
}

impl AppConfig {
    pub fn cycle_grid_scale(&mut self) {
        self.grid_scale_inches = match self.grid_scale_inches {
            6  => 12,
            12 => 18,
            18 => 24,
            _  => 6,
        };
    }

    pub fn grid_scale_label(&self) -> &'static str {
        match self.grid_scale_inches {
            6  => "6 in / cell  (½ ft)",
            18 => "18 in / cell (1½ ft)",
            24 => "24 in / cell (2 ft)",
            _  => "12 in / cell (1 ft)",
        }
    }
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
