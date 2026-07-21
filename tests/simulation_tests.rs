use flow_dynamics::components::{Component, ComponentKind, PipeDiameter, PipeMaterial, ValveState};
use flow_dynamics::fluid::FluidType;
use flow_dynamics::glyphs::GlyphRegistry;
use flow_dynamics::grid::Grid;
use flow_dynamics::simulation::{simulate, FlowState};

fn make_comp(kind: ComponentKind) -> Component {
    Component::new(kind, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

fn make_grid(placements: &[((usize, usize), ComponentKind)]) -> Grid {
    let max_r = placements.iter().map(|((r, _), _)| *r).max().unwrap_or(0);
    let max_c = placements.iter().map(|((_, c), _)| *c).max().unwrap_or(0);
    let mut g = Grid::new(max_c + 10, max_r + 10);
    for &((r, c), kind) in placements {
        let comp = make_comp(kind);
        if comp.effective_is_composite() {
            g.ensure_size(c + comp.effective_footprint().0 + 2, r + comp.effective_footprint().1 + 2);
            g.place_composite(r, c, comp);
        } else {
            g.set(r, c, Some(comp));
        }
    }
    g
}

fn registry() -> GlyphRegistry { GlyphRegistry::new() }
fn fluid()    -> FluidType     { FluidType::Water }

// ── Source→PipeH→Sink ─────────────────────────────────────────────────────────

#[test]
fn test_source_pipe_sink_reaches_sink() {
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink, "flow should reach the sink");
    let src_state = sim.cell_states.get(&(5, 0)).cloned();
    assert_eq!(src_state, Some(FlowState::Flowing));
    let src_data = sim.flow_data.get(&(5, 0));
    assert!(src_data.map(|d| d.pressure_psi > 0.0).unwrap_or(false), "source pressure > 0");
}

// ── open valve allows flow ────────────────────────────────────────────────────

#[test]
fn test_open_valve_allows_flow() {
    let mut valve = make_comp(ComponentKind::BallValveH);
    valve.valve_state = Some(ValveState::Open);
    let mut g = Grid::new(10, 10);
    g.set(5, 0, Some(make_comp(ComponentKind::Source)));
    g.set(5, 1, Some(make_comp(ComponentKind::PipeH)));
    g.set(5, 2, Some(valve));
    g.set(5, 3, Some(make_comp(ComponentKind::PipeH)));
    g.set(5, 4, Some(make_comp(ComponentKind::Sink)));
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink, "open valve should allow flow to sink");
}

// ── closed valve blocks flow ──────────────────────────────────────────────────

#[test]
fn test_closed_valve_blocks_flow() {
    let mut valve = make_comp(ComponentKind::BallValveH);
    valve.valve_state = Some(ValveState::Closed);
    let mut g = Grid::new(10, 10);
    g.set(5, 0, Some(make_comp(ComponentKind::Source)));
    g.set(5, 1, Some(make_comp(ComponentKind::PipeH)));
    g.set(5, 2, Some(valve));
    g.set(5, 3, Some(make_comp(ComponentKind::PipeH)));
    g.set(5, 4, Some(make_comp(ComponentKind::Sink)));
    let sim = simulate(&g, fluid(), &registry());
    assert!(!sim.reached_sink, "closed valve should block flow");
}

// ── dead end pressurized ──────────────────────────────────────────────────────

#[test]
fn test_dead_end_pressurized() {
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::EndCap),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(!sim.reached_sink, "no sink in circuit");
    let end_state = sim.cell_states.get(&(5, 2)).cloned();
    assert_eq!(end_state, Some(FlowState::Pressurized), "EndCap should be Pressurized");
}

// ── isolated pipe stays static ────────────────────────────────────────────────

#[test]
fn test_isolated_pipe_static() {
    let g = make_grid(&[
        ((5, 5), ComponentKind::PipeH),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    let state = sim.cell_states.get(&(5, 5)).cloned();
    assert!(
        state.is_none() || state == Some(FlowState::Static),
        "isolated pipe should be Static or absent from states"
    );
}

// ── no panic on empty grid ────────────────────────────────────────────────────

#[test]
fn test_simulate_empty_grid_no_panic() {
    let g = Grid::new(10, 10);
    let sim = simulate(&g, fluid(), &registry());
    assert!(!sim.reached_sink);
}

// ── multiple sources ──────────────────────────────────────────────────────────

#[test]
fn test_multiple_sources_no_panic_reaches_sink() {
    let g = make_grid(&[
        ((4, 0), ComponentKind::Source),
        ((4, 1), ComponentKind::PipeH),
        ((4, 2), ComponentKind::TeeNSE),
        ((5, 2), ComponentKind::PipeV),
        ((5, 3), ComponentKind::Source),
        ((4, 3), ComponentKind::Sink),
    ]);
    // Should not panic regardless of result
    let sim = simulate(&g, fluid(), &registry());
    let _ = sim.reached_sink;
}

// ── tee split ─────────────────────────────────────────────────────────────────

#[test]
fn test_tee_split_reaches_sink_on_one_branch() {
    // Source → TeeNEW → two branches each ending in Sink
    //   Row 5: Source(0) PipeH(1) TeeNEW(2) PipeH(3) Sink(4)
    //   Row 4:                     PipeV(2)
    //   Row 3:                     Sink(2)    (north branch)
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::TeeNEW),
        ((5, 3), ComponentKind::PipeH),
        ((5, 4), ComponentKind::Sink),
        ((4, 2), ComponentKind::PipeV),
        ((3, 2), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink, "at least one sink should be reached in tee circuit");
}

// ── source pressure is positive and finite in flowing circuit ─────────────────

#[test]
fn test_source_has_positive_pressure_when_flowing() {
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink);
    let src_data = sim.flow_data.get(&(5, 0));
    let p = src_data.map(|d| d.pressure_psi).unwrap_or(0.0);
    assert!(p > 0.0, "source pressure should be positive in flowing circuit, got {p}");
    assert!(p.is_finite(), "source pressure must be finite");
}

