/// Integration tests for custom composite glyphs — focusing on the "shared library"
/// workflow: User A builds a composite, saves it, User B loads and simulates with it.
use flow_dynamics::components::{Component, ComponentKind, PipeDiameter, PipeMaterial};
use flow_dynamics::fluid::FluidType;
use flow_dynamics::glyphs::{
    CustomCompDef, CustomPort, GlyphDef, GlyphLibrary, GlyphRegistry, PortFace, PortKind,
    port_face_for,
};
use flow_dynamics::grid::Grid;
use flow_dynamics::simulation::simulate;

// ── helpers ───────────────────────────────────────────────────────────────────

fn pipe() -> Component {
    Component::new(ComponentKind::PipeH, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

fn source() -> Component {
    Component::new(ComponentKind::Source, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

fn sink() -> Component {
    Component::new(ComponentKind::Sink, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

fn default_glyph() -> GlyphDef {
    GlyphDef { symbol: '◇', fg: [200, 200, 200] }
}

/// 5-wide × 3-tall composite with a West inlet (row=1,col=0) and East outlet (row=1,col=4).
/// This is the canonical "pass-through" fixture used across many tests.
fn passthrough_5x3(id: &str) -> CustomCompDef {
    let mut def = CustomCompDef::new(id.into(), id.into(), default_glyph());
    def.composite_size = Some((5, 3));
    def.ports = vec![
        CustomPort { name: "in_w".into(),  kind: PortKind::Inlet,  row: 1, col: 0 },
        CustomPort { name: "out_e".into(), kind: PortKind::Outlet, row: 1, col: 4 },
    ];
    def
}

/// Build a registry containing the given def, then place the composite on a grid
/// with a source chain on the west and a sink chain on the east, then simulate.
///
/// The composite anchor is at (anchor_r, anchor_c).
/// A source and one pipe are placed west of the external west port cell.
/// A pipe and sink are placed east of the external east port cell.
fn simulate_passthrough(def: CustomCompDef, anchor_r: usize, anchor_c: usize) -> bool {
    let id = def.id.clone();
    let (canvas_w, canvas_h) = def.composite_size.expect("passthrough must have composite_size");
    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(def);

    // Resolve external port cells from port_external_offsets.
    let defs = reg.custom_components();
    let offsets = defs[0].port_external_offsets();
    assert_eq!(offsets.len(), 2, "passthrough should have 2 port offsets");

    let mut g = Grid::new(30, 20);

    // Place the composite at the anchor — must set custom_footprint so that
    // effective_is_composite() returns true for grid placement.
    let mut comp = Component::new(ComponentKind::Custom, PipeDiameter::ThreeQuarter, PipeMaterial::Copper);
    comp.custom_id = Some(id.clone());
    comp.custom_footprint = Some((canvas_w, canvas_h));
    comp.custom_connections = Some([false, false, true, true]);
    g.place_composite(anchor_r, anchor_c, comp);

    // Find west and east external cells from the offsets.
    for (row_off, col_off, face) in &offsets {
        let er = (anchor_r as isize + row_off) as usize;
        let ec = (anchor_c as isize + col_off) as usize;
        match face {
            PortFace::West => {
                // source at (er, ec-1), pipe at (er, ec) [the external cell].
                g.set(er, ec,     Some(pipe()));
                g.set(er, ec - 1, Some(source()));
            }
            PortFace::East => {
                // pipe at (er, ec) [external], sink at (er, ec+1).
                g.set(er, ec,     Some(pipe()));
                g.set(er, ec + 1, Some(sink()));
            }
            _ => {}
        }
    }

    simulate(&g, FluidType::Water, &reg).reached_sink
}

// ── 1. JSON serialization round-trip ──────────────────────────────────────────

#[test]
fn test_composite_json_roundtrip_all_fields() {
    let mut def = passthrough_5x3("rt_full");
    def.set_cell(0, 0, '╔');
    def.set_cell(0, 4, '╗');
    def.set_cell(2, 0, '╚');
    def.set_cell(2, 4, '╝');
    def.set_cell_color(1, 2, [255, 128, 0]);

    let json = serde_json::to_string_pretty(&def).expect("serialize");
    let restored: CustomCompDef = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.id, "rt_full");
    assert_eq!(restored.composite_size, Some((5, 3)));
    assert_eq!(restored.ports.len(), 2);
    assert_eq!(restored.get_cell(0, 0), Some('╔'));
    assert_eq!(restored.get_cell(2, 4), Some('╝'));
    assert_eq!(restored.get_cell_color(1, 2), Some([255, 128, 0]));
}

#[test]
fn test_composite_json_roundtrip_empty_overrides() {
    let def = passthrough_5x3("rt_bare");
    let json = serde_json::to_string_pretty(&def).expect("serialize");
    let restored: CustomCompDef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.get_cell(1, 0), None);
    assert_eq!(restored.get_cell_color(1, 0), None);
}

#[test]
fn test_composite_port_kinds_survive_roundtrip() {
    let def = passthrough_5x3("rt_ports");
    let json = serde_json::to_string(&def).expect("serialize");
    let restored: CustomCompDef = serde_json::from_str(&json).expect("deserialize");

    let inlet  = restored.ports.iter().find(|p| p.name == "in_w").expect("inlet");
    let outlet = restored.ports.iter().find(|p| p.name == "out_e").expect("outlet");
    assert_eq!(inlet.kind,  PortKind::Inlet);
    assert_eq!(outlet.kind, PortKind::Outlet);
    assert_eq!(inlet.col, 0);
    assert_eq!(outlet.col, 4);
}

// ── 2. GlyphLibrary file round-trip ──────────────────────────────────────────

#[test]
fn test_library_file_roundtrip_single_composite() {
    let tmp = std::env::temp_dir().join("fd_test_single.json");
    let mut lib = GlyphLibrary::default();
    lib.version = "2.0".into();
    lib.custom_components.push(passthrough_5x3("saved_comp"));
    lib.save(&tmp).expect("save");

    let loaded = GlyphLibrary::load(&tmp).expect("load");
    assert_eq!(loaded.custom_components.len(), 1);
    assert_eq!(loaded.custom_components[0].id, "saved_comp");
    assert_eq!(loaded.custom_components[0].composite_size, Some((5, 3)));
    assert_eq!(loaded.custom_components[0].ports.len(), 2);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_library_file_roundtrip_multiple_composites() {
    let tmp = std::env::temp_dir().join("fd_test_multi.json");
    let mut lib = GlyphLibrary::default();
    lib.version = "2.0".into();
    lib.custom_components.push(passthrough_5x3("alpha"));
    lib.custom_components.push(passthrough_5x3("beta"));

    let mut drain_comp = CustomCompDef::new("gamma".into(), "Gamma".into(), default_glyph());
    drain_comp.composite_size = Some((7, 5));
    drain_comp.ports = vec![
        CustomPort { name: "in_n".into(),  kind: PortKind::Inlet, row: 0, col: 3 },
        CustomPort { name: "out_s".into(), kind: PortKind::Drain, row: 4, col: 3 },
    ];
    lib.custom_components.push(drain_comp);

    lib.save(&tmp).expect("save");
    let loaded = GlyphLibrary::load(&tmp).expect("load");

    assert_eq!(loaded.custom_components.len(), 3);
    let ids: Vec<&str> = loaded.custom_components.iter().map(|d| d.id.as_str()).collect();
    assert!(ids.contains(&"alpha"));
    assert!(ids.contains(&"beta"));
    assert!(ids.contains(&"gamma"));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_library_file_roundtrip_cell_overrides_preserved() {
    let tmp = std::env::temp_dir().join("fd_test_cells.json");
    let mut def = passthrough_5x3("cells_comp");
    def.set_cell(0, 0, '█');
    def.set_cell(1, 2, '·');
    def.set_cell_color(0, 0, [255, 0, 0]);

    let mut lib = GlyphLibrary::default();
    lib.version = "2.0".into();
    lib.custom_components.push(def);
    lib.save(&tmp).expect("save");

    let loaded = GlyphLibrary::load(&tmp).expect("load");
    let comp = &loaded.custom_components[0];
    assert_eq!(comp.get_cell(0, 0), Some('█'));
    assert_eq!(comp.get_cell(1, 2), Some('·'));
    assert_eq!(comp.get_cell_color(0, 0), Some([255, 0, 0]));
    let _ = std::fs::remove_file(&tmp);
}

// ── 3. V1 → V2 migration ─────────────────────────────────────────────────────

#[test]
fn test_v1_library_triggers_migration_on_load() {
    let tmp = std::env::temp_dir().join("fd_test_v1.json");
    // Craft a v1.0 composite: ports at old extended-footprint coords (dc=1 for west, dc=fw-2 for east),
    // composite_size reflects inner dims (3w × 1h), actual footprint was inner+2=5×3.
    let json = r#"{
        "name": "Test Library",
        "version": "1.0",
        "overrides": {},
        "custom_components": [{
            "id": "v1_comp",
            "label": "V1 Comp",
            "glyph": { "symbol": "◇", "fg": [200, 200, 200] },
            "composite_size": [3, 1],
            "ports": [
                { "name": "in_w",  "kind": "Inlet",  "row": 1, "col": 1 },
                { "name": "out_e", "kind": "Outlet", "row": 1, "col": 3 }
            ],
            "cell_overrides": {
                "1,1": "═",
                "1,2": "═",
                "1,3": "═"
            }
        }]
    }"#;
    std::fs::write(&tmp, json).unwrap();
    let loaded = GlyphLibrary::load(&tmp).expect("load v1");

    assert_eq!(loaded.version, "2.0", "version should be bumped to 2.0 after migration");
    let comp = &loaded.custom_components[0];
    // After migration: port col shifts by -1 (col=1 → col=0, col=3 → col=2)
    let inlet  = comp.ports.iter().find(|p| p.name == "in_w").expect("inlet");
    let outlet = comp.ports.iter().find(|p| p.name == "out_e").expect("outlet");
    assert_eq!(inlet.col, 0, "v1 west port col=1 should migrate to col=0");
    assert_eq!(outlet.col, 2, "v1 east port col=3 should migrate to col=2");
    // Border cells (row=0, row=fh-1, col=0, col=fw-1 in old coords) should be stripped.
    // Inner cells at old (1,1),(1,2),(1,3) → shift to (0,0),(0,1),(0,2) in new coords.
    // But old (1,1): r=1, c=1 in 5×3 footprint (fw=5, fh=3): r!=0, r+1!=3, c!=0, c+1!=5 → kept → new (0,0)
    assert!(comp.get_cell(0, 0).is_some(), "migrated interior cell should be kept");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_v2_library_does_not_remigrate() {
    // Ports of a v2 composite should not be double-shifted.
    let tmp = std::env::temp_dir().join("fd_test_v2_noremigrate.json");
    let mut lib = GlyphLibrary::default();
    lib.version = "2.0".into();
    lib.custom_components.push(passthrough_5x3("stable"));
    lib.save(&tmp).expect("save");

    let loaded = GlyphLibrary::load(&tmp).expect("load");
    let comp = &loaded.custom_components[0];
    let inlet = comp.ports.iter().find(|p| p.name == "in_w").expect("inlet");
    assert_eq!(inlet.col, 0, "v2 west port col should remain 0 after load");
    let _ = std::fs::remove_file(&tmp);
}

// ── 4. Robustness: minimal and extra-field JSON ───────────────────────────────

#[test]
fn test_deserialize_minimal_composite_json() {
    let json = r#"{
        "id": "minimal",
        "label": "Minimal",
        "glyph": { "symbol": "○", "fg": [100, 100, 100] }
    }"#;
    let def: CustomCompDef = serde_json::from_str(json).expect("deserialize minimal");
    assert_eq!(def.id, "minimal");
    assert!(def.composite_size.is_none(), "should default to single-cell");
    assert!(def.ports.is_empty(), "should have no ports");
    assert_eq!(def.get_cell(0, 0), None, "no cell overrides");
}

#[test]
fn test_deserialize_json_with_unknown_fields_is_lenient() {
    // serde(deny_unknown_fields) is NOT set on CustomCompDef, so extra fields are ignored.
    let json = r#"{
        "id": "future_comp",
        "label": "Future Comp",
        "glyph": { "symbol": "◈", "fg": [80, 200, 80] },
        "future_field": "some_value_added_in_v3",
        "another_new_array": [1, 2, 3]
    }"#;
    let def: CustomCompDef = serde_json::from_str(json).expect("should tolerate unknown fields");
    assert_eq!(def.id, "future_comp");
}

#[test]
fn test_deserialize_invalid_json_returns_error() {
    let bad = r#"{ "id": "broken", "label": BAD_JSON }"#;
    let result: Result<CustomCompDef, _> = serde_json::from_str(bad);
    assert!(result.is_err(), "invalid JSON should return an error, not panic");
}

#[test]
fn test_deserialize_missing_required_field_returns_error() {
    // `id`, `label`, and `glyph` are not marked serde(default), so they're required.
    let json = r#"{ "label": "No ID", "glyph": { "symbol": "?", "fg": [0,0,0] } }"#;
    let result: Result<CustomCompDef, _> = serde_json::from_str(json);
    assert!(result.is_err(), "missing id should fail deserialization");
}

// ── 5. port_face_for — direction correctness ──────────────────────────────────

#[test]
fn test_port_face_for_west_border() {
    // On the west edge (col=0), non-corner rows are unambiguously West.
    assert_eq!(port_face_for(1, 0, 5, 3), PortFace::West);
    assert_eq!(port_face_for(2, 0, 7, 5), PortFace::West);
    // Corner (0,0) ties North and West — North wins by tie-break order (tested separately).
}

#[test]
fn test_port_face_for_east_border() {
    // On the east edge (col=canvas_w-1).
    assert_eq!(port_face_for(1, 4, 5, 3), PortFace::East);
    assert_eq!(port_face_for(2, 6, 7, 5), PortFace::East);
}

#[test]
fn test_port_face_for_north_border() {
    assert_eq!(port_face_for(0, 2, 5, 3), PortFace::North);
    assert_eq!(port_face_for(0, 3, 7, 7), PortFace::North);
}

#[test]
fn test_port_face_for_south_border() {
    assert_eq!(port_face_for(2, 2, 5, 3), PortFace::South);
    assert_eq!(port_face_for(6, 3, 7, 7), PortFace::South);
}

#[test]
fn test_port_face_for_interior_nearest_west() {
    // (2, 1) in 10×5 canvas: dist_west=1, dist_east=8, dist_north=2, dist_south=2 → West
    assert_eq!(port_face_for(2, 1, 10, 5), PortFace::West);
}

#[test]
fn test_port_face_for_interior_nearest_north() {
    // (1, 5) in 11×7: dist_north=1, dist_south=5, dist_west=5, dist_east=5 → North
    assert_eq!(port_face_for(1, 5, 11, 7), PortFace::North);
}

// ── 6. port_external_offsets — arithmetic correctness ────────────────────────

#[test]
fn test_external_offset_west_port_standard() {
    // 5×3 composite, West inlet at (1,0). pr = 3/2 = 1 (anchor row).
    // West external offset: (dr - pr, -1) = (1-1, -1) = (0, -1).
    let def = passthrough_5x3("off_w");
    let offsets = def.port_external_offsets();
    let west = offsets.iter().find(|(_, _, f)| *f == PortFace::West).expect("west offset");
    assert_eq!((west.0, west.1), (0, -1));
}

#[test]
fn test_external_offset_east_port_standard() {
    // 5×3 composite, East outlet at (1,4). pr=1.
    // East external offset: (dr - pr, fw) = (1-1, 5) = (0, 5).
    let def = passthrough_5x3("off_e");
    let offsets = def.port_external_offsets();
    let east = offsets.iter().find(|(_, _, f)| *f == PortFace::East).expect("east offset");
    assert_eq!((east.0, east.1), (0, 5));
}

#[test]
fn test_external_offset_north_port() {
    // 5×5 composite with North port at (0, 2). pr = 5/2 = 2.
    // North external offset: (-(pr+1), dc) = (-3, 2).
    let mut def = CustomCompDef::new("off_n".into(), "off_n".into(), default_glyph());
    def.composite_size = Some((5, 5));
    def.ports = vec![
        CustomPort { name: "in_n".into(), kind: PortKind::Inlet, row: 0, col: 2 },
    ];
    let offsets = def.port_external_offsets();
    assert_eq!(offsets.len(), 1);
    assert_eq!((offsets[0].0, offsets[0].1), (-3, 2));
    assert_eq!(offsets[0].2, PortFace::North);
}

#[test]
fn test_external_offset_south_port() {
    // 5×5 composite, South port at (4,2). pr=2.
    // South offset: (fh - pr, dc) = (5 - 2, 2) = (3, 2).
    let mut def = CustomCompDef::new("off_s".into(), "off_s".into(), default_glyph());
    def.composite_size = Some((5, 5));
    def.ports = vec![
        CustomPort { name: "out_s".into(), kind: PortKind::Drain, row: 4, col: 2 },
    ];
    let offsets = def.port_external_offsets();
    assert_eq!((offsets[0].0, offsets[0].1), (3, 2));
    assert_eq!(offsets[0].2, PortFace::South);
}

// ── 7. Simulation — shared composite flows end-to-end ────────────────────────

#[test]
fn test_shared_composite_source_to_sink_basic() {
    // The most important integration test: a library received from another user
    // contains a passthrough composite; place and simulate it.
    assert!(simulate_passthrough(passthrough_5x3("shared_basic"), 5, 5));
}

#[test]
fn test_shared_composite_placed_at_grid_origin_vicinity() {
    // Composite near the grid top-left (anchor needs room for port row above it).
    // 5×3 with pr=1 → top of footprint is anchor_r - 1 → anchor_r must be >= 1.
    assert!(simulate_passthrough(passthrough_5x3("shared_origin"), 2, 2));
}

#[test]
fn test_shared_composite_json_loaded_then_simulated() {
    // Full round-trip: serialize → deserialize → add to registry → simulate.
    let original = passthrough_5x3("rt_sim");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: CustomCompDef = serde_json::from_str(&json).expect("deserialize");

    assert!(simulate_passthrough(restored, 5, 5));
}

#[test]
fn test_library_file_loaded_then_simulated() {
    // Save to disk → load → simulate. Mirrors the exact user workflow.
    let tmp = std::env::temp_dir().join("fd_test_simulate.json");
    let mut lib = GlyphLibrary::default();
    lib.version = "2.0".into();
    lib.custom_components.push(passthrough_5x3("disk_comp"));
    lib.save(&tmp).expect("save");

    let loaded = GlyphLibrary::load(&tmp).expect("load");
    let def = loaded.custom_components.into_iter().next().expect("one component");
    assert!(simulate_passthrough(def, 5, 5));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_composite_with_no_ports_uses_connections_nsew() {
    // A single-cell custom component (no composite_size, no ports) from another user
    // should still connect via connections_nsew when simulated.
    let mut def = CustomCompDef::new("simple_ew".into(), "Simple EW".into(), default_glyph());
    def.connections_nsew = [false, false, true, true]; // E/W

    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(def);

    let mut g = Grid::new(10, 10);
    g.set(5, 0, Some(source()));
    g.set(5, 1, Some(pipe()));

    let mut custom = Component::new(ComponentKind::Custom, PipeDiameter::ThreeQuarter, PipeMaterial::Copper);
    custom.custom_id = Some("simple_ew".into());
    custom.custom_connections = Some([false, false, true, true]);
    g.set(5, 2, Some(custom));

    g.set(5, 3, Some(pipe()));
    g.set(5, 4, Some(sink()));

    let sim = simulate(&g, FluidType::Water, &reg);
    assert!(sim.reached_sink, "single-cell custom via connections_nsew should reach sink");
}

// ── 8. Multi-port composite ───────────────────────────────────────────────────

#[test]
fn test_three_port_composite_reaches_sink() {
    // 7×5 mixing valve: West inlet (2,0), North inlet (0,3), East outlet (2,6).
    // Only the W→E path is connected in this test; N port is present but unconnected.
    let mut def = CustomCompDef::new("mixer".into(), "Mixer".into(), default_glyph());
    def.composite_size = Some((7, 5));
    def.ports = vec![
        CustomPort { name: "in_w".into(),  kind: PortKind::Inlet,  row: 2, col: 0 },
        CustomPort { name: "in_n".into(),  kind: PortKind::Inlet,  row: 0, col: 3 },
        CustomPort { name: "out_e".into(), kind: PortKind::Outlet, row: 2, col: 6 },
    ];

    let id = def.id.clone();
    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(def);

    // Anchor at (5, 5): pr = 5/2 = 2, footprint rows 3..7, cols 5..11
    let anchor_r = 5usize;
    let anchor_c = 5usize;
    let mut g = Grid::new(25, 15);

    let mut comp = Component::new(ComponentKind::Custom, PipeDiameter::ThreeQuarter, PipeMaterial::Copper);
    comp.custom_id = Some(id);
    comp.custom_footprint = Some((7, 5));
    comp.custom_connections = Some([false, false, true, true]);
    g.place_composite(anchor_r, anchor_c, comp);

    // West port (2,0): external at (2-2, -1) = (0, -1) from anchor → (5+0, 5-1) = (5, 4)
    g.set(5, 4, Some(pipe()));
    g.set(5, 3, Some(source()));
    // East port (2,6): external at (2-2, 7) = (0, 7) from anchor → (5, 12)
    g.set(5, 12, Some(pipe()));
    g.set(5, 13, Some(sink()));

    let sim = simulate(&g, FluidType::Water, &reg);
    assert!(sim.reached_sink, "3-port mixer should flow W→E and reach sink");
}

// ── 9. Duplicate ID handling from untrusted library ──────────────────────────

#[test]
fn test_loading_library_with_duplicate_ids_keeps_last() {
    // Two components with the same ID in a library — second replaces first.
    // connections_nsew defaults to [false,false,false,false] when omitted — that's fine here.
    let json = r#"{
        "name": "Dup Test",
        "version": "2.0",
        "overrides": {},
        "custom_components": [
            { "id": "dup", "label": "First",  "glyph": { "symbol": "A", "fg": [255, 0, 0] } },
            { "id": "dup", "label": "Second", "glyph": { "symbol": "B", "fg": [0, 255, 0] } }
        ]
    }"#;
    let lib: GlyphLibrary = serde_json::from_str(json).expect("parse");
    let mut reg = GlyphRegistry::new();
    for comp in lib.custom_components {
        reg.add_custom_component(comp);
    }
    assert_eq!(reg.custom_components().len(), 1, "duplicate ID should be deduplicated");
    assert_eq!(reg.custom_components()[0].label, "Second", "last definition wins");
}

// ── 10. Grid satellite rebuild after deserialization ─────────────────────────

#[test]
fn test_composite_grid_satellites_survive_json_roundtrip() {
    // Serialize a grid containing a composite, deserialize it, rebuild satellites,
    // and verify the anchor/satellite map is restored correctly.
    let mut def = passthrough_5x3("grid_rt");
    def.set_cell(0, 0, '╔');

    let mut reg = GlyphRegistry::new();
    reg.add_custom_component(def);

    let anchor_r = 4usize;
    let anchor_c = 3usize;
    let mut g = Grid::new(20, 12);
    let mut comp = Component::new(ComponentKind::Custom, PipeDiameter::ThreeQuarter, PipeMaterial::Copper);
    comp.custom_id = Some("grid_rt".into());
    comp.custom_footprint = Some((5, 3));
    comp.custom_connections = Some([false, false, true, true]);
    g.place_composite(anchor_r, anchor_c, comp);

    // Verify a satellite cell is registered before serialization.
    assert!(
        g.satellite_anchor(anchor_r - 1, anchor_c).is_some(),
        "row above anchor should be a satellite before serialize"
    );

    // Serialize → deserialize → rebuild.
    let json = serde_json::to_string(&g).expect("grid serialize");
    let mut g2: Grid = serde_json::from_str(&json).expect("grid deserialize");
    g2.rebuild_satellites();

    assert!(
        g2.satellite_anchor(anchor_r - 1, anchor_c).is_some(),
        "satellite should be restored after rebuild_satellites"
    );
    // Anchor cell should contain the component.
    assert!(g2.get(anchor_r, anchor_c).is_some(), "anchor cell should have a component");
}
