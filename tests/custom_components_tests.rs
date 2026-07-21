use flow_dynamics::components::{Component, ComponentKind, PipeDiameter, PipeMaterial};
use flow_dynamics::fluid::FluidType;
use flow_dynamics::glyphs::{CustomCompDef, CustomPort, GlyphDef, GlyphRegistry, PortKind};
use flow_dynamics::grid::Grid;
use flow_dynamics::simulation::simulate;

fn default_glyph() -> GlyphDef {
    GlyphDef { symbol: '◇', fg: [200, 200, 200] }
}

fn make_comp(kind: ComponentKind) -> Component {
    Component::new(kind, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

/// A simple single-cell custom component with E/W connections.
fn simple_custom_def(id: &str) -> CustomCompDef {
    let mut def = CustomCompDef::new(id.into(), id.into(), default_glyph());
    def.connections_nsew = [false, false, true, true]; // E/W only
    def
}

/// A composite custom component: 5 wide × 3 tall, W inlet + E outlet.
fn composite_custom_def(id: &str) -> CustomCompDef {
    let mut def = CustomCompDef::new(id.into(), id.into(), default_glyph());
    def.composite_size = Some((5, 3));
    def.ports = vec![
        CustomPort { name: "inlet_w".into(),  kind: PortKind::Inlet,  row: 1, col: 0 },
        CustomPort { name: "outlet_e".into(), kind: PortKind::Outlet, row: 1, col: 4 },
    ];
    def
}

fn registry_with_simple_custom() -> GlyphRegistry {
    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(simple_custom_def("simple_ew"));
    reg
}

fn registry_with_composite_custom() -> GlyphRegistry {
    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(composite_custom_def("comp_5x3"));
    reg
}

// ── registration ──────────────────────────────────────────────────────────────

#[test]
fn test_custom_component_registered() {
    let reg = registry_with_simple_custom();
    assert_eq!(reg.custom_components().len(), 1);
}

#[test]
fn test_custom_component_fields() {
    let reg = registry_with_simple_custom();
    let def = &reg.custom_components()[0];
    assert_eq!(def.id, "simple_ew");
    assert_eq!(def.label, "simple_ew");
    assert_eq!(def.connections_nsew, [false, false, true, true]);
}

#[test]
fn test_add_duplicate_id_replaces() {
    let mut reg = GlyphRegistry::new();
    let mut d1 = simple_custom_def("dup");
    d1.label = "first".into();
    reg.add_custom_component(d1);

    let mut d2 = simple_custom_def("dup");
    d2.label = "second".into();
    reg.add_custom_component(d2);

    assert_eq!(reg.custom_components().len(), 1, "duplicate id should replace");
    assert_eq!(reg.custom_components()[0].label, "second");
}

// ── composite ports ───────────────────────────────────────────────────────────

#[test]
fn test_composite_custom_port_count() {
    let reg = registry_with_composite_custom();
    let def = &reg.custom_components()[0];
    assert_eq!(def.ports.len(), 2, "composite should have 2 ports");
}

#[test]
fn test_composite_custom_port_kinds() {
    let reg = registry_with_composite_custom();
    let def = &reg.custom_components()[0];
    let kinds: Vec<&PortKind> = def.ports.iter().map(|p| &p.kind).collect();
    assert!(kinds.contains(&&PortKind::Inlet),  "should have Inlet port");
    assert!(kinds.contains(&&PortKind::Outlet), "should have Outlet port");
}

#[test]
fn test_composite_west_inlet_position() {
    let reg = registry_with_composite_custom();
    let def = &reg.custom_components()[0];
    let inlet = def.ports.iter().find(|p| p.name == "inlet_w").unwrap();
    assert_eq!(inlet.col, 0, "west inlet should be at col 0 in footprint");
    assert_eq!(inlet.row, 1, "west inlet should be at row 1 (middle of 3-row composite)");
}

#[test]
fn test_composite_east_outlet_position() {
    let reg = registry_with_composite_custom();
    let def = &reg.custom_components()[0];
    let outlet = def.ports.iter().find(|p| p.name == "outlet_e").unwrap();
    assert_eq!(outlet.col, 4, "east outlet should be at col 4 (rightmost of 5-col composite)");
}

#[test]
fn test_composite_size_stored() {
    let reg = registry_with_composite_custom();
    let def = &reg.custom_components()[0];
    assert_eq!(def.composite_size, Some((5, 3)));
}

// ── cell overrides ────────────────────────────────────────────────────────────

#[test]
fn test_cell_override_set_get() {
    let mut def = composite_custom_def("cell_test");
    def.set_cell(1, 2, 'X');
    assert_eq!(def.get_cell(1, 2), Some('X'));
}

#[test]
fn test_cell_override_clear() {
    let mut def = composite_custom_def("cell_test");
    def.set_cell(1, 2, 'X');
    def.clear_cell(1, 2);
    assert_eq!(def.get_cell(1, 2), None);
}

#[test]
fn test_cell_color_override_set_get() {
    let mut def = composite_custom_def("color_test");
    def.set_cell_color(1, 1, [255, 0, 128]);
    assert_eq!(def.get_cell_color(1, 1), Some([255, 0, 128]));
}

// ── simulate with simple custom ───────────────────────────────────────────────

#[test]
fn test_simulate_with_simple_custom_reaches_sink() {
    let mut reg = GlyphRegistry::new();
    let def = simple_custom_def("valve_ew");
    reg.add_custom_component(def);

    let mut g = Grid::new(10, 10);
    g.set(5, 0, Some(make_comp(ComponentKind::Source)));
    g.set(5, 1, Some(make_comp(ComponentKind::PipeH)));

    // Place a Custom component with the registered id
    let mut custom = make_comp(ComponentKind::Custom);
    custom.custom_id = Some("valve_ew".into());
    custom.custom_connections = Some([false, false, true, true]);
    g.set(5, 2, Some(custom));

    g.set(5, 3, Some(make_comp(ComponentKind::PipeH)));
    g.set(5, 4, Some(make_comp(ComponentKind::Sink)));

    let sim = simulate(&g, FluidType::Water, &reg);
    assert!(sim.reached_sink, "flow should reach sink through simple custom component");
}

// ── custom component no ports uses connections_nsew ───────────────────────────

#[test]
fn test_custom_no_ports_falls_back_to_connections_nsew() {
    let def = simple_custom_def("no_ports");
    assert!(def.ports.is_empty(), "simple custom should have no explicit ports");
    assert_eq!(def.connections_nsew, [false, false, true, true]);
}

// ── empty registry ────────────────────────────────────────────────────────────

#[test]
fn test_empty_registry_no_custom_components() {
    let reg = GlyphRegistry::new();
    assert_eq!(reg.custom_components().len(), 0);
}

// ── multiple custom components ────────────────────────────────────────────────

#[test]
fn test_multiple_custom_components() {
    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(simple_custom_def("a"));
    reg.add_custom_component(simple_custom_def("b"));
    reg.add_custom_component(composite_custom_def("c"));
    assert_eq!(reg.custom_components().len(), 3);
}

#[test]
fn test_custom_components_have_distinct_ids() {
    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(simple_custom_def("x1"));
    reg.add_custom_component(simple_custom_def("x2"));
    let ids: Vec<&str> = reg.custom_components().iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"x1"));
    assert!(ids.contains(&"x2"));
}
