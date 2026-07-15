use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::components::Component;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    pub cells: Vec<Vec<Option<Component>>>,
    pub width: usize,
    pub height: usize,
    /// Maps satellite cell positions → anchor cell position for composite components.
    /// Not serialised — reconstructed by `rebuild_satellites()` after load.
    #[serde(skip, default)]
    pub satellites: HashMap<(usize, usize), (usize, usize)>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![None; width]; height],
            width,
            height,
            satellites: HashMap::new(),
        }
    }

    pub fn get(&self, row: usize, col: usize) -> Option<&Component> {
        self.cells.get(row)?.get(col)?.as_ref()
    }

    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut Component> {
        self.cells.get_mut(row)?.get_mut(col)?.as_mut()
    }

    pub fn set(&mut self, row: usize, col: usize, comp: Option<Component>) {
        if row < self.height && col < self.width {
            self.cells[row][col] = comp;
        }
    }

    /// If (row, col) is a satellite cell, returns the anchor position; otherwise None.
    pub fn satellite_anchor(&self, row: usize, col: usize) -> Option<(usize, usize)> {
        self.satellites.get(&(row, col)).copied()
    }

    /// Returns the anchor position for any cell:
    /// — the cell itself for anchors and single-cell components,
    /// — the composite anchor for satellite cells.
    pub fn effective_pos(&self, row: usize, col: usize) -> (usize, usize) {
        self.satellites.get(&(row, col)).copied().unwrap_or((row, col))
    }

    /// Clear the component at (row, col).
    /// Resolves satellite → anchor automatically.
    /// For composite anchors, also removes all satellite registrations.
    pub fn clear_at(&mut self, row: usize, col: usize) {
        let (ar, ac) = self.effective_pos(row, col);

        if let Some(comp) = self.cells[ar][ac].as_ref() {
            if comp.effective_is_composite() {
                let (fw, fh) = comp.effective_footprint();
                let pr = comp.effective_port_row();
                for dr in 0..fh {
                    if let Some(r) = ar.checked_sub(pr).and_then(|b| b.checked_add(dr)) {
                        for dc in 0..fw {
                            if dr == pr && dc == 0 { continue; } // anchor itself
                            self.satellites.remove(&(r, ac + dc));
                        }
                    }
                }
            }
        }
        self.cells[ar][ac] = None;
    }

    /// Place a composite component at (anchor_r, anchor_c) and register satellite cells.
    /// Caller must verify the footprint is clear first.
    pub fn place_composite(&mut self, anchor_r: usize, anchor_c: usize, comp: Component) {
        debug_assert!(comp.effective_is_composite());
        let (fw, fh) = comp.effective_footprint();
        let pr = comp.effective_port_row();

        self.set(anchor_r, anchor_c, Some(comp));

        for dr in 0..fh {
            if let Some(r) = anchor_r.checked_sub(pr).and_then(|b| b.checked_add(dr)) {
                for dc in 0..fw {
                    if dr == pr && dc == 0 { continue; } // the anchor itself
                    let c = anchor_c + dc;
                    if r < self.height && c < self.width {
                        self.satellites.insert((r, c), (anchor_r, anchor_c));
                    }
                }
            }
        }
    }

    /// Rebuild the satellite map from anchor cells — must be called after deserialisation.
    pub fn rebuild_satellites(&mut self) {
        self.satellites.clear();
        for r in 0..self.height {
            for c in 0..self.width {
                if let Some(comp) = self.cells[r][c].as_ref() {
                    if comp.effective_is_composite() {
                        let (fw, fh) = comp.effective_footprint();
                        let pr = comp.effective_port_row();
                        for dr in 0..fh {
                            if let Some(row) = r.checked_sub(pr).and_then(|b| b.checked_add(dr)) {
                                for dc in 0..fw {
                                    if dr == pr && dc == 0 { continue; }
                                    let col = c + dc;
                                    if row < self.height && col < self.width {
                                        self.satellites.insert((row, col), (r, c));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Expand the grid to at least `min_w` × `min_h`, filling new cells with `None`.
    pub fn ensure_size(&mut self, min_w: usize, min_h: usize) {
        if min_h > self.height {
            for _ in self.height..min_h {
                self.cells.push(vec![None; self.width]);
            }
            self.height = min_h;
        }
        if min_w > self.width {
            for row in &mut self.cells {
                row.resize(min_w, None);
            }
            self.width = min_w;
        }
    }

    /// True when two cell positions have a connected port interface.
    /// Resolves satellite positions to their anchors first.
    /// Handles composite component footprints for east/west adjacency.
    pub fn are_connected(&self, r1: usize, c1: usize, r2: usize, c2: usize) -> bool {
        let (r1, c1) = self.effective_pos(r1, c1);
        let (r2, c2) = self.effective_pos(r2, c2);
        if r1 == r2 && c1 == c2 { return false; } // same component after resolution

        let Some(a) = self.get(r1, c1) else { return false; };
        let Some(b) = self.get(r2, c2) else { return false; };
        let (an, as_, ae, aw) = a.connections();
        let (bn, bs, be, bw) = b.connections();
        let a_fw = a.effective_footprint().0;
        let b_fw = b.effective_footprint().0;

        if r2 + 1 == r1 && c1 == c2 { return an && bs; }  // b directly north of a
        if r1 + 1 == r2 && c1 == c2 { return as_ && bn; } // b directly south of a
        // b is west of a: b's east edge (c2 + b_fw - 1) abuts a's west face (c1)
        if r1 == r2 && c2 + b_fw == c1 { return aw && be; }
        // b is east of a: a's east edge (c1 + a_fw - 1) abuts b's west face (c2)
        if r1 == r2 && c1 + a_fw == c2 { return ae && bw; }

        // North inlet port (e.g. BasinSink): water enters from above the top-center.
        // The inlet cell is one row above the composite's top edge at its horizontal center.
        if let Some((dr, dc)) = a.composite_north_inlet_offset() {
            let ir = r1 as isize + dr;
            let ic = c1 as isize + dc;
            if ir >= 0 && ic >= 0 && r2 == ir as usize && c2 == ic as usize {
                return b.connections().1; // b must have a south port
            }
        }
        if let Some((dr, dc)) = b.composite_north_inlet_offset() {
            let ir = r2 as isize + dr;
            let ic = c2 as isize + dc;
            if ir >= 0 && ic >= 0 && r1 == ir as usize && c1 == ic as usize {
                return a.connections().1; // a must have a south port
            }
        }

        // South drain port (e.g. BasinSink): a composite's drain is at a fixed offset
        // below the anchor and at its horizontal center. Check both a→b and b→a.
        if let Some((dr, dc)) = a.composite_south_drain_offset() {
            let drain_r = (r1 as isize + dr) as usize;
            let drain_c = (c1 as isize + dc) as usize;
            if r2 == drain_r && c2 == drain_c {
                // drain port of a connects to b — b must accept from its north port
                return b.connections().0; // bn
            }
        }
        if let Some((dr, dc)) = b.composite_south_drain_offset() {
            let drain_r = (r2 as isize + dr) as usize;
            let drain_c = (c2 as isize + dc) as usize;
            if r1 == drain_r && c1 == drain_c {
                // a sits at b's drain port location — a must have a north port
                return a.connections().0; // an
            }
        }
        false
    }
}
