//! Agent-demand prototype BENCHMARK (run with: `cargo test -p sim --release --test agents_bench
//! -- --nocapture`). Builds a Tokyo-scale synthetic world and measures, agent vs gravity:
//! population generation time + memory, per-tick wall-clock, and replay determinism. `Instant` is
//! fine here — this is a test binary, not the determinism-gated `crates/sim/src`.
use sim::agents::{Citizen, Population};
use sim::*;
use std::time::Instant;

const N_SIDE: i64 = 20; // 20×20 = 400 stations
const SPACING_MM: i64 = 1_000_000; // 1 km grid
const CELL_SIDE: i64 = 45; // 45×45 ≈ 2025 demand cells

/// A Tokyo-scale grid city: 400 stations, 20 horizontal + 10 vertical lines (dense interchanges),
/// ~2000 demand cells with suburban homes (origin) + a downtown job core (dest).
fn build_tokyo_scale() -> World {
    let span = (N_SIDE - 1) * SPACING_MM;
    let centre = span as f64 / 2.0;
    // Demand grid: homes spread, jobs concentrated at the centre.
    let mut cells = Vec::new();
    let cstep = span / (CELL_SIDE - 1);
    for r in 0..CELL_SIDE {
        for c in 0..CELL_SIDE {
            let x = c * cstep;
            let y = r * cstep;
            let dx = x as f64 - centre;
            let dy = y as f64 - centre;
            let d2 = (dx * dx + dy * dy) / (centre * centre);
            let dest_w = (8.0 * (-2.0 * d2).exp()) as f32; // downtown jobs
            let origin_w = (1.5 + 2.0 * d2.min(1.0)) as f32; // suburban homes
            cells.push(DemandCell { x_mm: x, y_mm: y, origin_w, dest_w });
        }
    }
    let city = CityData {
        id: "bench".into(),
        seed: 42,
        demand: DemandGrid { cell_m: (cstep / 1000) as f64, cells },
        ..Default::default()
    };
    let mut w = World::new(42, city);
    // Stations in row-major order: id = r*N_SIDE + c.
    for r in 0..N_SIDE {
        for c in 0..N_SIDE {
            w.apply(&Command::PlaceStation { x_mm: c * SPACING_MM, y_mm: r * SPACING_MM, name: None });
        }
    }
    let mut line_id = 0u32;
    let mut add_line = |w: &mut World, stops: Vec<u32>| {
        w.apply(&Command::CreateLine { color: 0x0072b2, name: None, loop_line: false, mode: 0, literal: false });
        for s in &stops {
            w.apply(&Command::AddStop { line: LineId(line_id), station: StationId(*s), after: None });
        }
        w.apply(&Command::AssignTrainset { line: LineId(line_id), spec: 0, count: 4 });
        w.apply(&Command::SetHeadway { line: LineId(line_id), headway_ms: 180_000 });
        line_id += 1;
    };
    for r in 0..N_SIDE {
        add_line(&mut w, (0..N_SIDE).map(|c| (r * N_SIDE + c) as u32).collect());
    }
    for c in (0..N_SIDE).step_by(2) {
        add_line(&mut w, (0..N_SIDE).map(|r| (r * N_SIDE + c) as u32).collect());
    }
    w.apply(&Command::SetRunning { running: true });
    w.tick(50); // settle: dispatch vehicles, build serving + captured weights + footpaths
    w
}

