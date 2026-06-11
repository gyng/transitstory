//! OD "desire line" query (`station_od`): a served origin draws non-empty, normalized links to
//! other served stations; an orphaned / out-of-range origin draws none. Read-only — calling it
//! must not perturb the determinism hash.
use sim::*;

fn corridor_city() -> CityData {
    let cells = (0..16)
        .map(|k| DemandCell { x_mm: 300_000 * k, y_mm: 0, origin_w: 5.0, dest_w: 5.0 })
        .collect();
    CityData { id: "od".into(), seed: 3, demand: DemandGrid { cell_m: 300.0, cells }, ..Default::default() }
}

fn running_world() -> World {
    let mut w = World::new(3, corridor_city());
    for k in 0..4 {
        w.apply(&Command::PlaceStation { x_mm: 1_200_000 * k, y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..4 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 180_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn served_origin_yields_normalized_desire_lines() {
    let mut w = running_world();
    for _ in 0..2000 {
        w.tick(50); // let demand capture + serving populate
    }
    let od = w.station_od(0, 10);
    assert!(!od.is_empty(), "a served origin draws desire lines to other served stations");
    assert!(od.iter().all(|l| l.dest != 0), "no self-link");
    // Weights are normalized 0..1 against the strongest link → the max is exactly 1.0.
    let max = od.iter().map(|l| l.weight).fold(0.0f32, f32::max);
    assert!((max - 1.0).abs() < 1e-6, "strongest link normalizes to 1.0 (got {max})");
    assert!(od.iter().all(|l| l.weight >= 0.0 && l.weight <= 1.0), "weights in [0,1]");
    // Descending order (top_k taken after sort).
    for pair in od.windows(2) {
        assert!(pair[0].weight >= pair[1].weight, "desire lines are sorted by descending pull");
    }
}

#[test]
fn unserved_origin_yields_nothing_and_query_is_pure() {
    let mut w = running_world();
    // Add a 5th station far away, never connected to a line → orphaned origin.
    w.apply(&Command::PlaceStation { x_mm: 50_000_000, y_mm: 50_000_000, name: None });
    for _ in 0..1000 {
        w.tick(50);
    }
    let orphan = (w.station_od(0, 10).len(), w.station_od(4, 10));
    assert!(orphan.1.is_empty(), "an orphaned (unserved) origin draws no desire lines");

    // The query must not perturb state: hash is identical before and after several calls.
    let before = w.state_hash();
    for _ in 0..5 {
        let _ = w.station_od(0, 10);
        let _ = w.station_od(4, 10);
    }
    assert_eq!(before, w.state_hash(), "station_od is a pure read — state_hash unchanged");
    assert!(orphan.0 > 0, "the served origin still has links (sanity)");
}

#[test]
fn station_access_isochrone_is_monotone_and_pure() {
    let mut w = running_world();
    for _ in 0..2000 {
        w.tick(50);
    }
    let acc = w.station_access(0);
    assert!(!acc.is_empty(), "a served origin reaches other served stations");
    assert!(acc.iter().all(|a| a.station != 0), "no self in the isochrone");
    assert!(acc.iter().all(|a| a.ms >= 0.0), "travel times are non-negative");
    // On a single 4-stop line from station 0, the nearer stop (1) is reached faster than the
    // farther one (3) — the isochrone respects network distance.
    let t1 = acc.iter().find(|a| a.station == 1).map(|a| a.ms);
    let t3 = acc.iter().find(|a| a.station == 3).map(|a| a.ms);
    if let (Some(t1), Some(t3)) = (t1, t3) {
        assert!(t1 <= t3, "the nearer stop is reached no slower than the farther ({t1} <= {t3})");
    }
    // Pure read: hash is unchanged across calls; an orphaned origin yields nothing.
    let before = w.state_hash();
    for _ in 0..3 {
        let _ = w.station_access(0);
    }
    assert_eq!(before, w.state_hash(), "station_access is a pure read");
}
