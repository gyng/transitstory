//! RAPTOR: time-dependent (frequency-aware) routing. Unlike the min-transfer `BfsRouter`,
//! `RaptorRouter` minimises expected travel time = per-leg wait (~headway/2) + in-vehicle time
//! (mode speed × arc-length) + per-stop dwell, bounded to `max_legs` trips (RAPTOR rounds).
//! These tests pin the behaviours BFS CANNOT express: prefer a faster line, and accept a
//! transfer when frequency/speed make it worthwhile — while staying deterministic in the loop.
use sim::*;

/// Assert a leg list is a contiguous origin→dest path (each transfer at a shared station).
fn check_path(legs: &[Leg], origin: u32, dest: u32) {
    assert!(!legs.is_empty(), "a reachable trip yields ≥1 leg");
    assert_eq!(legs[0].board, StationId(origin), "first leg boards at the origin");
    assert_eq!(legs.last().unwrap().alight, StationId(dest), "last leg alights at the dest");
    for w in legs.windows(2) {
        assert_eq!(w[0].alight, w[1].board, "legs are contiguous — transfer at a shared station");
    }
}

fn base_world() -> World {
    World::new(7, CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells: vec![] }, ..Default::default() })
}

fn place(w: &mut World, x_mm: i64, y_mm: i64) {
    w.apply(&Command::PlaceStation { x_mm, y_mm, name: None });
}

fn line(w: &mut World, mode: u8, stops: &[u32], headway_ms: i64) {
    w.apply(&Command::CreateLine { color: 0x0072b2, name: None, loop_line: false, mode });
    let li = LineId((w.lines.len() - 1) as u32);
    for &s in stops {
        w.apply(&Command::AddStop { line: li, station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: li, spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: li, headway_ms });
}

/// Two ways from O(0) to D(1): a slow BUS line that detours via 2, and a fast HEAVY line direct.
/// Both are single-leg, so min-transfer BFS is indifferent (picks the first) — RAPTOR must pick
/// the faster (heavy) line.
fn two_direct_world() -> World {
    let mut w = base_world();
    place(&mut w, 0, 0); // 0 = O
    place(&mut w, 10_000_000, 0); // 1 = D (10 km east)
    place(&mut w, 5_000_000, 0); // 2 = midpoint
    line(&mut w, trainset_mode_bus(), &[0, 2, 1], 180_000); // line 0: slow bus, detours via 2
    line(&mut w, trainset_mode_heavy(), &[0, 1], 180_000); // line 1: fast heavy rail, direct
    w.apply(&Command::SetRunning { running: true });
    w.tick(50); // build the serving map
    w
}

// tmode ids mirrored from trainset::tmode (kept local so the test reads intent, not magic numbers).
fn trainset_mode_bus() -> u8 {
    1
}
fn trainset_mode_heavy() -> u8 {
    4
}

/// Phase-aware transfer wait (the "auto-timetable" subslice): a TRANSFER boards a line that is
/// somewhat coordinated with the feeder, so it waits LESS than a cold origin boarding (h/2). The
/// origin boarding stays h/2 (unbiased arrival). Derived purely from RAPTOR labels so the
/// assertion cancels ride/dwell and pins the exact integer term.
#[test]
fn transfer_wait_is_phase_aware() {
    let mut w = base_world();
    let h = 600_000;
    place(&mut w, 0, 0); // 0 = O
    place(&mut w, 1_000_000, 0); // 1 = T (1 km — beyond the 400 m footpath range)
    place(&mut w, 2_000_000, 0); // 2 = D
    line(&mut w, 0, &[0, 1], h); // line 0: O→T
    line(&mut w, 0, &[1, 2], h); // line 1: T→D
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);

    let from_o = RaptorRouter.reachable(&w.lines, &w.serving, &w.footpaths, StationId(0), 4);
    let from_t = RaptorRouter.reachable(&w.lines, &w.serving, &w.footpaths, StationId(1), 4);
    // from_t[D] - h/2 is the pure T→D ride+dwell (T is the ORIGIN there, pays h/2). Subtract it
    // from the O→D minus O→T delta to isolate the transfer wait paid at T — no ride/dwell constants.
    let r_td = from_t[2] - h / 2;
    let transfer_wait = from_o[2] - from_o[1] - r_td;
    assert!(transfer_wait < h / 2, "a transfer waits LESS than a cold origin boarding: {transfer_wait} < {}", h / 2);
    assert!(transfer_wait >= 0 && transfer_wait <= h, "transfer wait stays within [0, headway]");
    assert_eq!(transfer_wait, 3 * h / 8, "transfer wait = 3/8 headway (coordinated), not the flat h/2");
}

#[test]
fn raptor_prefers_the_faster_line_where_bfs_is_indifferent() {
    let w = two_direct_world();
    // BFS (min legs): both options are 1 leg; it returns the first-scanned line (the slow bus).
    let bfs = BfsRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(0), StationId(1), 4).expect("bfs route");
    assert_eq!(bfs.len(), 1);
    assert_eq!(bfs[0].line, LineId(0), "BFS is time-blind — returns the first single-leg line");

    // RAPTOR (min time): picks the fast heavy-rail line, still a single leg.
    let rap = RaptorRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(0), StationId(1), 4).expect("raptor route");
    check_path(&rap, 0, 1);
    assert_eq!(rap.len(), 1, "the faster option is still a direct single leg");
    assert_eq!(rap[0].line, LineId(1), "RAPTOR routes over the faster (heavy-rail) line");
}

