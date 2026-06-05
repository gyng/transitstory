//! Accessibility-weighted demand: a trip's destination is chosen weighted by how fast the
//! transit network *reaches* it (wait + ride), not crow-flies distance. So a good network induces
//! demand toward the places it connects well — closing the loop between routing and demand.
use sim::*;

fn base() -> World {
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

const HEAVY: u8 = 4;
const BUS: u8 = 1;

#[test]
fn reachable_returns_ordered_one_to_all_times() {
    // O(0)→P(1) on a fast frequent heavy line; O(0)→Q(2) on a slow infrequent bus; R(3) isolated.
    let mut w = base();
    place(&mut w, 0, 0); // O
    place(&mut w, 10_000_000, 0); // P (10 km, fast line)
    place(&mut w, 0, 10_000_000); // Q (10 km, slow line)
    place(&mut w, 5_000_000, 5_000_000); // R (no line)
    line(&mut w, HEAVY, &[0, 1], 120_000);
    line(&mut w, BUS, &[0, 2], 1_200_000);
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);

    let acc = RaptorRouter.reachable(&w.lines, &w.serving, StationId(0), 4);
    assert_eq!(acc.len(), w.serving.len(), "one entry per station");
    assert_eq!(acc[0], 0, "the origin reaches itself at t=0");
    assert!(acc[1] > 0 && acc[1] < i64::MAX, "P is reachable");
    assert!(acc[2] > 0 && acc[2] < i64::MAX, "Q is reachable");
    assert!(acc[1] < acc[2], "the fast frequent line reaches P sooner than the slow one reaches Q");
    assert_eq!(acc[3], i64::MAX, "an off-network station is unreachable (i64::MAX)");
}

#[test]
fn the_default_router_exposes_accessibility() {
    // `BfsRouter` opts out (empty vec → callers fall back to geometry); the World's default does not.
    let mut w = base();
    place(&mut w, 0, 0);
    place(&mut w, 10_000_000, 0);
    line(&mut w, 0, &[0, 1], 120_000);
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert!(BfsRouter.reachable(&w.lines, &w.serving, StationId(0), 4).is_empty(), "BFS exposes no accessibility");
    assert!(!RaptorRouter.reachable(&w.lines, &w.serving, StationId(0), 4).is_empty(), "RAPTOR (default) does");
}

/// Two equally-attractive job clusters equidistant from a residential origin, but one (P) is on a
/// fast frequent line and the other (Q) on a slow infrequent one. Trips should skew to the better-
/// connected P even though jobs and distance are identical — demand follows accessibility.
fn two_jobs_world() -> World {
    // Demand cells sit exactly on the stations (500 m catchment, stations 10 km apart → no overlap):
    //   O: homes (origin)   P,Q: equal jobs (destinations)
    let cells = vec![
        DemandCell { x_mm: 0, y_mm: 0, origin_w: 50.0, dest_w: 0.0 }, // O = residential
        DemandCell { x_mm: 10_000_000, y_mm: 0, origin_w: 0.0, dest_w: 20.0 }, // P = jobs
        DemandCell { x_mm: 0, y_mm: 10_000_000, origin_w: 0.0, dest_w: 20.0 }, // Q = equal jobs
    ];
    let mut w = World::new(7, CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells }, ..Default::default() });
    place(&mut w, 0, 0); // 0 = O
    place(&mut w, 10_000_000, 0); // 1 = P
    place(&mut w, 0, 10_000_000); // 2 = Q
    line(&mut w, HEAVY, &[0, 1], 120_000); // O↔P: fast, 2-min headway
    line(&mut w, BUS, &[0, 2], 1_200_000); // O↔Q: slow, 20-min headway
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn demand_skews_toward_the_better_connected_destination() {
    let mut w = two_jobs_world();
    for _ in 0..20_000 {
        w.tick(300); // ~100 min of AM service — plenty of home→job trips
    }
    let to_p = w.alightings[1];
    let to_q = w.alightings[2];
    assert!(to_p > 0, "trips arrive at the well-connected jobs (P)");
    assert!(to_q > 0, "the poorly-connected jobs (Q) still get some trips (baseline keeps them possible)");
    assert!(to_p > to_q, "more trips choose the fast-frequent destination ({to_p} P vs {to_q} Q)");
}

#[test]
fn accessibility_weighted_demand_is_deterministic() {
    let run = || {
        let mut w = two_jobs_world();
        for _ in 0..8_000 {
            w.tick(300);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "accessibility weighting (RAPTOR reachable + access_cache) stays deterministic");
}
