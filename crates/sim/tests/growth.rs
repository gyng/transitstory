//! Transit-oriented demand growth: once per in-game day, cells near a SERVED station grow at the
//! city's growth rate, the rest at a third (ambient sprawl); capped; deterministic; disabled at 0.
use sim::tod::HOUR_MS;
use sim::*;

const DAY_MS: i64 = 24 * HOUR_MS;

/// Two demand cells: one at the origin (inside the line's catchment), one 5 km east (outside).
fn city(growth_bp: i64) -> CityData {
    CityData {
        id: "g".into(),
        seed: 7,
        demand: DemandGrid {
            cell_m: 300.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 2.0, dest_w: 1.0 },
                DemandCell { x_mm: 5_000_000, y_mm: 0, origin_w: 2.0, dest_w: 1.0 },
            ],
        },
        growth_bp_per_day: growth_bp,
        ..Default::default()
    }
}

/// One operational 2-stop line whose first stop sits on the first cell.
fn served_world(growth_bp: i64) -> World {
    let mut w = World::new(7, city(growth_bp));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Tick the world across `days` in-game day boundaries.
fn run_days(w: &mut World, days: i64) {
    let ticks = days * DAY_MS / 50 + 2;
    for _ in 0..ticks {
        w.tick(50);
    }
}

#[test]
fn cells_near_served_stations_grow_faster_than_ambient() {
    let mut w = served_world(250);
    let near0 = w.city.demand.cells[0].origin_w;
    let far0 = w.city.demand.cells[1].origin_w;
    run_days(&mut w, 3);
    let near = w.city.demand.cells[0].origin_w;
    let far = w.city.demand.cells[1].origin_w;
    assert!(near > near0, "a served cell grows: {near} > {near0}");
    assert!(far > far0, "an unserved cell still sprawls: {far} > {far0}");
    assert!(near > far, "transit-adjacent growth outpaces ambient: {near} > {far}");
    // dest weights grow too
    assert!(w.city.demand.cells[0].dest_w > 1.0, "dest weight grows alongside origin");
}

#[test]
fn growth_is_capped_and_disabled_at_zero() {
    // Cap: 2× the strongest initial cell (2.0) = 4.0; many days can't exceed it.
    let mut w = served_world(2_000); // 20%/day to hit the cap fast
    run_days(&mut w, 8);
    let near = w.city.demand.cells[0].origin_w;
    assert!(near <= 4.0 + 1e-4, "growth caps at 2x the strongest initial cell: {near} <= 4.0");

    // bp = 0 disables growth entirely (CityData::default() behaviour for native tests).
    let mut z = served_world(0);
    run_days(&mut z, 3);
    assert_eq!(z.city.demand.cells[0].origin_w, 2.0, "growth_bp 0 => the grid never changes");
}

#[test]
fn agent_population_tops_up_with_growth_and_replays() {
    // A city with enough homes that the population target clears the 1k floor, so growth moves it.
    let mk = || {
        let cells = (0..20)
            .map(|k| DemandCell { x_mm: 200_000 * k, y_mm: 0, origin_w: 2.0, dest_w: 2.0 })
            .collect();
        let mut w = World::new(7, CityData {
            id: "ag".into(),
            seed: 7,
            demand: DemandGrid { cell_m: 200.0, cells },
            growth_bp_per_day: 2_000, // 20%/day so the homes-derived target visibly rises
            ..Default::default()
        });
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
        w.apply(&Command::SetDemandMode { agents: true });
        w.apply(&Command::SetRunning { running: true });
        w
    };
    let mut a = mk();
    let n0 = a.population.as_ref().map(|p| p.citizens.len()).unwrap_or(0);
    run_days(&mut a, 3);
    let n3 = a.population.as_ref().map(|p| p.citizens.len()).unwrap_or(0);
    assert!(n3 > n0, "the population tops up as the city grows: {n3} > {n0}");

    // Bit-for-bit replay with agents + growth + top-ups all active.
    let mut b = mk();
    run_days(&mut b, 3);
    assert_eq!(a.state_hash(), b.state_hash(), "agent top-ups replay deterministically");
    assert_eq!(n3, b.population.as_ref().map(|p| p.citizens.len()).unwrap_or(0));
}

#[test]
fn growth_replays_deterministically_and_moves_the_coverage_denominator() {
    let mut a = served_world(250);
    let mut b = served_world(250);
    run_days(&mut a, 2);
    run_days(&mut b, 2);
    assert_eq!(a.state_hash(), b.state_hash(), "growth is a pure function of clock + service");

    // The stats snapshot exposes the grown denominator for the frontend day report.
    let total = a.stats_snapshot().demand_origin_total;
    assert!(total > 4.0, "demand_origin_total reflects the grown grid: {total} > 4.0");
}
