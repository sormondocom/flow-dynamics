use std::collections::{HashMap, HashSet, VecDeque};

use crate::components::ComponentKind;
use crate::fluid::FluidType;
use crate::glyphs::{GlyphRegistry, PortFace};
use crate::grid::Grid;

// ── Hazen-Williams constants ──────────────────────────────────────────────────
// Q (GPM) = HW_K × C × d_in^2.63 × (ΔP_psi / L_ft)^N_INV
// ΔP_psi  = R × Q^N_EXP   where R = L / (HW_R_CONST × C^N_EXP × d_in^4.871)

const N_EXP: f32 = 1.852;
const N_INV: f32 = 0.5403; // 1 / 1.852
const HW_R_CONST: f32 = 0.4879; // HW_K^1.852 = 0.6790^1.852

// Velocity: V_fps = VEL_K × Q_gpm / d_in²
const VEL_K: f32 = 0.4085;

// Blocked / effectively-infinite resistance sentinel
const R_INF: f32 = 1.0e12;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowState {
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
        let fw = comp.effective_footprint().0;
        let mut n = Vec::new();

        // For custom composites with explicit ports, port external cells replace the
        // default E/W footprint neighbors; otherwise use the legacy E/W behavior.
        let custom_has_ports = comp.kind == ComponentKind::Custom
            && comp.custom_id.as_ref().map_or(false, |id| {
                registry.custom_components().iter().any(|d| &d.id == id && !d.ports.is_empty())
            });

        if !custom_has_ports {
            if c > 0 { n.push((r, c - 1)); }
            if c + fw < grid.width { n.push((r, c + fw)); }
        }

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
    let viscosity_scale = fluid.viscosity_scale();
    let mut result = SimResult::default();

    // ── Phase 1: BFS reachability (determines FlowState) ─────────────────────
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
        }
    }

    // Reclassify dead-ends
    for (pos, state) in result.cell_states.iter_mut() {
        if *state != FlowState::Flowing { continue; }
        let (r, c) = *pos;
        let kind = match grid.get(r, c) { Some(co) => co.kind, None => continue };
        if matches!(kind, ComponentKind::Source | ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet) { continue; }
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

    // ── Phase 2: Hazen-Williams nodal pressure solver ─────────────────────────
    // Collect the set of flowing nodes and build an adjacency map.
    let flowing_nodes: Vec<(usize, usize)> = result
        .cell_states
        .iter()
        .filter(|(_, s)| **s == FlowState::Flowing)
        .map(|(&pos, _)| pos)
        .collect();

    if flowing_nodes.is_empty() {
        return result;
    }

    // Build adjacency for flowing subgraph
    let mut adjacency: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for &pos in &flowing_nodes {
        let (r, c) = pos;
        let neighbors: Vec<(usize, usize)> = {
            let comp = grid.get(r, c).unwrap();
            candidate_neighbors(grid, r, c, comp, registry)
                .into_iter()
                .filter(|nb| {
                    result.cell_states.get(nb) == Some(&FlowState::Flowing)
                        && sim_are_connected(grid, r, c, nb.0, nb.1, registry)
                })
                .collect()
        };
        adjacency.insert(pos, neighbors);
    }

    // Precompute edge resistances
    let mut edge_res: HashMap<((usize, usize), (usize, usize)), f32> = HashMap::new();
    for (&pos, neighbors) in &adjacency {
        for &nb in neighbors {
            let key = if pos <= nb { (pos, nb) } else { (nb, pos) };
            edge_res.entry(key).or_insert_with(|| {
                let r_a = cell_resistance(grid, pos, viscosity_scale);
                let r_b = cell_resistance(grid, nb, viscosity_scale);
                if r_a >= R_INF / 2.0 || r_b >= R_INF / 2.0 {
                    R_INF
                } else {
                    (r_a + r_b) / 2.0
                }
            });
        }
    }

    let get_edge_r = |a: (usize, usize), b: (usize, usize)| -> f32 {
        let key = if a <= b { (a, b) } else { (b, a) };
        *edge_res.get(&key).unwrap_or(&R_INF)
    };

    // Find max source pressure for initial guess
    let max_source_p: f32 = sources
        .iter()
        .filter_map(|&s| grid.get(s.0, s.1).map(|c| c.source_pressure_psi))
        .fold(60.0_f32, f32::max);

    // Initialize pressures
    let mut pressures: HashMap<(usize, usize), f32> = HashMap::new();
    for &pos in &flowing_nodes {
        let p = match grid.get(pos.0, pos.1).map(|c| c.kind) {
            Some(ComponentKind::Source) => grid.get(pos.0, pos.1).unwrap().source_pressure_psi,
            Some(ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet) => 0.0,
            _ => max_source_p * 0.5,
        };
        pressures.insert(pos, p);
    }

    // Gauss-Seidel with local Newton-Raphson per interior node
    for _iter in 0..150 {
        let mut max_change = 0.0_f32;

        for &pos in &flowing_nodes {
            let kind = grid.get(pos.0, pos.1).map(|c| c.kind);
            if matches!(kind, Some(ComponentKind::Source) | Some(ComponentKind::Sink) | Some(ComponentKind::Toilet) | Some(ComponentKind::Faucet)) {
                continue;
            }
            let neighbors = match adjacency.get(&pos) {
                Some(n) => n,
                None => continue,
            };
            if neighbors.is_empty() {
                continue;
            }

            let p_old = *pressures.get(&pos).unwrap_or(&0.0);
            let mut p = p_old;

            // Newton-Raphson: find P_pos s.t. sum of Q_pos_j = 0
            for _ in 0..20 {
                let mut f_val = 0.0_f32;
                let mut df_val = 0.0_f32;

                for &nb in neighbors {
                    let p_nb = *pressures.get(&nb).unwrap_or(&0.0);
                    let r = get_edge_r(pos, nb);
                    if r >= R_INF / 2.0 {
                        continue;
                    }
                    let dp = p - p_nb;
                    let dp_abs = dp.abs().max(1e-7);
                    // Q_ij = sign(dp) × (|dp| / R)^N_INV
                    f_val += dp.signum() * (dp_abs / r).powf(N_INV);
                    // dQ/dp = N_INV × |dp|^(N_INV-1) / R^N_INV
                    df_val += N_INV * dp_abs.powf(N_INV - 1.0) / r.powf(N_INV);
                }

                if df_val.abs() < 1e-14 {
                    break;
                }
                let corr = f_val / df_val;
                p -= corr;
                p = p.clamp(0.0, max_source_p);
                if corr.abs() < 1e-4 {
                    break;
                }
            }

            let change = (p - p_old).abs();
            if change > max_change {
                max_change = change;
            }
            pressures.insert(pos, p);
        }

        if max_change < 0.01 {
            break;
        }
    }

    // ── Phase 3: Compute per-node flow data from solved pressures ─────────────
    for &pos in &flowing_nodes {
        let p = *pressures.get(&pos).unwrap_or(&0.0);
        let comp = match grid.get(pos.0, pos.1) {
            Some(c) => c,
            None => continue,
        };

        // Max absolute flow on any adjacent edge ≈ throughput of this component
        let max_flow: f32 = adjacency
            .get(&pos)
            .map(|nbs| {
                nbs.iter()
                    .map(|&nb| {
                        let p_nb = *pressures.get(&nb).unwrap_or(&0.0);
                        let r = get_edge_r(pos, nb);
                        if r >= R_INF / 2.0 {
                            0.0_f32
                        } else {
                            let dp = (p - p_nb).abs().max(1e-7);
                            (dp / r).powf(N_INV)
                        }
                    })
                    .fold(0.0_f32, f32::max)
            })
            .unwrap_or(0.0);

        let d = comp.diameter.inner_diameter_in();
        let velocity = if d > 0.0 { VEL_K * max_flow / (d * d) } else { 0.0 };

        result.flow_data.insert(
            pos,
            NodeFlowData {
                pressure_psi: p,
                flow_gpm: max_flow,
                velocity_fps: velocity,
            },
        );

        // Warn when a sink's incoming pressure is below 20 PSI minimum
        let is_fixture = matches!(comp.kind, ComponentKind::Sink | ComponentKind::Toilet | ComponentKind::Faucet | ComponentKind::BasinSink);
        if is_fixture && p < 20.0 && p > 0.01 {
            result.warnings.push(format!(
                "Low pressure at fixture ({},{}): {:.1} PSI (min 20 PSI recommended).",
                pos.0, pos.1, p
            ));
        }
    }

    // Summarise velocity violations across all pipe segments
    let mut vel_violations: Vec<((usize, usize), f32, f32)> = Vec::new(); // (pos, actual, limit)
    for (&pos, fd) in &result.flow_data {
        if let Some(comp) = grid.get(pos.0, pos.1) {
            if matches!(comp.kind, ComponentKind::PipeH | ComponentKind::PipeV) {
                let limit = comp.material.max_velocity_fps();
                if fd.velocity_fps > limit {
                    vel_violations.push((pos, fd.velocity_fps, limit));
                }
            }
        }
    }
    if !vel_violations.is_empty() {
        // Sort by worst exceedance first for the summary message
        vel_violations.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let (worst_pos, worst_vel, worst_lim) = vel_violations[0];
        result.warnings.push(format!(
            "{} pipe(s) exceed velocity limit — worst: {:.1} ft/s at ({},{}) (max {:.0} ft/s).",
            vel_violations.len(), worst_vel, worst_pos.0, worst_pos.1, worst_lim
        ));
    }

    result
}

// ── Resistance helpers ────────────────────────────────────────────────────────

/// Hazen-Williams resistance for a cell: ΔP_psi = R × Q_gpm^1.852
fn cell_resistance(grid: &Grid, pos: (usize, usize), viscosity_scale: f32) -> f32 {
    let comp = match grid.get(pos.0, pos.1) {
        Some(c) => c,
        None => return 0.0,
    };

    if !comp.is_passable() {
        return R_INF;
    }

    let l_ft = comp.equiv_length_ft();
    if l_ft <= 0.0 {
        return 0.0;
    }

    let d_in = comp.diameter.inner_diameter_in();
    let c = comp.material.c_value();

    // R = L / (HW_R_CONST × C^1.852 × d^4.871) × viscosity_scale
    let denom = HW_R_CONST * c.powf(N_EXP) * d_in.powf(4.871);
    if denom < 1e-15 {
        R_INF
    } else {
        (l_ft / denom) * viscosity_scale
    }
}
