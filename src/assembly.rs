use serde::{Deserialize, Serialize};

use crate::components::{Component, ComponentKind};
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

    /// If (ar, ac) is a satellite cell of any composite in the assembly, returns the
    /// appropriate box-drawing character for that position. Returns None if the cell
    /// is empty or is a direct component anchor (the caller handles those).
    /// Return the ghost char for a Label or Note annotation overlay cell at (ar, ac).
    /// Label anchors and Note anchors are direct components handled by the caller;
    /// this covers the text/bracket/box chars that live in cells beyond the anchor.
    pub fn annotation_ghost_char(&self, ar: usize, ac: usize) -> Option<char> {
        for ra in 0..self.height {
            for ca in 0..self.width {
                let Some(comp) = self.get(ra, ca) else { continue };
                match comp.kind {
                    ComponentKind::Label => {
                        if ar != ra { continue; }
                        let Some(text) = &comp.text else { continue };
                        let n = text.chars().count();
                        if ac > ca && ac <= ca + n {
                            return text.chars().nth(ac - ca - 1);
                        }
                        if ac == ca + n + 1 {
                            return Some(']');
                        }
                    }
                    ComponentKind::Note => {
                        let Some(text) = &comp.text else { continue };
                        let segs: Vec<&str> = text.split('\n').collect();
                        let max_w = segs.iter().map(|s| s.chars().count()).max().unwrap_or(0);
                        let inner_w = max_w + 2;
                        let right_c = ca + inner_w + 1;
                        let n = segs.len();

                        if ar == ra {
                            // Top border (same row as '*' anchor)
                            if ac > ca && ac <= ca + inner_w { return Some('═'); }
                            if ac == right_c { return Some('╗'); }
                        } else if ar == ra + 1 {
                            // Top blank padding row
                            if ac == ca { return Some('║'); }
                            if ac > ca && ac < right_c { return Some(' '); }
                            if ac == right_c { return Some('║'); }
                        } else if ar >= ra + 2 && ar < ra + 2 + n {
                            // Content rows
                            let li = ar - ra - 2;
                            let chars: Vec<char> = segs[li].chars().collect();
                            if ac == ca { return Some('║'); }
                            if ac == ca + 1 { return Some(' '); }
                            if ac >= ca + 2 && ac < ca + 2 + max_w {
                                return Some(chars.get(ac - ca - 2).copied().unwrap_or(' '));
                            }
                            if ac == ca + max_w + 2 { return Some(' '); }
                            if ac == right_c { return Some('║'); }
                        } else if ar == ra + n + 2 {
                            // Bottom blank padding row
                            if ac == ca { return Some('║'); }
                            if ac > ca && ac < right_c { return Some(' '); }
                            if ac == right_c { return Some('║'); }
                        } else if ar == ra + n + 3 {
                            // Bottom border row
                            if ac == ca { return Some('╚'); }
                            if ac > ca && ac <= ca + inner_w { return Some('═'); }
                            if ac == right_c { return Some('╝'); }
                        }
                    }
                    ComponentKind::Link => {
                        // Single-content-row box: 5 rows total (anchor + 4)
                        let path_text = comp.text.as_deref().unwrap_or("(no path)");
                        let text_w = path_text.chars().count();
                        let inner_w = text_w + 2;
                        let right_c = ca + inner_w + 1;

                        if ar == ra {
                            // Top border row (anchor '⇒' is the component itself)
                            if ac > ca && ac <= ca + inner_w { return Some('═'); }
                            if ac == right_c { return Some('╗'); }
                        } else if ar == ra + 1 {
                            // Top blank padding
                            if ac == ca { return Some('║'); }
                            if ac > ca && ac < right_c { return Some(' '); }
                            if ac == right_c { return Some('║'); }
                        } else if ar == ra + 2 {
                            // Content row: ║ space path space ║
                            let chars: Vec<char> = path_text.chars().collect();
                            if ac == ca { return Some('║'); }
                            if ac == ca + 1 { return Some(' '); }
                            if ac >= ca + 2 && ac < ca + 2 + text_w {
                                return Some(chars.get(ac - ca - 2).copied().unwrap_or(' '));
                            }
                            if ac == ca + text_w + 2 { return Some(' '); }
                            if ac == right_c { return Some('║'); }
                        } else if ar == ra + 3 {
                            // Bottom blank padding
                            if ac == ca { return Some('║'); }
                            if ac > ca && ac < right_c { return Some(' '); }
                            if ac == right_c { return Some('║'); }
                        } else if ar == ra + 4 {
                            // Bottom border
                            if ac == ca { return Some('╚'); }
                            if ac > ca && ac <= ca + inner_w { return Some('═'); }
                            if ac == right_c { return Some('╝'); }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    pub fn composite_ghost_char(&self, ar: usize, ac: usize) -> Option<char> {
        for ra in 0..self.height {
            for ca in 0..self.width {
                let Some(comp) = self.get(ra, ca) else { continue };
                if !comp.effective_is_composite() { continue; }
                let (fw, fh) = comp.effective_footprint();
                let pr = comp.effective_port_row();
                let Some(top) = ra.checked_sub(pr) else { continue };
                let Some(dr) = ar.checked_sub(top) else { continue };
                let Some(dc) = ac.checked_sub(ca) else { continue };
                if dr >= fh || dc >= fw { continue; }
                if dr == pr && dc == 0 { continue; } // anchor — handled by caller
                let label = comp.effective_composite_label();
                let (_, _, ae, aw) = comp.connections();
                return Some(crate::ui::composite_box_char(fw, fh, pr, dr, dc, label, None, ae || aw));
            }
        }
        None
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
