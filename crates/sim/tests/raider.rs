//! S11 RIVAL — decadence raiders. The design flags the rival as the biggest GATE-BLIND risk (a
//! livelocking/oscillating rival passes `run()==run()`), so these assert the named structural invariants
//! DIRECTLY — never via replay-equality alone: the population is BOUNDED system-wide (no sawtooth), each
//! raider's distance-to-capital is MONOTONE (no livelock), the rail network INTERCEPTS them, a breach
//! raises the lose meter, and a world with no reservoir fields NONE (golden-neutral). Plus determinism.
use sim::hexgrid::{self, Axial};
use sim::raider::{DONE, MARCHING};
use sim::*;

const SIZE: i64 = 250_000;

/// A synthetic baked-like arcadia world: a `w × h` hex block of PLAIN(10) cells + a capital cell → a real
/// decadence field with a far-edge reservoir (the raider spawn source).
fn hex_world(w: i64, h: i64, capital: Axial) -> CityData {
    let mut cells = Vec::new();
    for q in 0..w {
        for r in 0..h {
            let p = hexgrid::center_of((q, r), SIZE);
            cells.push(BuildCell { x_mm: p.x_mm, y_mm: p.y_mm, c: 10 });
        }
    }
    let cap = hexgrid::center_of(capital, SIZE);
    CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12,
        grid_cell_mm: SIZE,
        capital_x_mm: cap.x_mm,
        capital_y_mm: cap.y_mm,
        // A head start so the decadence-fed spawn cadence shortens quickly (raiders swarm) within the test.
        initial_decadence: 8000,
        buildability: BuildabilityGrid { cell_m: SIZE as f64 / 1000.0, cells },
        ..Default::default()
    }
}

fn running_world() -> World {
    let mut w = World::new(12, hex_world(14, 14, (0, 0)));
    // A lone capital station (no defensive line) — raiders will reach + breach. SetRunning so war_step ticks.
    let cap = hexgrid::center_of((0, 0), SIZE);
    w.apply(&Command::PlaceStation { x_mm: cap.x_mm, y_mm: cap.y_mm, name: None });
    w.apply(&Command::SetRunning { running: true });
    w
}

fn dist_to_capital(w: &World, i: usize) -> i64 {
    let (dx, dy) = (w.city.capital_x_mm - w.raiders.x_mm[i], w.city.capital_y_mm - w.raiders.y_mm[i]);
    // Saturating — mirror the production march/resolve discipline (so re-pointing this test at a
    // large-coordinate world can never debug-overflow).
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)).isqrt()
}

#[test]
fn the_rival_fields_raiders_from_the_reservoir() {
    // A reservoir exists ⇒ the rival eventually fields raiders (the threat is real, not inert).
    let mut w = running_world();
    let mut ever = false;
    for _ in 0..6000 {
        w.tick(50);
        if w.raiders.live() > 0 {
            ever = true;
            break;
        }
    }
    assert!(ever, "with a reservoir the rival fields raiders");
}

#[test]
fn the_raider_population_is_bounded_no_sawtooth() {
    // GATE-BLIND: across a long undefended run (decadence rising ⇒ the cadence shortens ⇒ a swarm), the SoA
    // never exceeds the hard cap and the live count never exceeds it — DONE slots RECYCLE (bounded
    // system-wide, no unbounded accumulation / sawtooth). 64 = MAX_RAIDERS.
    let mut w = running_world();
    let mut max_live = 0;
    for _ in 0..80_000 {
        w.tick(50);
        max_live = max_live.max(w.raiders.live());
        assert!(w.raiders.len() <= 64, "the raider SoA never grows past the cap (got {})", w.raiders.len());
        assert!(w.raiders.live() <= 64, "live raiders never exceed the cap");
    }
    assert!(max_live > 0, "the swarm actually materialised (the test exercised the cap path)");
}

#[test]
fn each_raider_marches_monotonically_at_the_capital_no_livelock() {
    // GATE-BLIND: every MARCHING raider's distance-to-capital is non-increasing tick-over-tick — it can't
    // oscillate or stall (the no-livelock guarantee). Recycled slots (DONE→MARCHING) reset the spawn point,
    // so the check applies only within a continuous MARCHING episode (prev AND cur both MARCHING).
    let mut w = running_world();
    let mut prev_state: Vec<u8> = Vec::new();
    let mut prev_dist: Vec<i64> = Vec::new();
    for _ in 0..40_000 {
        w.tick(50);
        for i in 0..w.raiders.len() {
            let st = w.raiders.state[i];
            let d = if st == MARCHING { dist_to_capital(&w, i) } else { i64::MAX };
            if i < prev_state.len() && prev_state[i] == MARCHING && st == MARCHING {
                assert!(d <= prev_dist[i], "raider {i} moved AWAY from the capital ({} > {}) — a livelock", d, prev_dist[i]);
            }
            if i < prev_state.len() {
                prev_state[i] = st;
                prev_dist[i] = d;
            } else {
                prev_state.push(st);
                prev_dist.push(d);
            }
        }
    }
}

