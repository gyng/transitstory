//! Inter-station footpaths: two lines that share NO station but have stops within walking
//! distance form an interchange by foot. A trip whose only path rides line A, walks the gap, then
//! rides line B must complete — and it must stay deterministic. RED until footpath routing lands.
use sim::*;

/// Two unconnected lines on the x-axis with a walkable gap between their inner stops:
///   Line 0 (west): station 0 (x=-2.0 m·k) — station 1 (x=0)
///   Line 1 (east): station 2 (x=+0.3 m·k) — station 3 (x=+2.0 m·k)
/// Stations 1 and 2 are 300 mm·k = 300 m apart (walkable); 0 and 3 only connect via that footpath.
/// Demand sits ONLY at the far ends (0 and 3), so the sole trips are 0↔3 — unroutable without a walk.
fn footpath_world() -> World {
    let cells = vec![
        DemandCell { x_mm: -2_000_000, y_mm: 0, origin_w: 8.0, dest_w: 8.0 },
        DemandCell { x_mm: 2_000_000, y_mm: 0, origin_w: 8.0, dest_w: 8.0 },
    ];
    let mut w = World::new(
        7,
        CityData { id: "fp".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells }, patience_ms: 0, ..Default::default() },
    );
    w.apply(&Command::PlaceStation { x_mm: -2_000_000, y_mm: 0, name: None }); // 0  (line 0 west end)
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // 1  (line 0 east end)
    w.apply(&Command::PlaceStation { x_mm: 300_000, y_mm: 0, name: None }); // 2  (line 1 west end, 300 m from 1)
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None }); // 3  (line 1 east end)

    w.apply(&Command::CreateLine { color: 0x00aa00, name: None, loop_line: false, mode: 0, literal: false });
    for s in [0, 1] {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::CreateLine { color: 0xaa0000, name: None, loop_line: false, mode: 0, literal: false });
    for s in [2, 3] {
        w.apply(&Command::AddStop { line: LineId(1), station: StationId(s), after: None });
    }
    for l in 0..2 {
        w.apply(&Command::AssignTrainset { line: LineId(l), spec: 0, count: 2 });
        w.apply(&Command::SetHeadway { line: LineId(l), headway_ms: 120_000 });
    }
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn a_trip_completes_only_via_the_walking_interchange() {
    let mut w = footpath_world();
    for _ in 0..20_000 {
        w.tick(50); // ~17 min — a full ride + ~3.6 min walk + ride + arrive needs the headroom
    }
    let st = w.stats_snapshot();
    let by = |id: u32| st.per_station.iter().find(|p| p.station_id == id).cloned().unwrap_or_default();

    // Riders boarded line 0 at the west end (0) AND line 1 at the walk target (2) — proof that
    // someone rode, walked the gap, and rode on. And trips completed at BOTH far ends (0↔3).
    assert!(by(0).boardings > 0.0, "line 0 carries riders from the west end (got {})", by(0).boardings);
    assert!(by(2).boardings > 0.0, "line 1 boards riders who WALKED in from station 1 (got {})", by(2).boardings);
    assert!(by(3).alightings > 0.0, "a 0→3 trip completed via the footpath (got {} arrivals)", by(3).alightings);
    assert!(by(0).alightings > 0.0, "a 3→0 trip completed via the footpath (got {} arrivals)", by(0).alightings);
}

#[test]
fn footpath_routing_is_deterministic() {
    let mut a = footpath_world();
    let mut b = footpath_world();
    for _ in 0..20_000 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "same seed + log ⇒ identical state with footpaths");
    assert!(a.stats_snapshot().ridership_total > 0.0, "the walking-interchange network actually carries riders");
}
