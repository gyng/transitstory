//! Ridership develops on a serviced line, is deterministic across replays, and the coverage
//! score is monotonic under a network superset (PLAN §7 / game-design P3).
use sim::*;

fn city() -> CityData {
    // Demand cells strung along the corridor so the stations have catchment.
    let cells = (0..20)
        .map(|k| DemandCell { x_mm: 300_000 * k, y_mm: 0, origin_w: 3.0, dest_w: 3.0 })
        .collect();
    CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 300.0, cells }, ..Default::default() }
}

/// 3 stations always placed; the line covers the given subset of stops.
fn world_with_stops(stops: &[u32]) -> World {
    let mut w = World::new(7, city());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1 });
    for &s in stops {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 200_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn ridership_develops_and_is_deterministic() {
    let mut a = world_with_stops(&[0, 1, 2]);
    for _ in 0..4000 {
        a.tick(50); // ~200 s
    }
    let st = a.stats_snapshot();
    assert!(st.ridership_total > 0.0, "ridership develops; got {}", st.ridership_total);
    assert!(st.coverage_score > 0, "coverage score is positive when demand is served");

    let mut b = world_with_stops(&[0, 1, 2]);
    for _ in 0..4000 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "identical replay => identical state");
    assert_eq!(a.ridership_total, b.ridership_total);
}

#[test]
fn coverage_score_monotonic_under_superset() {
    let mut base = world_with_stops(&[0, 1]); // covers 2 of 3 stations
    base.tick(50);
    let mut sup = world_with_stops(&[0, 1, 2]); // superset: covers all 3
    sup.tick(50);
    assert!(
        sup.stats_snapshot().coverage_score >= base.stats_snapshot().coverage_score,
        "a superset network scores at least the baseline ({} >= {})",
        sup.stats_snapshot().coverage_score,
        base.stats_snapshot().coverage_score,
    );
}