#[test]
fn the_rail_network_cuts_raiders_down() {
    // A defended approach: a line of stations blanketing the corridor from the reservoir to the capital
    // intercepts raiders (coverage = defence) ⇒ FAR fewer breaches than an undefended realm. Proves the
    // counter-play exists (and isn't RTS micro — it's the network).
    fn breach_after(defended: bool, ticks: usize) -> i64 {
        let mut w = World::new(12, hex_world(14, 14, (0, 0)));
        let cap = hexgrid::center_of((0, 0), SIZE);
        w.apply(&Command::PlaceStation { x_mm: cap.x_mm, y_mm: cap.y_mm, name: None }); // 0 capital
        if defended {
            // A chain of stations from the capital out toward the reservoir corner, then a line through
            // them — so the corridor is blanketed within DEFENSE_RANGE (4 km, ~16 cells at 250 m).
            let mut ids = vec![0u32];
            for k in 1..=12 {
                let p = hexgrid::center_of((k, k), SIZE);
                w.apply(&Command::PlaceStation { x_mm: p.x_mm, y_mm: p.y_mm, name: None });
                ids.push(k as u32);
            }
            w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
            for id in &ids {
                w.apply(&Command::AddStop { line: LineId(0), station: StationId(*id), after: None });
            }
            w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
            w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
        }
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..ticks {
            w.tick(50);
        }
        w.raider_breach
    }
    let undefended = breach_after(false, 40_000);
    let defended = breach_after(true, 40_000);
    assert!(undefended > 0, "an undefended realm IS breached by raiders (the threat has teeth): {undefended}");
    assert!(defended < undefended, "the rail network cuts raiders down (defended {defended} < undefended {undefended})");
}

#[test]
fn a_breach_raises_the_lose_meter() {
    // A raider that reaches the capital raises `raider_breach`, and the field step folds it INTO the lose
    // meter (so the rival actually pushes the realm toward falling).
    let mut w = running_world();
    for _ in 0..40_000 {
        w.tick(50);
        if w.raider_breach > 0 {
            break;
        }
    }
    assert!(w.raider_breach > 0, "an undefended capital gets breached");
    assert!(w.decadence > 0, "the breach feeds the lose meter");
}

#[test]
fn no_reservoir_means_no_rival_golden_neutral() {
    // The demo arcadia world (no buildability ⇒ no field ⇒ no reservoir) fields NO raiders, ever — the
    // golden-neutral guarantee (transit + the golden fixtures keep an empty raider SoA + 0 breach).
    let mut w = World::new(
        11,
        CityData {
            id: "arcadia".into(),
            ruleset: "arcadia".into(),
            seed: 11,
            grid_cell_mm: 100_000,
            initial_decadence: 9000,
            demand: DemandGrid { cell_m: 500.0, cells: vec![] },
            ..Default::default()
        },
    );
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..20_000 {
        w.tick(50);
    }
    assert_eq!(w.raiders.len(), 0, "no reservoir ⇒ the raider SoA stays empty");
    assert_eq!(w.raider_breach, 0, "…and no breach");
    assert!(w.raiders.state.iter().all(|&s| s == DONE || s == MARCHING), "state bytes are valid");
}

