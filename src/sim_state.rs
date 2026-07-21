use crate::fluid::FluidType;
use crate::grid::Grid;
use crate::simulation::SimResult;

#[derive(Default)]
pub struct SimState {
    pub sim_result: Option<SimResult>,
    pub fluid_type: FluidType,
    pub splash_grid: Option<Grid>,
    pub splash_sim: Option<SimResult>,
    /// Set by refresh_sim() so the main loop knows to drain any stale background results.
    pub sim_refreshed: bool,
}
