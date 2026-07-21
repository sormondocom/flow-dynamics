use std::path::PathBuf;

use crate::app::AppMode;
use crate::assembly::{Assembly, AssemblyLibrary};

pub struct SelectionState {
    pub select_start: Option<(usize, usize)>,
    pub assembly_lib: AssemblyLibrary,
    pub assembly_path: Option<PathBuf>,
    pub assembly_idx: usize,
    pub pending_stamp: Option<Assembly>,
    pub stamp_cut_rect: Option<(usize, usize, usize, usize)>,
    pub pre_assembly_mode: AppMode,
}

impl SelectionState {
    pub fn new(assembly_lib: AssemblyLibrary) -> Self {
        Self {
            select_start: None,
            assembly_lib,
            assembly_path: Some(PathBuf::from("assemblies.json")),
            assembly_idx: 0,
            pending_stamp: None,
            stamp_cut_rect: None,
            pre_assembly_mode: AppMode::Build,
        }
    }
}
