//! Vehicle motion: dispatch + arc-length advance is deterministic, advances along the line,
//! stays in bounds, and reverses at the end (out-and-back).
use sim::*;

fn line_world() -> World {
    let mut w = World::new(1, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None }); // 3 km E
    w.apply(&Command::PlaceStation { x_mm: 6_000_000, y_mm: 0, name: None }); // 6 km E
    w.apply(&Command::CreateLine { color: 1 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 300_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn vehicle_movement_advances_and_is_deterministic() {
    let mut a = line_world();
    a.tick(50); // first tick dispatches vehicles
    assert!(a.vehicles.len() >= 1, "vehicles dispatched");
    let s0 = a.vehicles.s_mm[0];
    for _ in 0..200 {
        a.tick(50); // ~10 s
    }
    assert_ne!(s0, a.vehicles.s_mm[0], "vehicle advanced along the line");
    assert!(
        a.vehicles.s_mm[0] >= 0 && a.vehicles.s_mm[0] <= a.lines[0].length_mm(),
        "vehicle stays within line bounds",
    );

    // Determinism: a second identical run reaches the identical hashed state.
    let mut b = line_world();
    for _ in 0..201 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn vehicle_reverses_at_the_end() {
    let mut w = line_world();
    let mut reversed = false;
    for _ in 0..6000 {
        w.tick(50); // up to 5 min
        if w.vehicles.dir.iter().any(|&d| d == -1) {
            reversed = true;
            break;
        }
    }
    assert!(reversed, "a vehicle reverses direction at a line end");
}

#[test]
fn no_vehicles_until_running() {
    let mut w = World::new(1, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    // Not running yet: ticking still dispatches the vehicles but they don't move.
    w.tick(50);
    let s_before = w.vehicles.s_mm.clone();
    for _ in 0..50 {
        w.tick(50);
    }
    assert_eq!(s_before, w.vehicles.s_mm, "vehicles do not move while paused");
}
