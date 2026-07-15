use serde::{Deserialize, Serialize};

use crate::components::Component;
use crate::grid::Grid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assembly {
    pub name: String,
    pub description: String,
    pub width: usize,
    pub height: usize,
    /// Row-major flat array; `None` = empty cell within the assembly bounds.
    pub cells: Vec<Option<Component>>,
}

impl Assembly {
    pub fn from_selection(
        grid: &Grid,
        r_start: usize,
        c_start: usize,
        r_end: usize,
        c_end: usize,
        name: String,
        description: String,
    ) -> Self {
        let height = (r_end + 1).saturating_sub(r_start).max(1);
        let width = (c_end + 1).saturating_sub(c_start).max(1);
        let mut cells = Vec::with_capacity(height * width);
        for r in r_start..=r_end {
            for c in c_start..=c_end {
                cells.push(grid.get(r, c).cloned());
            }
        }
        Self { name, description, width, height, cells }
    }

    pub fn get(&self, r: usize, c: usize) -> Option<&Component> {
        if r < self.height && c < self.width {
            self.cells[r * self.width + c].as_ref()
        } else {
            None
        }
    }

    pub fn stamp_onto(&self, grid: &mut Grid, top_row: usize, top_col: usize) {
        for r in 0..self.height {
            for c in 0..self.width {
                let gr = top_row + r;
                let gc = top_col + c;
                if gr < grid.height && gc < grid.width {
                    grid.set(gr, gc, self.cells[r * self.width + c].clone());
                }
            }
        }
    }

    pub fn component_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssemblyLibrary {
    pub assemblies: Vec<Assembly>,
}

impl AssemblyLibrary {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&txt).map_err(|e| e.to_string())
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}
