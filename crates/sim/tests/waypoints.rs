//! Freeform waypoints: control points that BEND a line's track between its stops without being
//! stations. They shape the curve (and so its length/cost/speed) but are never halts.
use sim::*;

fn base() -> World {
    World::new(7, CityData { id: "t".into(), seed: 7, demand: DemandGrid { cell_m: 200.0, cells: vec![] }, ..Default::default() })
}

/// A straight 2-stop line along y=0 from (0,0) to (10 km, 0).
fn straight_line() -> World {
    let mut w = base();
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 10_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 0x0072b2, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w
}

#[test]
fn a_waypoint_bends_the_track_without_adding_a_stop() {
    let mut w = straight_line();
    let straight_len = w.lines[0].length_mm();
    let straight_stops = w.lines[0].paths[0].stop_arclen_mm.len();

    // One control point 3 km north of the midpoint, in the single span (after stop 0).
    let ev = w.apply(&Command::SetLineWaypoints { line: LineId(0), waypoints: vec![vec![[5_000_000, 3_000_000]]] });
    assert!(matches!(ev.as_slice(), [Event::WaypointsSet { .. }]), "got {ev:?}");

    // The polyline now bows north, and is longer than the straight chord…
    let max_y = w.lines[0].paths[0].polyline.iter().map(|p| p.y_mm).max().unwrap();
    assert!(max_y > 1_000_000, "track bends toward the waypoint (max_y = {max_y})");
    assert!(w.lines[0].length_mm() > straight_len, "the detour is longer than the straight line");
    // …but the waypoint is NOT a stop — still exactly two halts.
    assert_eq!(w.lines[0].paths[0].stop_arclen_mm.len(), straight_stops, "waypoints never add halts");
    assert_eq!(w.lines[0].stops.len(), 2);
}

#[test]
fn bending_the_track_raises_its_capital_cost() {
    let mut w = straight_line();
    let straight_cost = w.lines[0].capital_cost;
    w.apply(&Command::SetLineWaypoints { line: LineId(0), waypoints: vec![vec![[5_000_000, 6_000_000]]] });
    assert!(w.lines[0].capital_cost > straight_cost, "a longer (bent) line costs more to build");
}

#[test]
fn clearing_waypoints_straightens_the_line_again() {
    let mut w = straight_line();
    let straight_len = w.lines[0].length_mm();
    w.apply(&Command::SetLineWaypoints { line: LineId(0), waypoints: vec![vec![[5_000_000, 4_000_000]]] });
    assert!(w.lines[0].length_mm() > straight_len);
    w.apply(&Command::SetLineWaypoints { line: LineId(0), waypoints: vec![] });
    assert_eq!(w.lines[0].length_mm(), straight_len, "an empty waypoint list restores the straight track");
}

#[test]
fn set_waypoints_on_an_unknown_line_is_rejected() {
    let mut w = straight_line();
    let ev = w.apply(&Command::SetLineWaypoints { line: LineId(9), waypoints: vec![] });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]));
}

#[test]
fn waypoints_keep_the_sim_deterministic() {
    let run = || {
        let mut w = straight_line();
        w.apply(&Command::SetLineWaypoints { line: LineId(0), waypoints: vec![vec![[4_000_000, 2_000_000], [6_000_000, -2_000_000]]] });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 180_000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..3000 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "a line with waypoints replays bit-for-bit");
}

#[test]
fn waypoints_round_trip_through_the_command_json() {
    let c = Command::SetLineWaypoints { line: LineId(2), waypoints: vec![vec![[1, 2], [3, 4]], vec![]] };
    let json = serde_json::to_string(&c).expect("serialize");
    let back: Command = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(c, back, "SetLineWaypoints round-trips through the JSON wire");
}
