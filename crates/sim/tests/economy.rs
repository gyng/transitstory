//! Economy (Tier 3): a line accrues capital cost (track + trains); tunnelling costs more to
//! build than surface; fares accrue from ridership and raise the balance over time.
use sim::*;

fn rail_world() -> World {
    let mut w = World::new(1, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w
}

#[test]
fn capital_accrues_and_tunnel_costs_more_than_surface() {
    let mut w = rail_world();
    let surface = w.lines[0].capital_cost;
    assert!(surface > 0, "track + trains cost capital");
    w.apply(&Command::SetSegmentMode { line: LineId(0), span: u32::MAX, mode: 2 }); // tunnel
    assert!(w.lines[0].capital_cost > surface, "tunnel is more expensive to build");

    let st = w.stats_snapshot();
    assert!(st.economy_enabled);
    assert!(st.balance < START_BUDGET as f64, "capital reduces the balance");
    assert!((st.capital_spent - w.lines[0].capital_cost as f64).abs() < 1.0);
}

#[test]
fn fares_grow_the_balance_over_time() {
    let cells = (0..20)
        .map(|k| DemandCell { x_mm: 300_000 * k, y_mm: 0, origin_w: 3.0, dest_w: 3.0 })
        .collect();
    let city = CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 300.0, cells }, ..Default::default() };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false });
    for s in [0, 1, 2] {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 200_000 });
    w.apply(&Command::SetRunning { running: true });

    let before = w.stats_snapshot().balance;
    for _ in 0..4000 {
        w.tick(50);
    }
    let after = w.stats_snapshot();
    assert!(after.fare_revenue > 0.0, "fares accrue from ridership");
    assert!(after.balance > before, "fares raise the balance over time");
}
