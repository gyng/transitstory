//! The per-station pressure buckets (`denied`/`abandoned` in each StationStat) must sum to the
//! global `denied_boardings`/`abandoned` totals — i.e. the loss is only ever re-located to the
//! platform it happened at, never created or dropped. Bucketed loss is the per-station starvation
//! signal; this pins the bookkeeping and that it stays deterministic across replays.
use sim::*;

/// Heavy demand strung along a corridor so a low-capacity, infrequent line both DENIES boardings
/// (full trains pass queues) and sheds riders to renege (patience exceeded).
fn starved_city() -> CityData {
    let cells = (0..12)
        .map(|k| DemandCell { x_mm: 250_000 * k, y_mm: 0, origin_w: 12.0, dest_w: 12.0, commodity: 0 })
        .collect();
    CityData {
        id: "starve".into(),
        seed: 11,
        demand: DemandGrid { cell_m: 250.0, cells },
        patience_ms: 4_000, // riders give up after 2 CLOCK minutes of waiting
        ..Default::default()
    }
}

fn starved_world() -> World {
    let mut w = World::new(11, starved_city());
    for k in 0..3 {
        w.apply(&Command::PlaceStation { x_mm: 1_000_000 * k, y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..3 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    // One train on a long headway: deliberately too little service for the demand.
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 }); // sparse: 60 clock-min
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn per_station_pressure_sums_to_the_global_totals() {
    let mut w = starved_world();
    for _ in 0..8000 {
        w.tick(50); // ~400 s — long enough to fill trains and time out queues
    }
    let st = w.stats_snapshot();

    let denied_sum: f64 = st.per_station.iter().map(|p| p.denied).sum();
    let abandoned_sum: f64 = st.per_station.iter().map(|p| p.abandoned).sum();

    assert_eq!(denied_sum, st.denied_boardings, "Σ per-station denied == global denied_boardings");
    assert_eq!(abandoned_sum, st.abandoned, "Σ per-station abandoned == global abandoned");
    // The scenario must actually exercise the pressure path, or the equality is vacuous.
    assert!(
        st.denied_boardings + st.abandoned > 0.0,
        "starved scenario produces some denied/abandoned pressure (denied={}, abandoned={})",
        st.denied_boardings,
        st.abandoned,
    );
}

#[test]
fn pressure_buckets_are_deterministic() {
    let mut a = starved_world();
    let mut b = starved_world();
    for _ in 0..8000 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "identical replay => identical state (buckets hashed)");
    let (sa, sb) = (a.stats_snapshot(), b.stats_snapshot());
    let pa: Vec<(f64, f64)> = sa.per_station.iter().map(|p| (p.denied, p.abandoned)).collect();
    let pb: Vec<(f64, f64)> = sb.per_station.iter().map(|p| (p.denied, p.abandoned)).collect();
    assert_eq!(pa, pb, "per-station pressure buckets replay bit-for-bit");
}
