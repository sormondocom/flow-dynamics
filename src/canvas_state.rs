use crate::grid::Grid;

pub struct CanvasState {
    pub grid: Grid,
    pub cursor: (usize, usize),
    pub viewport: (usize, usize),
}

impl CanvasState {
    pub fn new(grid_cols: usize, grid_rows: usize) -> Self {
        Self {
            grid: Grid::new(grid_cols, grid_rows),
            cursor: (0, 0),
            viewport: (0, 0),
        }
    }
}
