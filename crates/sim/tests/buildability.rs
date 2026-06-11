//! Surface-rail buildability: a Surface line through built-up land accrues disruption; switching
//! the line to Elevated/Tunnel reduces it; a Surface line over water is flagged; deterministic.
use sim::*;

/// Grid (100 m cells): a built-up region ~1.5–2.5 km east, and a water region ~3.5–4.5 km east.
fn city_with_grid() -> CityData {
    let mut bcells = Vec::new();
    for k in 15..=25 {
        bcells.push(BuildCell { x_mm: k * 100_000, y_mm: 0, c: 3 }); // Built
    }
    for k in 35..=45 {
        bcells.push(BuildCell { x_mm: k * 100_000, y_mm: 0, c: 4 }); // Water
    }
    CityData {
        id: "t".into(),
        seed: 1,
        demand: DemandGrid::default(),
        buildability: BuildabilityGrid { cell_m: 100.0, cells: bcells },
        ..Default::default()
    }
}

/// One straight line from (0,0) east to (5km,0), crossing the built region then the water region.
fn line_world() -> World {
    let mut w = World::new(1, city_with_grid());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w
}

#[test]
fn surface_through_built_accrues_disruption_and_modes_reduce_it() {
    let mut w = line_world();
    let surface = w.lines[0].disruption_units;
    assert!(surface > 0, "surface track through built-up land has disruption: {surface}");
    assert!(w.lines[0].crosses_water_surface, "surface line over water is flagged");

    // Whole-line Elevated reduces disruption and clears the water flag.
    w.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 1 });
    let elevated = w.lines[0].disruption_units;
    assert!(elevated < surface, "elevated reduces disruption ({elevated} < {surface})");
    assert!(!w.lines[0].crosses_water_surface, "elevated clears the water-surface flag");

    // Tunnel reduces it further.
    w.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 2 });
    assert!(w.lines[0].disruption_units < elevated, "tunnel reduces disruption further");
}

#[test]
fn open_corridor_has_no_disruption() {
    // A line entirely east of the grid (open land) accrues nothing.
    let mut w = World::new(1, city_with_grid());
    w.apply(&Command::PlaceStation { x_mm: 6_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 8_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    assert_eq!(w.lines[0].disruption_units, 0);
    assert!(!w.lines[0].crosses_water_surface);
}

#[test]
fn surface_over_water_parks_the_line_until_legalized() {
    // Build legality is a CORE rule, not a UI courtesy: a land line with surface track over open
    // water gets NO vehicles and serves NO coverage, even with a trainset assigned. Tunnelling
    // (what the loader and the editor's Track buttons do) un-parks it.
    let mut w = line_world();
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert!(w.lines[0].crosses_water_surface, "precondition: the line is flagged");
    assert_eq!(w.vehicles.len(), 0, "an illegal surface-over-water line does not dispatch");
    assert!(w.serving.iter().all(|s| s.is_empty()), "a parked line serves no station");

    // Legalize the water spans (whole-line tunnel) → the line dispatches on the next tick.
    w.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 2 });
    w.tick(50);
    assert!(!w.lines[0].crosses_water_surface);
    assert_eq!(w.vehicles.len(), 2, "a legalized line dispatches its trainset");

    // Determinism: the park/un-park cycle replays bit-for-bit.
    let mut b = line_world();
    b.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    b.apply(&Command::SetRunning { running: true });
    b.tick(50);
    b.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 2 });
    b.tick(50);
    assert_eq!(w.state_hash(), b.state_hash());
}

#[test]
fn buildability_is_deterministic() {
    let mut a = line_world();
    a.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 1 });
    let mut b = line_world();
    b.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 1 });
    assert_eq!(a.state_hash(), b.state_hash());
}