/// A slow, infrequent BUS goes O(0)→D(1) directly (1 leg). A pair of fast, frequent HEAVY lines
/// goes O→T(2)→D with one transfer (2 legs). BFS takes the direct line (fewer transfers); RAPTOR
/// takes the transfer because wait+ride is far lower — the whole point of frequency-aware routing.
fn slow_direct_vs_fast_transfer_world() -> World {
    let mut w = base_world();
    place(&mut w, 0, 0); // 0 = O
    place(&mut w, 20_000_000, 0); // 1 = D (20 km east)
    place(&mut w, 10_000_000, 0); // 2 = T (interchange, 10 km)
    line(&mut w, trainset_mode_bus(), &[0, 1], 1_200_000); // line 0: direct but 20-min headway + slow
    line(&mut w, trainset_mode_heavy(), &[0, 2], 120_000); // line 1: O→T, 2-min headway, fast
    line(&mut w, trainset_mode_heavy(), &[2, 1], 120_000); // line 2: T→D, 2-min headway, fast
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    w
}

#[test]
fn raptor_takes_a_transfer_when_frequency_and_speed_make_it_faster() {
    let w = slow_direct_vs_fast_transfer_world();
    // BFS: the direct line is a single leg, so it wins on transfer count regardless of time.
    let bfs = BfsRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(0), StationId(1), 4).expect("bfs route");
    assert_eq!(bfs.len(), 1, "BFS minimises transfers — one slow direct leg");
    assert_eq!(bfs[0].line, LineId(0));

    // RAPTOR: two fast frequent legs beat one slow infrequent leg.
    let rap = RaptorRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(0), StationId(1), 4).expect("raptor route");
    check_path(&rap, 0, 1);
    assert_eq!(rap.len(), 2, "RAPTOR accepts a transfer to ride the fast frequent corridor");
    assert_eq!(rap[0].line, LineId(1));
    assert_eq!(rap[1].line, LineId(2));
    assert_eq!(rap[0].alight, StationId(2), "transfers at the interchange T");
}

/// A linear chain O(0)–[L0]–1–[L1]–2–[L2]–D(3): the only path is three legs. RAPTOR must honour
/// the `max_legs` (RAPTOR-round) bound — unreachable within 2 legs, reachable within 3.
fn three_leg_chain_world() -> World {
    let mut w = base_world();
    for k in 0..4 {
        place(&mut w, k as i64 * 10_000_000, 0); // 0,1,2,3
    }
    line(&mut w, 0, &[0, 1], 120_000);
    line(&mut w, 0, &[1, 2], 120_000);
    line(&mut w, 0, &[2, 3], 120_000);
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    w
}

#[test]
fn raptor_respects_the_max_legs_bound() {
    let w = three_leg_chain_world();
    assert!(
        RaptorRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(0), StationId(3), 2).is_none(),
        "a 3-leg-only trip is unreachable within max_legs=2",
    );
    let ok = RaptorRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(0), StationId(3), 3).expect("reachable in 3 legs");
    check_path(&ok, 0, 3);
    assert_eq!(ok.len(), 3, "the chain needs exactly three legs");
}

#[test]
fn raptor_reaches_everything_bfs_can() {
    // Reachability parity: wherever BFS finds a route within the bound, RAPTOR finds one too
    // (it may differ in which legs, but it is never less connective).
    let w = slow_direct_vs_fast_transfer_world();
    for (o, d) in [(0u32, 1u32), (0, 2), (2, 1), (1, 0)] {
        let bfs = BfsRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(o), StationId(d), 4);
        let rap = RaptorRouter.plan(&w.lines, &w.serving, &w.footpaths, StationId(o), StationId(d), 4);
        assert_eq!(bfs.is_some(), rap.is_some(), "RAPTOR reachability matches BFS for {o}->{d}");
        if let Some(legs) = rap {
            check_path(&legs, o, d);
        }
    }
}

/// Two crossing lines (interchange at station 2); RAPTOR is the World's default router. Ridership
/// develops and replay stays bit-identical — the determinism gate over the new routing path.
fn crossing_world() -> World {
    let cells = (0..30)
        .map(|k| DemandCell { x_mm: 200_000 * (k - 15), y_mm: 0, origin_w: 3.0, dest_w: 3.0 })
        .collect();
    let mut w = World::new(7, CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells }, ..Default::default() });
    for k in 0..5 {
        place(&mut w, (k as i64 - 2) * 1_500_000, 0); // 0..4 along y=0
    }
    place(&mut w, 0, 3_000_000); // 5
    place(&mut w, 0, 1_500_000); // 6
    place(&mut w, 0, -1_500_000); // 7
    place(&mut w, 0, -3_000_000); // 8
    line(&mut w, 0, &[0, 1, 2, 3, 4], 180_000);
    line(&mut w, 0, &[5, 6, 2, 7, 8], 180_000);
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn raptor_routing_is_deterministic_in_the_loop() {
    let mut a = crossing_world();
    for _ in 0..6000 {
        a.tick(50);
    }
    assert!(a.stats_snapshot().ridership_total > 0.0, "riders move on the default (RAPTOR) router");

    let mut b = crossing_world();
    for _ in 0..6000 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "RAPTOR routing stays deterministic across replays");
}
