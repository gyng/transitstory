//! The catchment no-double-count invariant: a grid cell's weight is SHARED across in-range
//! stations (normalized decay), so the total captured never exceeds the cell weight, and a
//! station beyond the radius captures nothing.
use sim::*;

fn city_one_cell() -> CityData {
    CityData {
        id: "t".into(),
        seed: 1,
        demand: DemandGrid {
            cell_m: 600.0,
            cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 10.0, dest_w: 6.0, commodity: 0 }],
        },
        ..Default::default()
    }
}

#[test]
fn catchment_capture_shares_a_cell_without_double_counting() {
    let mut w = World::new(1, city_one_cell());
    w.apply(&Command::PlaceStation { x_mm: 200_000, y_mm: 0, name: None }); // A: 200 m E
    w.apply(&Command::PlaceStation { x_mm: -200_000, y_mm: 0, name: None }); // B: 200 m W
    w.apply(&Command::PlaceStation { x_mm: 1_000_000, y_mm: 0, name: None }); // C: 1 km E (out of range)
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50); // triggers demand::prepare (catchment capture)

    let o = &w.captured_origin;
    let sum = o[0] + o[1] + o[2];
    assert!(
        (sum - 10.0).abs() < 0.01,
        "captured origin across all stations == cell weight (no double-count); got {sum}",
    );
    assert!(o[2].abs() < 1e-6, "station beyond the catchment radius captures nothing");
    assert!(o[0] > 0.0 && o[1] > 0.0, "in-range stations both capture");
    assert!((o[0] - o[1]).abs() < 0.01, "equidistant stations split the cell equally");
}

#[test]
fn catchment_capture_is_fresh_in_build_mode_without_ticking() {
    // The inspect surfaces (homes/jobs hover, catchment-fill density, desire arcs) read captured
    // demand at PLACEMENT time — in Build mode, before any tick. `apply` must refresh it eagerly.
    let mut w = World::new(1, city_one_cell());
    w.apply(&Command::PlaceStation { x_mm: 200_000, y_mm: 0, name: None }); // 200 m E, in range
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None }); // 5 km E, out of range
    // No CreateLine / AssignTrainset / SetRunning, and crucially no tick().
    assert!(
        w.captured_origin[0] > 0.0 && w.captured_dest[0] > 0.0,
        "in-range station captures homes+jobs in Build mode (no tick); got origin={} dest={}",
        w.captured_origin[0],
        w.captured_dest[0],
    );
    assert!(w.captured_origin[1].abs() < 1e-6, "out-of-range station still captures nothing");

    // Removing a station must re-share its catchment immediately (still no tick).
    let before = w.captured_origin[0];
    w.apply(&Command::PlaceStation { x_mm: 200_000, y_mm: 0, name: None }); // co-located with #0
    assert!(
        w.captured_origin[0] < before,
        "a co-located neighbour halves the share at once in Build mode; {} should be < {before}",
        w.captured_origin[0],
    );
}
