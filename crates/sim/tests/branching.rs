//! P3 — branching lines (see docs/capacity-roadmap.md). A line is a trunk plus a tree of branches;
//! the engine derives a root-to-leaf service path per branch and trains run them (round-robin), so a
//! Y-shaped real service (the Circle Line CE branch, the Jurong Region Line's 3-way Bahar Junction)
//! is representable and operable. Written RED-first: a train must reach the branch terminus, which it
//! cannot until branches are dispatched and traversed. Tested through Commands + observable positions.
use sim::*;

/// A Y: trunk A(0,0)–B(2km,0)–C(4km,0); a branch leaves the trunk at B (trunk stop 1) to
/// D(2km,2km)–E(2km,4km). Service paths: [A,B,C] and [A,B,D,E].
fn y_line(trains: u16) -> World {
    let mut w = World::new(7, CityData::default());
    for (x, y) in [(0, 0), (2_000_000, 0), (4_000_000, 0), (2_000_000, 2_000_000), (2_000_000, 4_000_000)] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    // Trunk A–B–C.
    for s in [0u32, 1, 2] {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    // Branch off B (trunk stop index 1): D then E.
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(3) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(4) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Has any vehicle reached within 120 m of point `(x,y)`?
fn any_vehicle_near(w: &World, x: i64, y: i64) -> bool {
    (0..w.vehicles.len()).any(|i| {
        (w.vehicles.x_mm[i] - x).abs() < 120_000 && (w.vehicles.y_mm[i] - y).abs() < 120_000
    })
}

#[test]
fn a_train_runs_the_branch_to_its_terminus() {
    // With two trains they split across the two service paths (round-robin), so one runs the branch
    // and must reach E at (2 km, 4 km) — a point NOT on the trunk. RED until P3: no train ever leaves
    // the trunk, so E is never reached.
    let mut w = y_line(2);
    assert_eq!(w.lines[0].branches.len(), 1, "the branch was recorded on the line");
    let mut reached_e = false;
    for _ in 0..3000 {
        w.tick(50);
        if any_vehicle_near(&w, 2_000_000, 4_000_000) {
            reached_e = true;
            break;
        }
    }
    assert!(reached_e, "a train must run the branch all the way to terminus E");
}

#[test]
fn branched_line_replays_bit_for_bit() {
    let mut a = y_line(3);
    let mut b = y_line(3);
    for _ in 0..1500 {
        a.tick(50);
    }
    for _ in 0..1500 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "same seed + log ⇒ identical hashed state");
}
