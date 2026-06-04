//! Transfers: a trip whose origin and destination are on different lines routes via the
//! shared interchange (2 legs), and ridership develops on a two-line network with transfers.
use sim::*;

/// Two lines crossing at an interchange (station 2):
///   Line 0 (E–W): stations 0,1,2,3,4   Line 1 (N–S): stations 5,6,2,7,8
fn two_line_world() -> World {
    let cells = (0..30)
        .map(|k| DemandCell { x_mm: 200_000 * (k - 15), y_mm: 0, origin_w: 3.0, dest_w: 3.0 })
        .collect();
    let mut w = World::new(7, CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells }, ..Default::default() });
    // E–W line stations 0..4 along y=0
    for k in 0..5 {
        w.apply(&Command::PlaceStation { x_mm: (k as i64 - 2) * 1_500_000, y_mm: 0, name: None });
    }
    // N–S line stations 5,6 (north), 7,8 (south); interchange reuses station 2 (origin).
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 3_000_000, name: None }); // 5
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 1_500_000, name: None }); // 6
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: -1_500_000, name: None }); // 7
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: -3_000_000, name: None }); // 8

    w.apply(&Command::CreateLine { color: 0x00aa00, name: None, loop_line: false, mode: 0 });
    for s in [0, 1, 2, 3, 4] {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::CreateLine { color: 0xaa0000, name: None, loop_line: false, mode: 0 });
    for s in [5, 6, 2, 7, 8] {
        w.apply(&Command::AddStop { line: LineId(1), station: StationId(s), after: None });
    }
    for l in 0..2 {
        w.apply(&Command::AssignTrainset { line: LineId(l), spec: 0, count: 3 });
        w.apply(&Command::SetHeadway { line: LineId(l), headway_ms: 180_000 });
    }
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn route_across_two_lines_uses_a_transfer() {
    let mut w = two_line_world();
    w.tick(50); // builds the serving map + vehicles
    // station 0 (E–W only) -> station 8 (N–S only): must transfer at interchange station 2.
    let legs = plan_route(&w.lines, &w.serving, StationId(0), StationId(8), 4).expect("route exists");
    assert_eq!(legs.len(), 2, "origin/dest on different lines => 2 legs (one transfer)");
    assert_eq!(legs[0].line, LineId(0));
    assert_eq!(legs[1].line, LineId(1));
    assert_eq!(legs[0].alight, StationId(2), "transfer at the interchange");
    assert_eq!(legs[1].board, StationId(2));
}

#[test]
fn same_line_route_is_direct() {
    let mut w = two_line_world();
    w.tick(50);
    let legs = plan_route(&w.lines, &w.serving, StationId(0), StationId(4), 4).expect("route");
    assert_eq!(legs.len(), 1, "same-line trip is a single direct leg");
}

#[test]
fn ridership_develops_with_transfers_and_is_deterministic() {
    let mut a = two_line_world();
    for _ in 0..6000 {
        a.tick(50);
    }
    assert!(a.stats_snapshot().ridership_total > 0.0, "riders move across the two-line network");

    let mut b = two_line_world();
    for _ in 0..6000 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "transfers stay deterministic");
}
