//! Buses are the ROAD-bound mode: cheap + fast along an existing road (class::ROAD, which the
//! bake already produces but nothing used), but slow and pricey (building a busway) off-road.
//! Deterministic — integer raster lookup, the same machinery as the street-running speed cap.
use sim::city::class;
use sim::*;

/// A horizontal ROAD strip along y=0 (x ∈ [-5,5] km); everything else defaults to OPEN.
fn road_city() -> CityData {
    let mut cells = Vec::new();
    for cx in -50..50 {
        cells.push(BuildCell { x_mm: cx * 100_000, y_mm: 0, c: class::ROAD });
    }
    CityData {
        id: "road".into(),
        seed: 7,
        buildability: BuildabilityGrid { cell_m: 100.0, cells },
        demand: DemandGrid { cell_m: 200.0, cells: vec![] },
        patience_ms: 0,
        ..Default::default()
    }
}

/// A 2-stop BUS line from (-3 km, y) to (3 km, y), one vehicle. Returns its line id.
fn bus_line(w: &mut World, y: i64) -> u32 {
    let s0 = w.stations.len() as u32;
    w.apply(&Command::PlaceStation { x_mm: -3_000_000, y_mm: y, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: y, name: None });
    let li = w.lines.len() as u32;
    w.apply(&Command::CreateLine { color: 0xd55e00, name: None, loop_line: false, mode: 1 }); // BUS
    w.apply(&Command::AddStop { line: LineId(li), station: StationId(s0), after: None });
    w.apply(&Command::AddStop { line: LineId(li), station: StationId(s0 + 1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(li), spec: 0, count: 1 });
    w.apply(&Command::SetHeadway { line: LineId(li), headway_ms: 300_000 });
    li
}

#[test]
fn a_bus_is_cheaper_and_faster_on_a_road_than_off_road() {
    let mut w = World::new(7, road_city());
    let road = bus_line(&mut w, 0); // along the road (y=0)
    let offroad = bus_line(&mut w, 2_000_000); // 2 km north, over OPEN land (no road)
    w.apply(&Command::SetRunning { running: true });

    let st = w.stats_snapshot();
    let cap = |id: u32| st.per_line.iter().find(|l| l.line_id == id).map(|l| l.capital_cost).unwrap();
    // B: an on-road bus lays no track (rides the existing road); an off-road one builds a busway.
    assert!(cap(road) < cap(offroad), "on-road bus is cheaper to build: {} < {}", cap(road), cap(offroad));

    // A: run a while; the on-road vehicle covers more arc-length than the off-road one (which
    // crawls without a road). Few enough ticks that neither reaches the far stop.
    for _ in 0..1000 {
        w.tick(50);
    }
    let s_of = |line: u32| -> i64 {
        (0..w.vehicles.len()).find(|&i| w.vehicles.line[i].0 == line).map(|i| w.vehicles.s_mm[i]).unwrap_or(0)
    };
    assert!(s_of(road) > s_of(offroad), "on-road bus is faster: s_road {} > s_offroad {}", s_of(road), s_of(offroad));
}

#[test]
fn bus_road_awareness_is_deterministic() {
    let build = || {
        let mut w = World::new(7, road_city());
        bus_line(&mut w, 0);
        bus_line(&mut w, 2_000_000);
        w.apply(&Command::SetRunning { running: true });
        w
    };
    let (mut a, mut b) = (build(), build());
    for _ in 0..1000 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "road-aware bus physics replays bit-for-bit");
}