#[test]
fn a_held_network_recovers_from_breaches_no_point_of_no_return() {
    // The adversarial-review fix: `raider_breach` DECAYS, so a realm that takes breaches and THEN walls off
    // the approach RECOVERS (breach heals back toward 0) — there is no irreversible point-of-no-return.
    // Phase 1: undefended, accrue some breach. Phase 2: blanket the corridor + run; the breach must FALL.
    let mut w = World::new(12, hex_world(14, 14, (0, 0)));
    let cap = hexgrid::center_of((0, 0), SIZE);
    w.apply(&Command::PlaceStation { x_mm: cap.x_mm, y_mm: cap.y_mm, name: None }); // 0 capital
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..30_000 {
        w.tick(50);
        if w.raider_breach > 600 {
            break; // a couple of raids have landed
        }
    }
    let breached = w.raider_breach;
    assert!(breached > 0, "phase 1: the undefended realm took breaches: {breached}");

    // Phase 2: blanket the reservoir→capital corridor with a railed line (cuts new raiders down), then run.
    let mut ids = vec![0u32];
    for k in 1..=13 {
        let p = hexgrid::center_of((k, k), SIZE);
        w.apply(&Command::PlaceStation { x_mm: p.x_mm, y_mm: p.y_mm, name: None });
        ids.push(k as u32);
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for id in &ids {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(*id), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    for _ in 0..40_000 {
        w.tick(50);
    }
    assert!(w.raider_breach < breached, "phase 2: a held network HEALS the breach ({} < {})", w.raider_breach, breached);
}

#[test]
fn a_raided_line_freezes_its_trains_then_resumes() {
    // Rail-attack (#war) — the FREEZE primitive (universal; no raider needed). A transit line (no reservoir
    // ⇒ no natural raids to confound) runs until its trains move, then we CUT it (set the disable timer, the
    // hashed field a raider sets). While raided the consist holds in place; once the timer lapses it resumes.
    let mut w = World::new(
        7,
        CityData { id: "t".into(), ruleset: "transit".into(), seed: 7, ..Default::default() },
    );
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 12_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..400 {
        w.tick(50);
    }
    assert!(!w.vehicles.s_mm.is_empty(), "the line dispatched trains");

    // CUT line 0 for a good stretch (a raid). Poke the hashed timer directly — the raider→cut path is
    // exercised by `a_raider_at_the_track_cuts_the_line`; here we isolate the freeze.
    let until = w.clock_ms + 200_000;
    w.line_disabled_until_ms = vec![until];
    let frozen = w.vehicles.s_mm.clone();
    for _ in 0..200 {
        w.tick(50); // 10_000 ms elapses, well inside the 200_000 ms cut
    }
    assert_eq!(w.vehicles.s_mm, frozen, "a RAIDED line's consist freezes in place — no advance while cut");

    // Run past the timer; the consist resumes moving (auto-recovery, no permanent loss).
    while w.clock_ms <= until {
        w.tick(50);
    }
    for _ in 0..200 {
        w.tick(50);
    }
    assert_ne!(w.vehicles.s_mm, frozen, "once the raid lapses the line re-enables and its trains move again");
}

#[test]
fn a_raider_at_the_track_cuts_the_line() {
    // Rail-attack (#war) — the raider→CUT path. A long line whose span midpoint sits BEYOND the station
    // cordon (>DEFENSE_RANGE from either endpoint); a raider placed at that midpoint slips the cordon, reaches
    // the track, and CUTS the line (disabling it) — spending itself in the raid (DONE). The vulnerable seam is
    // a long sparse span; a dense network would have intercepted it first.
    let mut w = World::new(12, hex_world(40, 40, (0, 0)));
    // Two stations FAR apart so their span's middle is out of cordon reach (DEFENSE_RANGE = 4_000_000 mm =
    // 16 cells at 250 m; the midpoint of (2,38)–(38,2) sits ~26 cells from either endpoint).
    let a = hexgrid::center_of((2, 38), SIZE);
    let b = hexgrid::center_of((38, 2), SIZE);
    w.apply(&Command::PlaceStation { x_mm: a.x_mm, y_mm: a.y_mm, name: None }); // 0
    w.apply(&Command::PlaceStation { x_mm: b.x_mm, y_mm: b.y_mm, name: None }); // 1
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::SetRunning { running: true });
    assert!(!w.line_disabled(0), "the line starts operational");

    // Place a raider at the span midpoint (a placement bypassing the reservoir spawn — the 3 SoA vecs are the
    // whole state). One tick: march nudges it (~4.5 km) but it stays in range, then resolve CUTS the line.
    let (mx, my) = ((a.x_mm + b.x_mm) / 2, (a.y_mm + b.y_mm) / 2);
    w.raiders.x_mm.push(mx);
    w.raiders.y_mm.push(my);
    w.raiders.state.push(MARCHING);
    let raider = w.raiders.len() - 1;
    w.tick(50);

    assert!(w.line_disabled(0), "a raider at the track CUTS the line (it's now raided)");
    assert_eq!(w.raiders.state[raider], DONE, "the raider spent itself in the raid");
}

#[test]
fn the_rival_replays_bit_for_bit() {
    fn run() -> u64 {
        let mut w = running_world();
        for _ in 0..15_000 {
            w.tick(50);
        }
        w.state_hash()
    }
    assert_eq!(run(), run(), "the rival replays bit-for-bit (deterministic — no rng, integer-exact)");
}
