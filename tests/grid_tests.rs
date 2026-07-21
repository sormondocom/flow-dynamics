use flow_dynamics::components::{Component, ComponentKind, PipeDiameter, PipeMaterial};
use flow_dynamics::grid::Grid;

fn make_comp(kind: ComponentKind) -> Component {
    Component::new(kind, PipeDiameter::ThreeQuarter, PipeMaterial::Copper)
}

fn place(grid: &mut Grid, r: usize, c: usize, kind: ComponentKind) {
    let comp = make_comp(kind);
    if comp.effective_is_composite() {
        grid.ensure_size(c + comp.effective_footprint().0 + 1, r + comp.effective_footprint().1 + 1);
        grid.place_composite(r, c, comp);
    } else {
        grid.set(r, c, Some(comp));
    }
}

// ── east-west connectivity ────────────────────────────────────────────────────

#[test]
fn test_connected_east_west() {
    let mut g = Grid::new(20, 10);
    place(&mut g, 5, 5, ComponentKind::PipeH);
    place(&mut g, 5, 6, ComponentKind::PipeH);
    assert!(g.are_connected(5, 5, 5, 6), "PipeH should connect east→west");
    assert!(g.are_connected(5, 6, 5, 5), "connectivity must be symmetric");
}

#[test]
fn test_not_connected_east_west_gap() {
    let mut g = Grid::new(20, 10);
    place(&mut g, 5, 5, ComponentKind::PipeH);
    // leave (5,6) empty
    place(&mut g, 5, 7, ComponentKind::PipeH);
    assert!(!g.are_connected(5, 5, 5, 7));
}

// ── north-south connectivity ──────────────────────────────────────────────────

#[test]
fn test_connected_north_south() {
    let mut g = Grid::new(20, 10);
    place(&mut g, 5, 5, ComponentKind::PipeV);
    place(&mut g, 6, 5, ComponentKind::PipeV);
    assert!(g.are_connected(5, 5, 6, 5), "PipeV should connect north↔south");
    assert!(g.are_connected(6, 5, 5, 5), "connectivity must be symmetric");
}

#[test]
fn test_not_connected_diagonal() {
    let mut g = Grid::new(20, 10);
    place(&mut g, 5, 5, ComponentKind::Cross);
    place(&mut g, 6, 6, ComponentKind::Cross);
    assert!(!g.are_connected(5, 5, 6, 6));
}

// ── satellite registration ────────────────────────────────────────────────────

#[test]
fn test_satellite_registration_water_softener() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    // WaterSoftener: 17×5, anchor at (2,0), port_row = 2
    // A cell in the composite (not the anchor) should be a satellite
    let (fw, _fh) = ComponentKind::WaterSoftener.footprint();
    // last column of composite is anchor_col + fw - 1
    let last_col = 0 + fw - 1;
    let sat = g.satellite_anchor(2, last_col);
    assert!(sat.is_some(), "last column of composite should be a satellite");
    assert_eq!(sat.unwrap(), (2, 0), "satellite should point to anchor");
}

#[test]
fn test_anchor_not_in_satellite_map() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    assert!(g.satellite_anchor(2, 0).is_none(), "anchor should not appear in satellite map");
}

// ── rebuild_satellites() ──────────────────────────────────────────────────────

#[test]
fn test_rebuild_satellites() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    // Manually clear the satellite map
    g.satellites.clear();
    assert!(g.satellite_anchor(2, 8).is_none(), "cleared satellite should be gone");
    g.rebuild_satellites();
    assert!(g.satellite_anchor(2, 8).is_some(), "satellite should return after rebuild");
}

// ── effective_pos() ───────────────────────────────────────────────────────────

#[test]
fn test_effective_pos_anchor_returns_self() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    assert_eq!(g.effective_pos(2, 0), (2, 0));
}

#[test]
fn test_effective_pos_satellite_returns_anchor() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    // Column 8 is a satellite (port_row = 2, middle of composite)
    assert_eq!(g.effective_pos(2, 8), (2, 0));
}

// ── clear_at() ────────────────────────────────────────────────────────────────

#[test]
fn test_clear_at_removes_composite_and_satellites() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    g.clear_at(2, 0);
    assert!(g.get(2, 0).is_none(), "anchor should be cleared");
    assert!(g.satellite_anchor(2, 8).is_none(), "satellites should be removed");
}

#[test]
fn test_clear_at_via_satellite_clears_anchor() {
    let mut g = Grid::new(30, 10);
    place(&mut g, 2, 0, ComponentKind::WaterSoftener);
    g.clear_at(2, 8); // clear via satellite
    assert!(g.get(2, 0).is_none(), "anchor should be cleared when satellite cleared");
}

// ── ensure_size() ─────────────────────────────────────────────────────────────

#[test]
fn test_ensure_size_grows() {
    let mut g = Grid::new(5, 5);
    assert_eq!(g.width, 5);
    assert_eq!(g.height, 5);
    g.ensure_size(10, 15);
    assert_eq!(g.width, 10);
    assert_eq!(g.height, 15);
}

#[test]
fn test_ensure_size_no_shrink() {
    let mut g = Grid::new(10, 10);
    g.ensure_size(3, 3);
    assert_eq!(g.width, 10);
    assert_eq!(g.height, 10);
}

// ── single-cell component basics ──────────────────────────────────────────────

#[test]
fn test_set_and_get() {
    let mut g = Grid::new(5, 5);
    let comp = make_comp(ComponentKind::Source);
    g.set(2, 3, Some(comp.clone()));
    let got = g.get(2, 3).unwrap();
    assert_eq!(got.kind, ComponentKind::Source);
}

#[test]
fn test_get_out_of_bounds_returns_none() {
    let g = Grid::new(5, 5);
    assert!(g.get(100, 100).is_none());
}
