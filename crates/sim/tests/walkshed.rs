//! Network-walkshed catchment: capture respects pedestrian barriers in the buildability raster.
//! WATER is an impassable edge (a station can't draw riders across a coast/river); major ROAD and
//! RAIL corridors are crossable but costly, so a sightline that crosses one is penalised and a
//! marginal cell past the budget drops out. With NO buildability grid the capture is bit-identical
//! to the old pure-Euclidean shed (regression guard), and the whole thing still replays exactly.
use sim::*;

/// A city whose single demand cell sits `cell_east_mm` east of the origin, with an optional
/// vertical barrier strip of class `barrier` at column `bar_x_mm` (None ⇒ no buildability grid).
fn city_with_barrier(cell_east_mm: i64, bar_x_mm: Option<i64>, barrier: u8) -> CityData {
    let mut buildability = BuildabilityGrid { cell_m: 100.0, cells: Vec::new() };
    if let Some(bx) = bar_x_mm {
        // A tall 1-column strip so every eastward sightline crosses it.
        for gy in -4..=4 {
            buildability.cells.push(BuildCell { x_mm: bx, y_mm: gy * 100_000, c: barrier });
        }
    }
    CityData {
        id: "t".into(),
        seed: 1,
        demand: DemandGrid {
            cell_m: 100.0,
            cells: vec![DemandCell { x_mm: cell_east_mm, y_mm: 0, origin_w: 10.0, dest_w: 6.0 }],
        },
        buildability: if bar_x_mm.is_some() { buildability } else { BuildabilityGrid::default() },
        ..Default::default()
    }
}

/// Capture for a single station at the origin against a demand cell `cell_east_mm` east, with an
/// optional barrier strip. Catchment capture (`demand::prepare`) runs inside `tick()` while
/// running, so one short tick populates `captured_origin` (no line needed — capture is geographic).
fn captured_origin_single(cell_east_mm: i64, bar_x_mm: Option<i64>, barrier: u8) -> f32 {
    let mut w = World::new(1, city_with_barrier(cell_east_mm, bar_x_mm, barrier));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    w.captured_origin[0]
}

#[test]
fn water_severs_the_far_bank() {
    // Same geometry, with vs without a water strip between the station and the demand cell.
    let dry = captured_origin_single(300_000, None, 0);
    let wet = captured_origin_single(300_000, Some(150_000), sim::city::class::WATER);
    assert!((dry - 10.0).abs() < 0.01, "with no barrier a lone in-range station captures the whole cell; got {dry}");
    assert!(wet < 0.01, "a station captures ~nothing across a water barrier; got {wet}");
}

#[test]
fn a_road_crossing_drops_a_marginal_cell() {
    // A cell near the budget edge (480 m of a 500 m shed). Clear path → captured; a road corridor
    // across the sightline inflates the effective distance past the budget → the cell drops out.
    let clear = captured_origin_single(480_000, None, 0);
    let crossed = captured_origin_single(480_000, Some(240_000), sim::city::class::ROAD);
    assert!((clear - 10.0).abs() < 0.01, "a marginal cell with a clear path is captured; got {clear}");
    assert!(crossed < 0.01, "the same cell drops out once a motorway must be crossed; got {crossed}");
}

#[test]
fn no_buildability_grid_is_pure_euclidean() {
    // The old invariant (mirrors catchment.rs): with no raster, two equidistant stations split a
    // cell equally and the total never exceeds the cell weight — proving the no-barrier path is
    // unchanged bit-for-bit.
    let mut w = World::new(1, city_with_barrier(0, None, 0));
    w.apply(&Command::PlaceStation { x_mm: 200_000, y_mm: 0, name: None }); // 200 m E
    w.apply(&Command::PlaceStation { x_mm: -200_000, y_mm: 0, name: None }); // 200 m W
    w.apply(&Command::PlaceStation { x_mm: 1_000_000, y_mm: 0, name: None }); // 1 km E (out of range)
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    let o = &w.captured_origin;
    assert!(((o[0] + o[1] + o[2]) - 10.0).abs() < 0.01, "no double-count: total == cell weight");
    assert!((o[0] - o[1]).abs() < 0.01, "equidistant, unobstructed stations split equally");
    assert!(o[2].abs() < 1e-6, "the out-of-range station still captures nothing");
}

#[test]
fn walkshed_query_is_lopsided_across_water() {
    // The visual shed query for a station hard against a water column: cells on the dry (west) side
    // are reachable; cells across the water (east) are severed and omitted — a lopsided shed.
    let mut w = World::new(1, city_with_barrier(0, Some(150_000), sim::city::class::WATER));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    let shed = w.station_walkshed(0);
    assert!(!shed.is_empty(), "a station on land has a non-empty walkshed");
    assert!(
        shed.iter().any(|c| c.x_mm < -100_000.0),
        "the dry (west) side is in the shed",
    );
    assert!(
        shed.iter().all(|c| c.x_mm < 220_000.0),
        "nothing across the water column (east) is in the shed — the shed is lopsided",
    );
}

#[test]
fn barrier_capture_replays_deterministically() {
    // A small network over a water city, ticked, must hash identically across two fresh runs —
    // the barrier sampling introduces no nondeterminism.
    let run = || {
        let mut w = World::new(7, city_with_barrier(300_000, Some(150_000), sim::city::class::WATER));
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: -300_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..200 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "barrier-aware capture replays bit-for-bit");
}
