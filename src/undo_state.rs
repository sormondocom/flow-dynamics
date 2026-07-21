use std::collections::HashMap;

use crate::components::Component;
use crate::grid::Grid;

pub const UNDO_MAX: usize = 50;

/// Stores only occupied cells — typically 50× smaller than a full Grid clone.
/// An empty layout snapshot stores zero entries.
#[derive(Debug, Clone)]
pub struct SparseGridSnapshot {
    pub width: usize,
    pub height: usize,
    pub occupied: HashMap<(usize, usize), Component>,
}

impl SparseGridSnapshot {
    pub fn capture(grid: &Grid) -> Self {
        let mut occupied = HashMap::new();
        for r in 0..grid.height {
            for c in 0..grid.width {
                if let Some(comp) = grid.get(r, c) {
                    occupied.insert((r, c), comp.clone());
                }
            }
        }
        Self { width: grid.width, height: grid.height, occupied }
    }

    /// Restore this snapshot into `grid`.
    /// Caller must call `grid.rebuild_satellites()` afterward.
    pub fn restore(&self, grid: &mut Grid) {
        for row in &mut grid.cells {
            for cell in row {
                *cell = None;
            }
        }
        grid.ensure_size(self.width, self.height);
        for (&(r, c), comp) in &self.occupied {
            grid.set(r, c, Some(comp.clone()));
        }
    }
}

pub struct UndoState {
    undo_stack: Vec<SparseGridSnapshot>,
    redo_stack: Vec<SparseGridSnapshot>,
}

impl UndoState {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Snapshot the grid before a mutation. Clears the redo stack.
    /// Evicts the oldest entry when the stack exceeds UNDO_MAX.
    pub fn push(&mut self, grid: &Grid) {
        let snap = SparseGridSnapshot::capture(grid);
        self.undo_stack.push(snap);
        if self.undo_stack.len() > UNDO_MAX {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Restore the previous grid state. Returns true if something was undone.
    /// Caller must call `grid.rebuild_satellites()` when this returns true.
    pub fn undo(&mut self, grid: &mut Grid) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            let current = SparseGridSnapshot::capture(grid);
            self.redo_stack.push(current);
            prev.restore(grid);
            true
        } else {
            false
        }
    }

    /// Re-apply a previously undone mutation. Returns true if something was redone.
    /// Caller must call `grid.rebuild_satellites()` when this returns true.
    pub fn redo(&mut self, grid: &mut Grid) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            let current = SparseGridSnapshot::capture(grid);
            self.undo_stack.push(current);
            next.restore(grid);
            true
        } else {
            false
        }
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}
