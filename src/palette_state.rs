use std::collections::HashMap;

use crate::components::{ComponentKind, PipeDiameter, PipeMaterial};

pub struct PaletteState {
    pub palette: Vec<ComponentKind>,
    pub palette_idx: usize,
    pub palette_custom_indices: Vec<Option<usize>>,
    pub selected_diameter: PipeDiameter,
    pub selected_material: PipeMaterial,
    pub default_lengths: HashMap<ComponentKind, f32>,
    pub default_arm_lengths: HashMap<ComponentKind, [f32; 4]>,
    pub palette_search: String,
    pub palette_search_active: bool,
    pub build_color_cursor: usize,
    pub build_custom_rgb: Option<[u8; 3]>,
}

impl Default for PaletteState {
    fn default() -> Self {
        let mut default_lengths = HashMap::new();
        default_lengths.insert(ComponentKind::PipeH, 1.0_f32);
        default_lengths.insert(ComponentKind::PipeV, 1.0_f32);
        Self {
            palette: Vec::new(),
            palette_idx: 2,
            palette_custom_indices: Vec::new(),
            selected_diameter: PipeDiameter::ThreeQuarter,
            selected_material: PipeMaterial::Copper,
            default_lengths,
            default_arm_lengths: HashMap::new(),
            palette_search: String::new(),
            palette_search_active: false,
            build_color_cursor: 11,
            build_custom_rgb: None,
        }
    }
}
