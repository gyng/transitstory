//! Ferries are the WATER-bound mode (the water twin of road-bound buses): cheap + full-speed on
//! open water (class::WATER), crawling + pricey if forced onto land. Reuses the buildability
//! raster; deterministic (integer raster lookup).
use sim::city::class;
use sim::*;

/// A horizontal WATER strip along y=0 (x ∈ [-5,5] km); everything else defaults to OPEN (land).
fn water_city() -> CityData {
    let mut cells = Vec::new();
    for cx in -50..50 {
        cells.push(BuildCell { x_mm: cx * 100_000, y_mm: 0, c: class::WATER });
    }
    CityData {
        id: "water".into(),
        seed: 7,
        buildability: BuildabilityGrid { cell_m: 100.0, cells },
        demand: DemandGrid { cell_m: 200.0, cells: vec![] },
        patience_ms: 0,
        ..Default::default()
    }
}

/// A 2-stop FERRY line from (-3 km, y) to (3 km, y), one vehicle. Returns its line id.
fn ferry_line(w: &mut World, y: i64) -> u32 {
    let s0 = w.stations.len() as u32;
    w.apply(&Command::PlaceStation { x_mm: -3_000_000, y_mm: y, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: y, name: None });
    let li = w.lines.len() as u32;
    w.apply(&Command::CreateLine { color: 0x009e73, name: None, loop_line: false, mode: 2 }); // FERRY
    w.apply(&Command::AddStop { line: LineId(li), station: StationId(s0), after: None });
    w.apply(&Command::AddStop { line: LineId(li), station: StationId(s0 + 1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(li), spec: 0, count: 1 });
    w.apply(&Command::SetHeadway { line: LineId(li), headway_ms: 300_000 });
    li
}

#[test]
fn a_ferry_is_cheaper_and_faster_on_water_than_on_land() {
    let mut w = World::new(7, water_city());
    let water = ferry_line(&mut w, 0); // on the open-water strip
    let land = ferry_line(&mut w, 2_000_000); // 2 km north, over OPEN land
    w.apply(&Command::SetRunning { running: true });

    let st = w.stats_snapshot();
    let cap = |id: u32| st.per_line.iter().find(|l| l.line_id == id).map(|l| l.capital_cost).unwrap();
    // B: an open-water ferry lays no capital; a ferry forced over land would have to build something.
    assert!(cap(water) < cap(land), "an open-water ferry is cheaper: {} < {}", cap(water), cap(land));

    // A: the water ferry cruises; the land-bound one crawls. Short window (2 sim-s): at the
    // clock-frame speeds a ferry covers ~500 m here — far from the terminal, so `s` is still a
    // monotone speed proxy (after a reversal the position comparison would be meaningless).
    for _ in 0..40 {
        w.tick(50);
    }
    let s_of = |line: u32| -> i64 {
        (0..w.vehicles.len()).find(|&i| w.vehicles.line[i].0 == line).map(|i| w.vehicles.s_mm[i]).unwrap_or(0)
    };
    assert!(s_of(water) > s_of(land), "the water ferry is faster: s_water {} > s_land {}", s_of(water), s_of(land));
}

#[test]
fn a_ferry_follows_the_water_channel_around_land() {
    // A U-shaped WATER channel: up the left (x=0), across the top (y=2 km), down the right
    // (x=4 km). Both terminals sit at the bottom ends — a straight line between them crosses LAND,
    // so the ferry must navigate the channel (the water twin of the bus's U-shaped road).
    let mut cells = Vec::new();
    for cy in 0..=20 {
        cells.push(BuildCell { x_mm: 0, y_mm: cy * 100_000, c: class::WATER });
    }
    for cx in 0..=40 {
        cells.push(BuildCell { x_mm: cx * 100_000, y_mm: 2_000_000, c: class::WATER });
    }
    for cy in 0..=20 {
        cells.push(BuildCell { x_mm: 40 * 100_000, y_mm: cy * 100_000, c: class::WATER });
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
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // bottom-left terminal, on water
    w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None }); // bottom-right terminal
    w.apply(&Command::CreateLine { color: 0x009e73, name: None, loop_line: false, mode: 2 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });

    let max_y = w.lines[0].polyline.iter().map(|p| p.y_mm).max().unwrap();
    assert!(max_y > 1_000_000, "the ferry follows the water channel, not straight over land (max y {max_y})");
}

#[test]
fn ferry_water_awareness_is_deterministic() {
    let build = || {
        let mut w = World::new(7, water_city());
        ferry_line(&mut w, 0);
        ferry_line(&mut w, 2_000_000);
        w.apply(&Command::SetRunning { running: true });
        w
    };
    let (mut a, mut b) = (build(), build());
    for _ in 0..1000 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "water-aware ferry physics replays bit-for-bit");
}
