use std::collections::{HashMap, HashSet, VecDeque};

use crate::components::ComponentKind;
use crate::fluid::FluidType;
use crate::glyphs::{GlyphRegistry, PortFace};
use crate::grid::Grid;


// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowState {
    #[default]
    Static,
    Flowing,
    Pressurized,
}

#[derive(Debug, Clone, Default)]
pub struct NodeFlowData {
    pub pressure_psi: f32,
    pub flow_gpm: f32,
    pub velocity_fps: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SimResult {
    pub cell_states: HashMap<(usize, usize), FlowState>,
    pub flow_data: HashMap<(usize, usize), NodeFlowData>,
    /// Flow direction into each cell: (dr, dc) from upstream neighbor (-1/0/1 each).
    pub flow_dirs: HashMap<(usize, usize), (i8, i8)>,
    pub warnings: Vec<String>,
    pub reached_sink: bool,
}

// ── Neighbor helpers ──────────────────────────────────────────────────────────

/// Returns connected simulation neighbors for (r, c), resolving satellites to anchors.
/// For composite anchors uses footprint-aware port positions.
/// Returns an empty vec for satellite cells (they are not simulation nodes).
fn candidate_neighbors(
    grid: &Grid,
    r: usize, c: usize,
    comp: &crate::components::Component,
    registry: &GlyphRegistry,
) -> Vec<(usize, usize)> {
    if grid.satellite_anchor(r, c).is_some() { return vec![]; }

    let raw: Vec<(usize, usize)> = if comp.effective_is_composite() {
        let (fw, fh) = comp.effective_footprint();
        let mut n = Vec::new();

        // For custom composites with explicit ports, port external cells replace the
        // default E/W footprint neighbors; otherwise use the legacy E/W behavior.
        let custom_has_ports = comp.kind == ComponentKind::Custom
            && comp.custom_id.as_ref().is_some_and(|id| {
                registry.custom_components().iter().any(|d| &d.id == id && !d.ports.is_empty())
            });

        // Legacy single-cell BallValve (satellite cells occupied — no satellites registered):
        // fall back to the standard 4-neighbor scan so old diagrams stay connected.
        let ball_valve_legacy = matches!(comp.kind, ComponentKind::BallValveH | ComponentKind::BallValveV)
            && (if comp.kind == ComponentKind::BallValveH {
                !(c + 1 < grid.width && grid.satellite_anchor(r, c + 1) == Some((r, c)))
            } else {
                !(r + 1 < grid.height && grid.satellite_anchor(r + 1, c) == Some((r, c)))
            });

        if ball_valve_legacy {
            // Treat as ordinary single-cell component.
            if r > 0 { n.push((r - 1, c)); }
            if r + 1 < grid.height { n.push((r + 1, c)); }
            if c > 0 { n.push((r, c - 1)); }
            if c + 1 < grid.width { n.push((r, c + 1)); }
        } else {
        if !custom_has_ports {
            if fw > 1 {
                if c > 0 { n.push((r, c - 1)); }
                if c + fw < grid.width { n.push((r, c + fw)); }
            }
            // Tall composite (fh > 1): N/S external ports one row beyond top/bottom.
            if fh > 1 {
                let pr = comp.effective_port_row();
                if r >= pr && r - pr > 0 { n.push((r - pr - 1, c)); }       // North external
                let bot = r + (fh - 1 - pr);
                if bot + 1 < grid.height { n.push((bot + 1, c)); }           // South external
            }
        }
        } // end !ball_valve_legacy

        // Custom port external cells
        if let Some(id) = &comp.custom_id {
            if let Some(def) = registry.custom_components().iter().find(|d| &d.id == id) {
                for (row_off, col_off, _face) in def.port_external_offsets() {
                    let er = r as isize + row_off;
                    let ec = c as isize + col_off;
                    if er >= 0 && ec >= 0 {
                        let er = er as usize;
                        let ec = ec as usize;
                        if er < grid.height && ec < grid.width { n.push((er, ec)); }
                    }
                }
            }
        }

        // South drain port
        if let Some((dr, dc)) = comp.composite_south_drain_offset() {
            let drain_r = (r as isize + dr) as usize;
            let drain_c = (c as isize + dc) as usize;
            if drain_r < grid.height && drain_c < grid.width {
                n.push((drain_r, drain_c));
            }
        }
        // North inlet port (e.g. BasinSink)
        if let Some((dr, dc)) = comp.composite_north_inlet_offset() {
            let ir = r as isize + dr;
            let ic = c as isize + dc;
            if ir >= 0 && ic >= 0 {
                let ir = ir as usize;
                let ic = ic as usize;
                if ir < grid.height && ic < grid.width {
                    n.push((ir, ic));
                }
            }
        }
        n
    } else {
        match comp.kind {
            ComponentKind::CheckValveH => {
                if c + 1 < grid.width { vec![(r, c + 1)] } else { vec![] }
            }
            ComponentKind::CheckValveV => {
                if r + 1 < grid.height { vec![(r + 1, c)] } else { vec![] }
            }
            _ => {
                let mut n = Vec::new();
                if r > 0 { n.push((r - 1, c)); }
                if r + 1 < grid.height { n.push((r + 1, c)); }
                if c > 0 { n.push((r, c - 1)); }
                if c + 1 < grid.width { n.push((r, c + 1)); }
                n
            }
        }
    };

    raw.into_iter()
        .map(|(nr, nc)| grid.effective_pos(nr, nc))
        .filter(|&(nr, nc)| grid.get(nr, nc).is_some())
        .collect()
}

// ── Custom-port connectivity helpers ──────────────────────────────────────────

/// Checks whether component at (r_a, c_a) has a custom port whose external cell is (r_b, c_b),
/// and if so whether the component at (r_b, c_b) exposes the complementary connection face.
fn custom_port_connects(
    grid: &Grid,
    r_a: usize, c_a: usize,
    r_b: usize, c_b: usize,
    registry: &GlyphRegistry,
) -> bool {
    let (r_a, c_a) = grid.effective_pos(r_a, c_a);
    let Some(comp_a) = grid.get(r_a, c_a) else { return false; };
    if comp_a.kind != ComponentKind::Custom { return false; }
    let Some(id) = &comp_a.custom_id else { return false; };
    let Some(def) = registry.custom_components().iter().find(|d| &d.id == id) else { return false; };
    if def.ports.is_empty() { return false; };
    let Some(comp_b) = grid.get(r_b, c_b) else { return false; };
    let (bn, bs, be, bw) = comp_b.connections();
    for (row_off, col_off, face) in def.port_external_offsets() {
        let er = r_a as isize + row_off;
        let ec = c_a as isize + col_off;
        if er >= 0 && ec >= 0 && r_b == er as usize && c_b == ec as usize {
            return match face {
                PortFace::West  => be,
                PortFace::East  => bw,
                PortFace::North => bs,
                PortFace::South => bn,
            };
        }
    }
    false
}

/// Connectivity check used inside the simulation, extended with custom-port awareness.
fn sim_are_connected(
    grid: &Grid,
    r1: usize, c1: usize,
    r2: usize, c2: usize,
    registry: &GlyphRegistry,
) -> bool {
    grid.are_connected(r1, c1, r2, c2)
        || custom_port_connects(grid, r1, c1, r2, c2, registry)
        || custom_port_connects(grid, r2, c2, r1, c1, registry)
}

// ── Main entry point ──────────────────────────────────────────────────────────

pub fn simulate(grid: &Grid, fluid: FluidType, registry: &GlyphRegistry) -> SimResult {
    let _ = fluid;
    let mut result = SimResult::default();

    // ── Collect sources and sinks ─────────────────────────────────────────────
    let mut sources = vec![];
    let mut sinks: HashSet<(usize, usize)> = HashSet::new();

    for r in 0..grid.height {
        for c in 0..grid.width {
            if let Some(comp) = grid.get(r, c) {
                match comp.kind {
                    ComponentKind::Source => sources.push((r, c)),
                    ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet => {
                        sinks.insert((r, c));
                    }
                    _ => {}
                }
            }
        }
    }

    if sources.is_empty() {
        result.warnings.push("No Source (S) placed — fluid has no inlet.".into());
        return result;
    }
    if sinks.is_empty() {
        result.warnings.push("No Drain (D) placed — system has no outlet.".into());
    }

    // ── BFS reachability ──────────────────────────────────────────────────────
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    let mut propagated: HashSet<(usize, usize)> = HashSet::new();

    for src in &sources {
        visited.insert(*src);
        queue.push_back(*src);
        result.cell_states.insert(*src, FlowState::Flowing);
    }

    while let Some((r, c)) = queue.pop_front() {
        let comp = match grid.get(r, c) {
            Some(c) => c,
            None => continue,
        };

        if !comp.is_passable() {
            result.cell_states.insert((r, c), FlowState::Static);
            continue;
        }
        if comp.kind == ComponentKind::EndCap {
            continue;
        }

        let neighbors = candidate_neighbors(grid, r, c, comp, registry);

        for (nr, nc) in neighbors {
            if visited.contains(&(nr, nc)) || !sim_are_connected(grid, r, c, nr, nc, registry) {
                continue;
            }
            visited.insert((nr, nc));
            propagated.insert((r, c));
            result.cell_states.insert((nr, nc), FlowState::Flowing);
            queue.push_back((nr, nc));
            if sinks.contains(&(nr, nc)) {
                result.reached_sink = true;
            }
            // Record BFS propagation direction for animation (first writer wins).
            let dir_r = if r < nr { 1i8 } else if r > nr { -1i8 } else { 0i8 };
            let dir_c = if c < nc { 1i8 } else if c > nc { -1i8 } else { 0i8 };
            result.flow_dirs.entry((nr, nc)).or_insert((dir_r, dir_c));
        }
    }

    // Reclassify dead-ends
    for (pos, state) in result.cell_states.iter_mut() {
        if *state != FlowState::Flowing { continue; }
        let (r, c) = *pos;
        let kind = match grid.get(r, c) { Some(co) => co.kind, None => continue };
        if matches!(kind, ComponentKind::Source | ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet) { continue; }
        // Gauges and meters are valid branch terminals — leave them Flowing so the solver
        // can assign them a pressure and flow_data is populated for the footer display.
        if matches!(kind, ComponentKind::PressureGauge | ComponentKind::FlowMeterH | ComponentKind::FlowMeterV) { continue; }
        if !propagated.contains(pos) {
            *state = FlowState::Pressurized;
            if kind == ComponentKind::BasinSink {
                result.warnings.push(format!(
                    "Basin sink overflow at ({},{}): no drain pipe connected.", r, c
                ));
            } else if kind != ComponentKind::EndCap {
                result.warnings.push(format!(
                    "Dead-end at ({},{}): no outlet from {:?}.", r, c, kind
                ));
            }
        }
    }

    for src in &sources {
        if !propagated.contains(src) {
            result.warnings.push(format!(
                "Source at ({},{}) has no connected pipes.", src.0, src.1
            ));
        }
    }
    if !sinks.is_empty() && !result.reached_sink {
        result.warnings.push("Flow does not reach any Drain — check connections.".into());
    }

    result
}


// ── DWV Validation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct DwvResult {
    /// Per-cell DFU accumulation (reserved for future per-pipe DFU overlay)
    #[allow(dead_code)]
    pub pipe_dfu: HashMap<(usize, usize), u32>,
    /// Warning messages per fixture position
    pub fixture_warnings: HashMap<(usize, usize), String>,
    /// Top-level summary warnings
    pub warnings: Vec<String>,
    /// Total DFU load on the system
    pub total_dfu: u32,
    /// Whether every fixture has a P-trap within range
    pub all_trapped: bool,
    /// Whether at least one vent is present
    pub has_vent: bool,
}

