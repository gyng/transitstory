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
fn buses_slow_in_peak_traffic() {
    // The congestion factor is a pure integer of the clock: worst at the peaks, none overnight.
    let peak = sim::tod::congestion_pct(120_000); // hour 8 (AM rush)
    let night = sim::tod::congestion_pct(1_200_000); // hour 2 (night)
    assert!(peak < night, "peak traffic is worse than night: {peak} < {night}");
    assert_eq!(night, 100, "no congestion overnight");
    assert!(peak >= 1 && peak <= 100, "congestion factor in (0, 100]");

    // A bus cruising a long ROAD moves slower once the AM peak hits.
    let mut cells = Vec::new();
    for cx in -1000..1000 {
        cells.push(BuildCell { x_mm: cx * 100_000, y_mm: 0, c: class::ROAD });
    }
    let city = CityData {
        id: "c".into(),
        seed: 7,
        buildability: BuildabilityGrid { cell_m: 100.0, cells },
        demand: DemandGrid { cell_m: 200.0, cells: vec![] },
        patience_ms: 0,
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: -90_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 90_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 0xd55e00, name: None, loop_line: false, mode: 1 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 600_000 });
    w.apply(&Command::SetRunning { running: true });

    for _ in 0..600 {
        w.tick(50); // → clock 30 s (hour 6, night): cruising at full speed
    }
    let v_night = w.vehicles.v_mm_s[0];
    for _ in 0..1800 {
        w.tick(50); // → clock 120 s (hour 8, AM peak)
    }
    let v_peak = w.vehicles.v_mm_s[0];
    assert!(v_peak < v_night, "the bus cruises slower in peak traffic: peak {v_peak} < night {v_night}");
    assert!(v_peak > 0, "but it still moves");
}

#[test]
fn a_bus_follows_the_road_between_stops() {
    // A U-shaped road: up the left wall (x=0), across the top (y=2 km), down the right wall
    // (x=4 km). Both stops sit at the bottom ends — a straight line between them is OPEN land, so
    // the only ROAD route detours up and over. The bus geometry must follow it.
    let mut cells = Vec::new();
    for cy in 0..=20 {
        cells.push(BuildCell { x_mm: 0, y_mm: cy * 100_000, c: class::ROAD }); // left wall
    }
    for cx in 0..=40 {
        cells.push(BuildCell { x_mm: cx * 100_000, y_mm: 2_000_000, c: class::ROAD }); // top
    }
    for cy in 0..=20 {
        cells.push(BuildCell { x_mm: 40 * 100_000, y_mm: cy * 100_000, c: class::ROAD }); // right wall
    }
    let city = CityData {
        id: "u".into(),
        seed: 7,
        buildability: BuildabilityGrid { cell_m: 100.0, cells },
        demand: DemandGrid { cell_m: 200.0, cells: vec![] },
        patience_ms: 0,
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // bottom-left, on the road
    w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None }); // bottom-right, on the road
    w.apply(&Command::CreateLine { color: 0xd55e00, name: None, loop_line: false, mode: 1 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });

    // A straight line stays at y≈0; routing the U-shaped road climbs to ~2 km.
    let max_y = w.lines[0].polyline.iter().map(|p| p.y_mm).max().unwrap();
    assert!(max_y > 1_000_000, "the bus routes along the U-shaped road, not straight (max y {max_y})");

    // …and it stays deterministic.
    let mut a = w;
    let mut b = World::new(7, a.city.clone());
    for c in a.cmd_log.clone() {
        b.apply(&c);
    }
    for _ in 0..400 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "road-followed bus geometry replays bit-for-bit");
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