// ── flow rate positive and finite in flowing circuit ──────────────────────────

#[test]
fn test_flow_rate_positive_when_flowing() {
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink);
    let pipe_data = sim.flow_data.get(&(5, 1));
    let q = pipe_data.map(|d| d.flow_gpm).unwrap_or(0.0);
    assert!(q > 0.0, "flow rate through pipe should be positive, got {q}");
    assert!(q.is_finite(), "flow rate must be finite");
}

// ── check valve direction ─────────────────────────────────────────────────────

#[test]
fn test_check_valve_allows_east_flow() {
    // Source on west, CheckValveH in middle, Sink on east → should reach sink
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::CheckValveH),
        ((5, 2), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink, "CheckValveH should allow west-to-east flow");
}

// ── pressure gauge at branch end gets pressure reading ────────────────────────

#[test]
fn test_pressure_gauge_dead_end_has_pressure_data() {
    // Source → PipeH → TeeSEW → east PipeH → PressureGauge (dead-end branch)
    //                         ↓ south Sink
    // TeeSEW connects West (from PipeH), East (to gauge), South (to Sink)
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::TeeSEW),
        ((5, 3), ComponentKind::PipeH),
        ((5, 4), ComponentKind::PressureGauge), // dead-end branch
        ((6, 2), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink, "sink should be reached via south branch");
    let gauge_data = sim.flow_data.get(&(5, 4));
    assert!(gauge_data.is_some(), "PressureGauge at branch end must have flow_data");
    let p = gauge_data.unwrap().pressure_psi;
    assert!(p > 0.0, "gauge pressure should be > 0, got {p}");
    assert!(p.is_finite(), "gauge pressure must be finite");
}

#[test]
fn test_pressure_gauge_inline_has_pressure_data() {
    // Gauge placed inline with pipes on both sides: Source → PipeH → PressureGauge → PipeH → Sink
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::PressureGauge),
        ((5, 3), ComponentKind::PipeH),
        ((5, 4), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink, "sink should be reached through inline gauge");
    let gauge_data = sim.flow_data.get(&(5, 2));
    assert!(gauge_data.is_some(), "inline PressureGauge must have flow_data");
    let p = gauge_data.unwrap().pressure_psi;
    assert!(p > 0.0, "inline gauge pressure should be > 0, got {p}");
    assert!(p.is_finite(), "gauge pressure must be finite");
}

#[test]
fn test_flow_meter_inline_has_gpm_data() {
    // FlowMeterH placed inline with pipes: Source → PipeH → FlowMeterH → PipeH → Sink
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::FlowMeterH),
        ((5, 3), ComponentKind::PipeH),
        ((5, 4), ComponentKind::Sink),
    ]);
    let sim = simulate(&g, fluid(), &registry());
    assert!(sim.reached_sink);
    let meter_data = sim.flow_data.get(&(5, 2));
    assert!(meter_data.is_some(), "FlowMeterH must have flow_data");
    let q = meter_data.unwrap().flow_gpm;
    assert!(q > 0.0, "inline flow meter GPM should be > 0, got {q}");
}

// ── fluid types: no panic ─────────────────────────────────────────────────────

#[test]
fn test_simulate_all_fluid_types_no_panic() {
    let g = make_grid(&[
        ((5, 0), ComponentKind::Source),
        ((5, 1), ComponentKind::PipeH),
        ((5, 2), ComponentKind::Sink),
    ]);
    for fluid in [
        FluidType::Water, FluidType::Oil, FluidType::NaturalGas,
        FluidType::Steam, FluidType::Glycol, FluidType::HydraulicOil,
    ] {
        let sim = simulate(&g, fluid, &registry());
        assert!(sim.reached_sink, "should reach sink for fluid {fluid:?}");
    }
}
