//! The balance harness (fantasy-build-plan.md "Fast iteration, telemetry & balancing"): run the
//! fantasy loop HEADLESS across seeds, collect pacing telemetry, and gate the two things paper can't
//! answer — is the loop WINNABLE (conquest outpaces the rot) and does it BITE (the flywheel turns
//! within a sane horizon)? This is the determinism dividend: a pure, reproducible sim self-plays, so
//! the non-derivable knobs (LAUNCH_COST, decadence rates, the catchment, BUFFER_CAP) can be checked +
//! tuned without a human. Prints the per-seed telemetry (run with `--nocapture`) for inspection.
use sim::*;

/// One scripted playthrough's pacing telemetry (ticks to each milestone; 0 = never reached).
#[derive(Debug, Default)]
struct Telemetry {
    first_tribute: usize,
    first_legion: usize,
    first_conquest: usize,
    decadence_peak: i64,
    final_tribute: i64,
    final_towns: i64,
    lost: bool,
}

/// A standard "supply a town + field legions from a barracks" realm — the canonical fantasy loop. The
/// source↔town are kept well past the ~500 m catchment so supply flows (the surfaced lesson).
fn play(seed: u64, horizon: usize) -> Telemetry {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: 0 }, // barracks + ore source
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 90.0, commodity: 0 }, // the town to feed + take
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(seed, city);
    w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });

    let mut t = Telemetry::default();
    for tick in 1..=horizon {
        w.tick(50);
        let s = w.stats_snapshot();
        if t.first_tribute == 0 && s.tribute > 0.0 {
            t.first_tribute = tick;
        }
        if t.first_legion == 0 && s.army_count >= 1 {
            t.first_legion = tick;
        }
        if t.first_conquest == 0 && s.towns_captured >= 1.0 {
            t.first_conquest = tick;
        }
        t.decadence_peak = t.decadence_peak.max(w.decadence);
    }
    t.final_tribute = w.tribute;
    t.final_towns = w.towns_captured;
    t.lost = sim::decadence::is_lost(&w);
    t
}

/// THE pacing gate: across several seeds, the canonical realm must SUPPLY (tribute), FIELD a legion,
/// CONQUER a town, and HOLD (not be overrun) — within the horizon. Proves the loop is winnable + bites,
/// for every seed (determinism makes any failure a reproducible counterexample). Prints the telemetry.
#[test]
fn fantasy_loop_is_winnable_and_bites() {
    const HORIZON: usize = 12_000; // 10 sim-minutes at 50ms/tick — a generous "does it turn at all" window
    for seed in [1u64, 7, 42, 100, 2024] {
        let t = play(seed, HORIZON);
        eprintln!(
            "seed {seed}: tribute@{} legion@{} conquest@{} decay_peak={} final(tribute={},towns={},lost={})",
            t.first_tribute, t.first_legion, t.first_conquest, t.decadence_peak, t.final_tribute, t.final_towns, t.lost
        );
        assert!(t.first_tribute > 0, "seed {seed}: the supply loop never produced tribute");
        assert!(t.first_legion > 0, "seed {seed}: tribute never funded a legion (war stalled)");
        assert!(t.first_conquest > 0, "seed {seed}: no town was ever conquered (the loop doesn't close)");
        assert!(!t.lost, "seed {seed}: the realm was overrun despite conquering — not winnable at these knobs");
        // Ordering sanity: you must earn tribute before a legion, and field a legion before a conquest.
        assert!(t.first_tribute <= t.first_legion, "seed {seed}: legion before tribute?!");
        assert!(t.first_legion <= t.first_conquest, "seed {seed}: conquest before a legion?!");
        // SOFT pacing gate (the design's "bites in ~60–120 s" target): the first conquest must land
        // within ~120 s (2400 ticks @ 50 ms). Harness-tuned to ~84 s; this catches a knob change that
        // would let the flywheel drag. A balance regression, not a correctness one — re-tune (don't
        // just bump the bound) if it trips.
        assert!(t.first_conquest <= 2400, "seed {seed}: first conquest @{} ticks — loop drags past the ~120s bite window", t.first_conquest);
    }
}

/// The harness is itself deterministic: the same seed yields identical telemetry (so a tuning sweep's
/// counterexamples replay bit-for-bit).
#[test]
fn balance_telemetry_is_reproducible() {
    let a = play(42, 6_000);
    let b = play(42, 6_000);
    assert_eq!(
        (a.first_tribute, a.first_legion, a.first_conquest, a.decadence_peak, a.final_tribute, a.final_towns, a.lost),
        (b.first_tribute, b.first_legion, b.first_conquest, b.decadence_peak, b.final_tribute, b.final_towns, b.lost),
        "the balance harness must be reproducible (determinism dividend)"
    );
}