fn run_ticks(w: &mut World, ticks: usize, dt: i64) -> f64 {
    let t = Instant::now();
    for _ in 0..ticks {
        w.tick(dt);
    }
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn agent_demand_benchmark() {
    let ticks = 6000usize;
    let dt = 50i64; // 300 s sim (06:00 + 300s/HOUR_MS hours) — covers the heart of the AM rush

    let base = build_tokyo_scale();
    println!("\n=== Agent-demand prototype benchmark (Tokyo-scale: {} stations, {} lines, {} cells) ===",
        base.stations.len(), base.lines.iter().filter(|l| l.trainset.is_some()).count(), base.city.demand.cells.len());
    println!("sizeof(Citizen) = {} bytes", std::mem::size_of::<Citizen>());

    // --- 1. Population generation: time + memory at several sizes ---
    for &n in &[50_000usize, 200_000, 1_000_000] {
        let t = Instant::now();
        let pop = Population::generate(&base, n, base.seed);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("gen N={:>9}: {:6.1} ms   ~{:5.1} MB heap", n, ms, pop.mem_bytes() as f64 / 1e6);
    }

    // --- 2. Per-tick wall-clock: gravity vs agents (same world, same schedule) ---
    let mut wg = build_tokyo_scale();
    let grav_ms = run_ticks(&mut wg, ticks, dt);

    let mut wa = build_tokyo_scale();
    let mut pop = Population::generate(&wa, 100_000, wa.seed);
    // Silence gravity for the agent run: zero captured weights (no station changes follow, so
    // `prepare()` won't recompute them) → the only trips come from the agent scheduler.
    wa.captured_origin.iter_mut().for_each(|x| *x = 0.0);
    wa.captured_dest.iter_mut().for_each(|x| *x = 0.0);
    let t = Instant::now();
    let mut spawn_ms = 0.0;
    for _ in 0..ticks {
        let s = Instant::now();
        pop.spawn_trips(&mut wa, dt);
        spawn_ms += s.elapsed().as_secs_f64() * 1000.0;
        wa.tick(dt);
    }
    let agent_ms = t.elapsed().as_secs_f64() * 1000.0;

    println!("\n{} ticks @ dt={}ms ({} in-game hours):", ticks, dt, ticks as i64 * dt / sim::tod::HOUR_MS);
    println!("  gravity: {:7.0} ms total   ({:.3} ms/tick)   ridership {:>6}   distinct routes {}",
        grav_ms, grav_ms / ticks as f64, wg.stats_snapshot().ridership_total as u64, wg.route_cache.len());
    println!("  agents : {:7.0} ms total   ({:.3} ms/tick)   ridership {:>6}   distinct routes {}",
        agent_ms, agent_ms / ticks as f64, wa.stats_snapshot().ridership_total as u64, wa.route_cache.len());
    println!("  agent spawn (cold cache): {:.0} ms total ({:.4} ms/tick) — i.e. ~{:.1} µs per distinct route (RAPTOR plan)",
        spawn_ms, spawn_ms / ticks as f64, spawn_ms * 1000.0 / wa.route_cache.len().max(1) as f64);

    // Warm-cache spawn: re-route the SAME trips with the cache already populated — isolates the
    // pure scheduling + Pax-push cost from the (one-time, amortized) RAPTOR routing.
    let mut pop_warm = Population::generate(&wa, 100_000, wa.seed); // same O/D set → all cache hits
    let mut warm_ms = 0.0;
    let mut clk = 0i64;
    for _ in 0..ticks {
        clk += dt;
        wa.clock_ms = clk; // advance the schedule window without a full tick (routing is what we isolate)
        let s = Instant::now();
        pop_warm.spawn_trips(&mut wa, dt);
        warm_ms += s.elapsed().as_secs_f64() * 1000.0;
    }
    println!("  agent spawn (WARM cache): {:.1} ms total ({:.5} ms/tick) — scheduling + queue push only\n",
        warm_ms, warm_ms / ticks as f64);

    // The whole point: agents must develop real ridership (the population actually rides).
    assert!(wa.stats_snapshot().ridership_total > 0.0, "agent population produces ridership");
}

/// A small served grid (25 stations, 10 lines — every row + column) for the mode/determinism test.
fn build_small() -> World {
    let cells: Vec<_> = (0..100)
        .map(|k| DemandCell { x_mm: (k % 10) * 1_000_000, y_mm: (k / 10) * 1_000_000, origin_w: 3.0, dest_w: 3.0 })
        .collect();
    let mut w = World::new(7, CityData { id: "s".into(), seed: 7, demand: DemandGrid { cell_m: 1000.0, cells }, ..Default::default() });
    for k in 0..25 {
        w.apply(&Command::PlaceStation { x_mm: (k % 5) * 2_000_000, y_mm: (k / 5) * 2_000_000, name: None });
    }
    let mut li = 0u32;
    let mut line = |w: &mut World, stops: Vec<u32>| {
        w.apply(&Command::CreateLine { color: 0, name: None, loop_line: false, mode: 0, literal: false });
        for s in stops {
            w.apply(&Command::AddStop { line: LineId(li), station: StationId(s), after: None });
        }
        w.apply(&Command::AssignTrainset { line: LineId(li), spec: 0, count: 3 });
        w.apply(&Command::SetHeadway { line: LineId(li), headway_ms: 120_000 });
        li += 1;
    };
    for r in 0..5 {
        line(&mut w, (0..5).map(|c| r * 5 + c).collect());
    }
    for c in 0..5 {
        line(&mut w, (0..5).map(|r| r * 5 + c).collect());
    }
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn agent_demand_mode_via_command_develops_ridership_and_replays() {
    let run = || {
        let mut w = build_small();
        w.apply(&Command::SetDemandMode { agents: true }); // the opt-in toggle (command-sourced)
        for _ in 0..3000 {
            w.tick(50); // run into the AM rush
        }
        w
    };
    assert!(run().stats_snapshot().ridership_total > 0.0, "agent-demand mode develops ridership via the command path");
    assert_eq!(run().state_hash(), run().state_hash(), "agent-demand mode (command-sourced) replays bit-for-bit");
}

#[test]
fn agent_demand_survives_an_empty_demand_grid() {
    // Empty grid (no cells) + agent demand must NOT panic in tick() — the cell_station map is never
    // length-rebuilt, so the lookup path has to be bounds-safe (regression: index-OOB panic).
    let mut w = World::new(7, CityData { id: "e".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells: vec![] }, ..Default::default() });
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 0, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetDemandMode { agents: true });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..3000 {
        w.tick(50); // must not panic
    }
    assert_eq!(w.stats_snapshot().ridership_total, 0.0, "no demand grid ⇒ no agent trips (but no panic)");
}

