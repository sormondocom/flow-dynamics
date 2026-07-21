use flow_dynamics::components::{Component, ComponentKind, PipeDiameter, PipeMaterial, ValveState};

fn make_comp(kind: ComponentKind) -> Component {
    Component::new(kind, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

// ── connections() known values ─────────────────────────────────────────────────

#[test]
fn test_connections_source_all_four() {
    let (n, s, e, w) = ComponentKind::Source.connections();
    assert!(n && s && e && w);
}

#[test]
fn test_connections_sink_all_four() {
    let (n, s, e, w) = ComponentKind::Sink.connections();
    assert!(n && s && e && w);
}

#[test]
fn test_connections_pipe_h_east_west_only() {
    let (n, s, e, w) = ComponentKind::PipeH.connections();
    assert!(!n && !s && e && w);
}

#[test]
fn test_connections_pipe_v_north_south_only() {
    let (n, s, e, w) = ComponentKind::PipeV.connections();
    assert!(n && s && !e && !w);
}

#[test]
fn test_connections_elbow_ne() {
    let (n, s, e, w) = ComponentKind::ElbowNE.connections();
    assert!(n && !s && e && !w);
}

#[test]
fn test_connections_elbow_nw() {
    let (n, s, e, w) = ComponentKind::ElbowNW.connections();
    assert!(n && !s && !e && w);
}

#[test]
fn test_connections_elbow_se() {
    let (n, s, e, w) = ComponentKind::ElbowSE.connections();
    assert!(!n && s && e && !w);
}

#[test]
fn test_connections_elbow_sw() {
    let (n, s, e, w) = ComponentKind::ElbowSW.connections();
    assert!(!n && s && !e && w);
}

#[test]
fn test_connections_tee_nse() {
    let (n, s, e, w) = ComponentKind::TeeNSE.connections();
    assert!(n && s && e && !w);
}

#[test]
fn test_connections_tee_nsw() {
    let (n, s, e, w) = ComponentKind::TeeNSW.connections();
    assert!(n && s && !e && w);
}

#[test]
fn test_connections_tee_new() {
    let (n, s, e, w) = ComponentKind::TeeNEW.connections();
    assert!(n && !s && e && w);
}

#[test]
fn test_connections_tee_sew() {
    let (n, s, e, w) = ComponentKind::TeeSEW.connections();
    assert!(!n && s && e && w);
}

#[test]
fn test_connections_cross_all_four() {
    let (n, s, e, w) = ComponentKind::Cross.connections();
    assert!(n && s && e && w);
}

#[test]
fn test_connections_ball_valve_h_east_west() {
    let (n, s, e, w) = ComponentKind::BallValveH.connections();
    assert!(!n && !s && e && w);
}

#[test]
fn test_connections_ball_valve_v_north_south() {
    let (n, s, e, w) = ComponentKind::BallValveV.connections();
    assert!(n && s && !e && !w);
}

#[test]
fn test_connections_check_valve_h_east_west() {
    let (n, s, e, w) = ComponentKind::CheckValveH.connections();
    assert!(!n && !s && e && w);
}

#[test]
fn test_connections_label_no_connections() {
    let (n, s, e, w) = ComponentKind::Label.connections();
    assert!(!n && !s && !e && !w);
}

#[test]
fn test_connections_note_no_connections() {
    let (n, s, e, w) = ComponentKind::Note.connections();
    assert!(!n && !s && !e && !w);
}

#[test]
fn test_connections_link_no_connections() {
    let (n, s, e, w) = ComponentKind::Link.connections();
    assert!(!n && !s && !e && !w);
}

// ── connections() exhaustive — no panics ──────────────────────────────────────

#[test]
fn test_connections_no_panic_all_kinds() {
    for &kind in ComponentKind::all_palette() {
        let _ = kind.connections(); // must not panic
    }
    let _ = ComponentKind::Custom.connections();
}

// ── symbol() ─────────────────────────────────────────────────────────────────

#[test]
fn test_symbol_non_empty_all_kinds() {
    for &kind in ComponentKind::all_palette() {
        let ch = kind.symbol();
        assert!(ch != '\0', "{kind:?} returned NUL symbol");
        assert!(!ch.is_ascii_control() || ch == '\0', "unexpected control char for {kind:?}");
    }
}

// ── footprint() ──────────────────────────────────────────────────────────────

#[test]
fn test_footprint_positive_all_kinds() {
    for &kind in ComponentKind::all_palette() {
        let (w, h) = kind.footprint();
        assert!(w >= 1, "{kind:?} footprint width < 1");
        assert!(h >= 1, "{kind:?} footprint height < 1");
    }
}

#[test]
fn test_footprint_water_softener() {
    assert_eq!(ComponentKind::WaterSoftener.footprint(), (17, 5));
}

#[test]
fn test_footprint_water_heater() {
    assert_eq!(ComponentKind::WaterHeater.footprint(), (15, 5));
}

#[test]
fn test_footprint_toilet() {
    assert_eq!(ComponentKind::Toilet.footprint(), (11, 5));
}

#[test]
fn test_footprint_basin_sink() {
    assert_eq!(ComponentKind::BasinSink.footprint(), (13, 5));
}

// ── is_composite() ────────────────────────────────────────────────────────────

#[test]
fn test_is_composite_matches_footprint() {
    for &kind in ComponentKind::all_palette() {
        let (fw, _) = kind.footprint();
        assert_eq!(
            kind.is_composite(),
            fw > 1,
            "{kind:?}: is_composite() mismatch with footprint width {fw}"
        );
    }
}

// ── is_annotation() ──────────────────────────────────────────────────────────

#[test]
fn test_is_annotation_only_label_note_link() {
    use ComponentKind::*;
    for &kind in ComponentKind::all_palette() {
        let expected = matches!(kind, Label | Note | Link);
        assert_eq!(
            kind.is_annotation(),
            expected,
            "{kind:?}: is_annotation() should be {expected}"
        );
    }
}

// ── is_valve() ───────────────────────────────────────────────────────────────

#[test]
fn test_is_valve_only_ball_valves() {
    for &kind in ComponentKind::all_palette() {
        let expected = matches!(kind, ComponentKind::BallValveH | ComponentKind::BallValveV);
        assert_eq!(
            kind.is_valve(),
            expected,
            "{kind:?}: is_valve() should be {expected}"
        );
    }
}

// ── equiv_length_diameters() ──────────────────────────────────────────────────

#[test]
fn test_equiv_length_finite_nonnegative_all_kinds() {
    for &kind in ComponentKind::all_palette() {
        let v = kind.equiv_length_diameters();
        assert!(v >= 0.0, "{kind:?}: equiv_length_diameters negative");
        assert!(v.is_finite(), "{kind:?}: equiv_length_diameters not finite");
    }
}

// ── toggle_valve() ────────────────────────────────────────────────────────────

#[test]
fn test_toggle_valve_cycles_open_closed_open() {
    let mut comp = make_comp(ComponentKind::BallValveH);
    assert_eq!(comp.valve_state, Some(ValveState::Open));
    comp.toggle_valve();
    assert_eq!(comp.valve_state, Some(ValveState::Closed));
    comp.toggle_valve();
    assert_eq!(comp.valve_state, Some(ValveState::Open));
}

#[test]
fn test_toggle_valve_noop_on_non_valve() {
    let mut comp = make_comp(ComponentKind::PipeH);
    assert_eq!(comp.valve_state, None);
    comp.toggle_valve();
    assert_eq!(comp.valve_state, None);
}

// ── Component::new() defaults ─────────────────────────────────────────────────

#[test]
fn test_component_new_pipe_no_valve_state() {
    let c = make_comp(ComponentKind::PipeH);
    assert_eq!(c.valve_state, None);
}

#[test]
fn test_component_new_valve_starts_open() {
    let c = make_comp(ComponentKind::BallValveH);
    assert_eq!(c.valve_state, Some(ValveState::Open));
}

#[test]
fn test_component_new_default_pipe_length() {
    let c = make_comp(ComponentKind::PipeH);
    assert!((c.pipe_length - 1.0).abs() < 1e-6);
}

#[test]
fn test_component_new_default_source_pressure() {
    let c = make_comp(ComponentKind::Source);
    assert!((c.source_pressure_psi - 60.0).abs() < 1e-6);
}

#[test]
fn test_component_new_no_custom_id() {
    let c = make_comp(ComponentKind::PipeH);
    assert!(c.custom_id.is_none());
}

#[test]
fn test_component_new_arm_lengths_zero() {
    let c = make_comp(ComponentKind::TeeNSE);
    assert_eq!(c.arm_lengths, [0.0; 4]);
}
