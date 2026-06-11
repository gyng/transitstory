//! Ridership develops on a serviced line, is deterministic across replays, and the coverage
//! score is monotonic under a network superset (PLAN §7 / game-design P3).
use sim::*;

fn city() -> CityData {
    city_patience(0) // patience 0 => renege disabled (existing ridership/coverage tests)
}

fn city_patience(patience_ms: i64) -> CityData {
    // Demand cells strung along the corridor so the stations have catchment.
    let cells = (0..20)
        .map(|k| DemandCell { x_mm: 300_000 * k, y_mm: 0, origin_w: 3.0, dest_w: 3.0 })
        .collect();
    CityData {
        id: "t".into(),
        seed: 7,
        demand: DemandGrid { cell_m: 300.0, cells },
        patience_ms,
        ..Default::default()
    }
}

/// 3 stations always placed; the line covers the given subset of stops at the given headway.
fn world_with_stops_headway(stops: &[u32], headway_ms: i64) -> World {
    let mut w = World::new(7, city());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    for &s in stops {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// 3 stations always placed; the line covers the given subset of stops (default headway).
fn world_with_stops(stops: &[u32]) -> World {
    world_with_stops_headway(stops, 10_000) // 5 clock-min
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

#[test]
fn coverage_score_rewards_shorter_headway() {
    // Same coverage, shorter headway => the gauge must NOT be lower (and here, strictly higher:
    // a frequent line is better service than an infrequent one over identical stops).
    let mut slow = world_with_stops_headway(&[0, 1, 2], 40_000); // 20 clock-min
    slow.tick(50);
    let mut fast = world_with_stops_headway(&[0, 1, 2], 2_000); // 1 clock-min
    fast.tick(50);
    let (slow_c, fast_c) = (slow.stats_snapshot().coverage_score, fast.stats_snapshot().coverage_score);
    assert!(fast_c >= slow_c, "shorter headway never lowers coverage ({fast_c} >= {slow_c})");
    assert!(fast_c > slow_c, "a frequent line should out-score an infrequent one ({fast_c} > {slow_c})");
}

#[test]
fn coverage_score_monotonic_under_superset_and_shorter_headway() {
    // The AGENTS contract: superset network + shorter headway => score not lower.
    let mut base = world_with_stops_headway(&[0, 1], 40_000); // 20 clock-min
    base.tick(50);
    let mut better = world_with_stops_headway(&[0, 1, 2], 2_000); // 1 clock-min
    better.tick(50);
    assert!(
        better.stats_snapshot().coverage_score >= base.stats_snapshot().coverage_score,
        "superset + shorter headway scores at least the baseline ({} >= {})",
        better.stats_snapshot().coverage_score,
        base.stats_snapshot().coverage_score,
    );
}

#[test]
fn lifecycle_telemetry_populates_and_left_behind_aliases_denials() {
    // Journey/wait telemetry must populate once trips complete, and left_behind must be the
    // real cumulative denied-boarding counter (not the live waiting-queue depth).
    let mut w = world_with_stops_headway(&[0, 1, 2], 2_000); // 1 clock-min
    for _ in 0..6000 {
        w.tick(50);
    }
    let st = w.stats_snapshot();
    assert!(st.ridership_total > 0.0, "riders board");
    assert!(st.avg_wait_ms > 0.0, "platform wait telemetry populates after boardings");
    assert!(st.avg_journey_ms > 0.0, "journey telemetry populates after completed trips");
    assert_eq!(st.left_behind, st.denied_boardings, "left_behind aliases denied_boardings");
}

#[test]
fn crowded_stops_extend_dwell_beyond_base() {
    // Load-dependent dwell is the bunching mechanism: a vehicle that boards/alights a crowd dwells
    // longer than the base. Run a busy single line and catch a dwell extended past base+ε.
    let mut w = world_with_stops(&[0, 1, 2]); // headway 200_000, demand corridor
    let base = sim::trainset::spec_for_mode(0).dwell_ms; // rail base dwell
    let mut saw_extended = false;
    for _ in 0..8000 {
        w.tick(50);
        for i in 0..w.vehicles.len() {
            // dwell_until set this tick to clock + base + extra; extra>0 pushes it past clock+base.
            if w.vehicles.dwell_until_ms[i] > w.clock_ms + base + 50 {
                saw_extended = true;
            }
        }
        if saw_extended {
            break;
        }
    }
    assert!(saw_extended, "a crowded stop dwells longer than the base (bunching pressure)");

    // Determinism holds with load-dependent dwell.
    let mut a = world_with_stops(&[0, 1, 2]);
    let mut b = world_with_stops(&[0, 1, 2]);
    for _ in 0..3000 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn riders_abandon_when_service_is_too_infrequent() {
    // One slow vehicle on a long route can't drain the queue within a short patience window,
    // so waiting riders give up — the difficulty signal. Deterministic, and disabled at p=0.
    let build = |patience: i64| {
        let mut w = World::new(7, city_patience(patience));
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
        for s in [0, 1, 2] {
            w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
        }
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 }); // sparse service
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 }); // 60 clock-min (the max)
        w.apply(&Command::SetRunning { running: true });
        w
    };

    let mut a = build(4_000); // 2 clock-min patience
    for _ in 0..8000 {
        a.tick(50); // 400 s
    }
    assert!(a.stats_snapshot().abandoned > 0.0, "infrequent service loses riders to renege");

    // Deterministic replay.
    let mut b = build(4_000);
    for _ in 0..8000 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash());

    // Patience 0 disables renege entirely.
    let mut c = build(0);
    for _ in 0..8000 {
        c.tick(50);
    }
    assert_eq!(c.stats_snapshot().abandoned, 0.0, "patience 0 => nobody gives up");
}