#[test]
fn agent_demand_picks_up_a_network_built_after_it_is_enabled() {
    // Enable agents BEFORE any line exists, then build — the cell→nearest-served-station map must
    // REFRESH on the network change (regression: it was frozen at "all unserved" → zero trips ever).
    let cells = vec![
        DemandCell { x_mm: 0, y_mm: 0, origin_w: 20.0, dest_w: 2.0 }, // homes near station 0
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 20.0 }, // jobs near station 1
    ];
    let mut w = World::new(7, CityData { id: "n".into(), seed: 7, demand: DemandGrid { cell_m: 500.0, cells }, ..Default::default() });
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::SetDemandMode { agents: true });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..600 {
        w.tick(50); // no served stations yet → cell_station all -1 → no trips
    }
    assert_eq!(w.stats_snapshot().ridership_total, 0.0, "no lines yet ⇒ no agent trips");
    // Build a SHORT, frequently-served line connecting home↔job AFTER agents were enabled — the
    // cell_station map must refresh (a long line wouldn't cycle a train back within the test window).
    w.apply(&Command::CreateLine { color: 0, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
    for _ in 0..8000 {
        w.tick(50);
    }
    assert!(w.stats_snapshot().ridership_total > 0.0, "agents pick up the network built AFTER enabling (cell_station refreshed)");
}

#[test]
fn journey_inspector_names_an_agent_commuter() {
    let mut w = build_small();
    w.apply(&Command::SetDemandMode { agents: true });
    let mut found = None;
    'outer: for _ in 0..4000 {
        w.tick(50);
        for s in 0..w.stations.len() as u32 {
            if let Some(j) = sim::journey::sample(&w, s, 0) {
                found = Some(j);
                break 'outer;
            }
        }
    }
    let j = found.expect("an agent commuter should be waiting somewhere during the AM rush");
    assert!(!j.anonymous, "agent-demand riders are named citizens, not anonymous");
    assert!(!j.name.is_empty(), "the commuter has a name");
    assert!(!j.legs.is_empty() && !j.dest.is_empty(), "the journey carries a route to a destination");
}

#[test]
fn agent_demand_is_deterministic() {
    let run = || {
        let mut w = build_tokyo_scale();
        let mut pop = Population::generate(&w, 40_000, w.seed);
        w.captured_origin.iter_mut().for_each(|x| *x = 0.0);
        w.captured_dest.iter_mut().for_each(|x| *x = 0.0);
        for _ in 0..3000 {
            pop.spawn_trips(&mut w, 50);
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "agent demand replays bit-for-bit (population is seed-derived)");
}
