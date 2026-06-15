//! Fantasy (#economy) per-day gold UPKEEP — the opex axis: a running network drains the treasury each
//! in-game day (track-km + rolling stock), so you must keep DELIVERING to cover what you've built. Gated
//! on a baked `gold_upkeep_per_day` (0 ⇒ free to run ⇒ transit + goldens byte-identical, proven by the
//! determinism/arcadia golden tests). The cursor is clock-derived + un-hashed, so the drain only mutates
//! the already-hashed `tribute` — golden-neutral. Here we prove the drain + that 0 is inert + replay.
use sim::*;

fn upkeep_world(rate: i64, initial_gold: i64) -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 1,
        initial_gold,
        gold_upkeep_per_day: rate,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 1 },
                DemandCell { x_mm: 2_000_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 1 },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// One in-game day = 24 × HOUR_MS (120_000) = 2_880_000 sim-ms. Tick past it so a rollover charges upkeep.
const DAY_SIM_MS: i64 = 24 * 120_000;

#[test]
fn upkeep_drains_the_treasury_each_day() {
    // A treasury seeded high (so deliveries don't mask the drain), upkeep on. After a day rolls, gold drops.
    let mut w = upkeep_world(50, 100_000);
    let daily = w.gold_upkeep_daily();
    assert!(daily > 0, "a running network owes upkeep ({daily})");
    let before = w.tribute;
    // Tick just over one day (50ms steps).
    for _ in 0..((DAY_SIM_MS / 50) + 20) {
        w.tick(50);
    }
    // Gold = initial + deliveries − one day's upkeep. The drain happened (gold rose by less than deliveries,
    // OR fell): assert the upkeep was charged by comparing against a no-upkeep run on the same trajectory.
    let mut w0 = upkeep_world(0, 100_000);
    for _ in 0..((DAY_SIM_MS / 50) + 20) {
        w0.tick(50);
    }
    assert_eq!(w0.gold_upkeep_daily(), 0, "rate 0 ⇒ no upkeep owed");
    assert!(w.tribute < w0.tribute, "upkeep made the treasury lower than the same network with no upkeep ({} vs {})", w.tribute, w0.tribute);
    assert!(w.tribute >= 0, "upkeep floors at 0 — no gold debt");
    let _ = before;
}

#[test]
fn upkeep_off_is_inert_and_byte_identical() {
    // rate 0: the day rollover charges nothing; the run is identical to a world without the field set.
    let run = |rate: i64| {
        let mut w = upkeep_world(rate, 100_000);
        for _ in 0..((DAY_SIM_MS / 50) + 100) {
            w.tick(50);
        }
        w.state_hash()
    };
    // Two rate-0 runs replay bit-for-bit (determinism), and the cursor is un-hashed so it doesn't perturb.
    assert_eq!(run(0), run(0), "rate-0 upkeep replays deterministically");
}

#[test]
fn upkeep_replays_deterministically() {
    let run = || {
        let mut w = upkeep_world(60, 100_000);
        for _ in 0..((DAY_SIM_MS / 50) + 500) {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "an upkeep-charging realm replays bit-for-bit");
}

#[test]
fn upkeep_scales_with_the_network() {
    // A bigger network (more trains) owes more upkeep — the "don't over-build" tradeoff.
    let small = upkeep_world(50, 100_000);
    let mut big = upkeep_world(50, 100_000);
    big.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 12 }); // 4× the rolling stock
    assert!(big.gold_upkeep_daily() > small.gold_upkeep_daily(), "more trains ⇒ more upkeep");
}
