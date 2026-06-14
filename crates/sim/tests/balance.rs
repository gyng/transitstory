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

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Baked-CONTINENT balance (the demo's `play` proves the demo; this proves the large baked world).
// The procedural continent (build_world.py, seed 12) places towns 60+ km from the SW-coastal capital —
// ~40× the 1.5 km demo. The demo-tuned army speed (50 m/s) can't reach a town inside a playable window,
// so this harness mirrors the baked scale (gentle 6/s decadence from 5345, towns 60 km out) and tunes
// the externalised `army_speed_mm_s` knob the bake sets. Synthetic-but-faithful: distances + decadence
// match the bake, supply rides a 2-input ARMS Liebig chain (ore+aether), conquest marches the 60 km.
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The army speed the bake (`build_world.py`) sets for the continent — kept in sync with `ARMY_SPEED_MM_S`
/// there. 200 m/s ⇒ a 60 km march in ~300 s: a few-minute legion for an epic continent, well inside the
/// ~40-min decadence runway, and observable in the e2e probe. (The demo default is 50 000.)
const BAKED_ARMY_SPEED_MM_S: i64 = 200_000;

/// A continent-scale realm at the baked knobs. Supply rides CONTINENT-LENGTH lines (sources ~30 km from
/// the ARMS town — the real bake's sources sit tens of km from towns, so the tribute ramp is realistic,
/// not the optimistic few-km of a toy world). `with_conquest`: a capital-BARRACKS on a 60 km line to a
/// far town (legions field + march to conquer) versus an IDLE realm (a plain capital, supply only, no
/// barracks ⇒ no legions) — the contrast that makes "the realm holds" an EARNED, non-vacuous result.
/// `army_speed_mm_s` is the tuned knob; decadence is the baked gentle 6/s from 5345.
fn play_continent(army_speed_mm_s: i64, with_conquest: bool, horizon: usize) -> Telemetry {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12,
        grid_cell_mm: 250_000,         // the baked hex pitch
        initial_decadence: 5345,       // the certified seed-12 starting corruption
        decadence_growth_per_s: 6,     // the baked gentle lose-meter rate (froze under the old truncation)
        army_speed_mm_s,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                // ARMS supply chain at CONTINENT scale (ore + aether sources ~30 km from the ARMS town,
                // matching the real bake where sources sit tens of km out): Liebig ore+aether → tribute.
                DemandCell { x_mm: 0, y_mm: 60_000_000, origin_w: 90.0, dest_w: 2.0, commodity: 0 }, // ore source (60 km N)
                DemandCell { x_mm: 30_000_000, y_mm: 30_000_000, origin_w: 90.0, dest_w: 2.0, commodity: 2 }, // aether source (~42 km out)
                DemandCell { x_mm: 0, y_mm: 30_000_000, origin_w: 2.0, dest_w: 90.0, commodity: 0 }, // ARMS town: ore demand (30 km N)
                DemandCell { x_mm: 0, y_mm: 30_000_000, origin_w: 2.0, dest_w: 90.0, commodity: 2 }, // ARMS town: aether demand
                // conquest target town, 60 km E of the capital (the baked nearest-town reach)
                DemandCell { x_mm: 60_000_000, y_mm: 0, origin_w: 2.0, dest_w: 40.0, commodity: 0 },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(12, city);
    // The capital: a BARRACKS (conquest) or a plain station (idle — no legions ⇒ the rot runs unchecked).
    if with_conquest {
        w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None }); // N0 capital-barracks
    } else {
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // N0 capital (no conquest)
    }
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 60_000_000, name: None }); // N1 ore source
    w.apply(&Command::PlaceStation { x_mm: 30_000_000, y_mm: 30_000_000, name: None }); // N2 aether source
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 30_000_000, name: None }); // N3 ARMS town (sink)
    w.apply(&Command::PlaceStation { x_mm: 60_000_000, y_mm: 0, name: None }); // N4 conquest target
    // Supply line: ore → ARMS town → aether (both inputs delivered ⇒ Liebig fires ⇒ tribute). Runs in
    // BOTH arms — the idle realm still earns tribute; it simply never fields legions (no barracks).
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false }); // line 0
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(3), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    if with_conquest {
        // Conquest line: barracks → far target town (the legion launches here and marches 60 km).
        w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 0, literal: false }); // line 1
        w.apply(&Command::AddStop { line: LineId(1), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(1), station: StationId(4), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(1), spec: 0, count: 2 });
        w.apply(&Command::SetHeadway { line: LineId(1), headway_ms: 120_000 });
    }
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

/// THE baked-continent pacing gate: at the bake's `army_speed_mm_s`, the continent realm must SUPPLY,
/// FIELD a legion, march it 60 km, CONQUER, and HOLD — and the hold must be EARNED, proven by a contrast
/// against an IDLE realm (same supply, no barracks) that IS overrun in the same window. The horizon runs
/// PAST the idle-loss point so "holds" is non-vacuous; the army-speed sweep is printed (`--nocapture`) so
/// conquest timing (the march bottleneck) is visible. Note: the harness models supply at continent scale
/// but is still synthetic — the real seed-12 geometry (and that the demo speed misses the window there)
/// is proven end-to-end by `e2e/fantasy-conquest.spec.ts`; this gate certifies the knobs are sound + the
/// win is conquest-attributable.
#[test]
fn fantasy_baked_continent_is_winnable() {
    // 60 000 ticks = 3000 sim-s. The idle realm loses at (20000−5345)/6 = 2442 sim-s ≈ 48 840 ticks, so a
    // realm still holding at the horizon held BECAUSE conquest pushed the rot back (not because time ran out).
    const HORIZON: usize = 60_000;
    for speed in [50_000i64, 100_000, BAKED_ARMY_SPEED_MM_S, 400_000] {
        let t = play_continent(speed, true, HORIZON);
        eprintln!(
            "army_speed {speed}: tribute@{} legion@{} conquest@{} decay_peak={} final(tribute={},towns={},lost={})",
            t.first_tribute, t.first_legion, t.first_conquest, t.decadence_peak, t.final_tribute, t.final_towns, t.lost
        );
    }

    // Earned-win CONTRAST: same continent + supply, no barracks ⇒ no conquest ⇒ the rot overruns it.
    let idle = play_continent(BAKED_ARMY_SPEED_MM_S, false, HORIZON);
    eprintln!("IDLE (no conquest): conquest@{} decay_peak={} lost={}", idle.first_conquest, idle.decadence_peak, idle.lost);
    assert_eq!(idle.first_conquest, 0, "idle realm: no barracks ⇒ no conquest (the contrast control)");
    assert!(idle.lost, "idle realm: an unconquering continent realm IS overrun in the horizon (the rot has teeth)");

    // At the baked army speed the realm SUPPLIES → FIELDS → CONQUERS → HOLDS — and the hold is earned
    // (the idle control above falls in the same window, so it isn't the horizon merely running out).
    let t = play_continent(BAKED_ARMY_SPEED_MM_S, true, HORIZON);
    assert!(t.first_tribute > 0, "continent: the supply loop never produced tribute");
    assert!(t.first_legion > 0, "continent: tribute never funded a legion");
    assert!(t.first_conquest > 0, "continent: no town conquered — the army can't reach a 60 km town in window");
    assert!(!t.lost, "continent: the realm was overrun despite conquering — not winnable at these knobs");
    assert!(t.first_tribute <= t.first_legion, "continent: legion before tribute?!");
    assert!(t.first_legion <= t.first_conquest, "continent: conquest before a legion?!");
}