/// Validate DWV connectivity: P-trap presence, venting, DFU load sizing.
/// Uses a simple BFS from each fixture drain — cheap and runs every tick.
pub fn validate_dwv(grid: &Grid) -> DwvResult {
    use crate::components::{ComponentKind, DrainDiameter};

    let mut result = DwvResult::default();
    let mut all_trapped = true;
    let mut has_vent = false;
    let mut total_dfu: u32 = 0;

    // DWV connections: only DWV-kind cells connect to other DWV-kind cells
    let dwv_neighbors = |r: usize, c: usize| -> Vec<(usize, usize)> {
        let comp = match grid.get(r, c) { Some(c) => c, None => return vec![] };
        let (cn, cs, ce, cw) = comp.kind.connections();
        let mut out = vec![];
        if cn && r > 0                       { if let Some(n) = grid.get(r-1, c) { if n.kind.is_dwv() || n.kind == ComponentKind::Sink { out.push((r-1, c)); } } }
        if cs && r + 1 < grid.height        { if let Some(n) = grid.get(r+1, c) { if n.kind.is_dwv() || n.kind == ComponentKind::Sink { out.push((r+1, c)); } } }
        if ce && c + 1 < grid.width         { if let Some(n) = grid.get(r, c+1) { if n.kind.is_dwv() || n.kind == ComponentKind::Sink { out.push((r, c+1)); } } }
        if cw && c > 0                       { if let Some(n) = grid.get(r, c-1) { if n.kind.is_dwv() || n.kind == ComponentKind::Sink { out.push((r, c-1)); } } }
        out
    };

    // Scan for vents
    for r in 0..grid.height {
        for c in 0..grid.width {
            if let Some(comp) = grid.get(r, c) {
                if comp.kind == ComponentKind::Vent {
                    has_vent = true;
                }
            }
        }
    }

    // Scan fixtures
    for r in 0..grid.height {
        for c in 0..grid.width {
            let comp = match grid.get(r, c) { Some(c) => c, None => continue };
            let dfu = comp.kind.dfu();
            if dfu == 0 { continue; }

            total_dfu += dfu;

            // BFS from fixture looking for PTrap within 10 hops of DWV pipe
            let mut found_trap = false;
            let mut visited: HashSet<(usize, usize)> = HashSet::new();
            let mut queue: VecDeque<(usize, usize, u32)> = VecDeque::new();

            // Find adjacent DWV cells from this fixture
            for dr in [-1i32, 0, 1] {
                for dc in [-1i32, 0, 1] {
                    if dr == 0 && dc == 0 { continue; }
                    if dr != 0 && dc != 0 { continue; } // cardinal only
                    let nr = r as i32 + dr;
                    let nc = c as i32 + dc;
                    if nr < 0 || nc < 0 { continue; }
                    let nr = nr as usize; let nc = nc as usize;
                    if let Some(n) = grid.get(nr, nc) {
                        if n.kind.is_dwv() {
                            queue.push_back((nr, nc, 0));
                            visited.insert((nr, nc));
                        }
                    }
                }
            }

            while let Some((nr, nc, dist)) = queue.pop_front() {
                if dist > 10 { break; }
                if let Some(nc_comp) = grid.get(nr, nc) {
                    if nc_comp.kind == ComponentKind::PTrap {
                        found_trap = true;
                        break;
                    }
                    for next in dwv_neighbors(nr, nc) {
                        if !visited.contains(&next) {
                            visited.insert(next);
                            queue.push_back((next.0, next.1, dist + 1));
                        }
                    }
                }
            }

            if !found_trap {
                all_trapped = false;
                result.fixture_warnings.insert((r, c),
                    format!("{} at ({},{}) has no P-trap within 10 cells", comp.kind.label(), r, c));
            }
        }
    }

    // DFU load and pipe sizing warnings
    let mut dfu_warnings = vec![];
    for r in 0..grid.height {
        for c in 0..grid.width {
            if let Some(comp) = grid.get(r, c) {
                if matches!(comp.kind, ComponentKind::DrainH | ComponentKind::DrainV | ComponentKind::DrainWye) {
                    // Simple: use total_dfu as the estimated load for now
                    let required = DrainDiameter::min_for_dfu(total_dfu);
                    if comp.drain_diameter.rank() < required.rank() {
                        dfu_warnings.push(format!(
                            "Drain at ({},{}) is {} but {total_dfu} DFU requires {}",
                            r, c,
                            comp.drain_diameter.label(),
                            required.label()
                        ));
                    }
                }
            }
        }
    }

    if !has_vent && total_dfu > 0 {
        result.warnings.push("No vent pipe found — required by code for drain systems.".into());
    }
    if !all_trapped {
        result.warnings.push("One or more fixtures lack a nearby P-trap.".into());
    }
    result.warnings.extend(dfu_warnings.into_iter().take(3)); // cap at 3 sizing warnings

    result.total_dfu = total_dfu;
    result.all_trapped = all_trapped;
    result.has_vent = has_vent;
    result
}
