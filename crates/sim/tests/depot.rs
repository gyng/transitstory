//! Depot rework — a line runs trains only if one of its stops is a DEPOT (built + connected), when the
//! depot requirement is on. Baked on for arcadia (the fantasy rolling-stock rework); a runtime opt-in for
//! the real-city transit mode (off by default ⇒ byte-identical, proven by the determinism/arcadia golden
//! tests). Here we prove the per-line gate: no depot ⇒ no trains; add a depot stop ⇒ trains; toggle off ⇒
//! trains run freely; and the whole thing replays bit-for-bit.
use sim::*;

fn city(require_depot: bool) -> CityData {
    CityData {
        id: "t".into(),
        seed: 1,
        require_depot,
        demand: DemandGrid { cell_m: 500.0, cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 30.0, dest_w: 30.0, commodity: 0 }] },
        ..Default::default()
    }
}

fn rejected(evs: &[Event]) -> bool {
    evs.iter().any(|e| matches!(e, Event::Rejected { .. }))
}

#[test]
fn a_line_without_a_depot_cannot_run_trains() {
    let mut w = World::new(7, city(true)); // depot required
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    // No depot on this line ⇒ assigning trains is rejected.
    let evs = w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    assert!(rejected(&evs), "a depot-less line must be refused rolling stock: {evs:?}");
    assert!(w.lines[0].trainset.is_none(), "no trainset attached");
}

#[test]
fn a_depot_stop_unlocks_rolling_stock() {
    let mut w = World::new(7, city(true));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceDepot { x_mm: 1_000_000, y_mm: 0, name: None }); // station 2, a depot
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None }); // the depot, connected
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    let evs = w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    assert!(!rejected(&evs), "a line served by a depot runs trains: {evs:?}");
    assert!(w.lines[0].trainset.is_some(), "the trainset attached");
}

#[test]
fn requirement_off_runs_trains_freely() {
    let mut w = World::new(7, city(false)); // the shipped default — no depot requirement
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    let evs = w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    assert!(!rejected(&evs), "with the requirement off, any line runs trains");
}

#[test]
fn the_requirement_toggles_at_runtime() {
    // Transit opt-in: start free, toggle the requirement on, and a depot-less line loses its rolling stock.
    let mut w = World::new(7, city(false));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    assert!(!rejected(&w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 })), "free before the toggle");
    w.apply(&Command::SetRequireDepot { enabled: true });
    assert!(rejected(&w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 })), "gated after the toggle (no depot)");
}

#[test]
fn depot_gate_replays_deterministically() {
    let run = || {
        let mut w = World::new(9, city(true));
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceDepot { x_mm: 1_000_000, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..400 { w.tick(50); }
        w.state_hash()
    };
    assert_eq!(run(), run(), "a depot-served line replays bit-for-bit");
}
