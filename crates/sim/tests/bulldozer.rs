//! Bulldozer (#5): RemoveStation / RemoveLine tombstone the entity (its id/slot is never
//! reused — determinism) and neutralise it everywhere — despawned vehicles, dropped stops,
//! freed catchment, excluded from counts/cost. Plus the cost-preview query (#2-$) must equal a
//! committed line's track cost. The determinism gate must stay green WITH removes in the log.
use sim::*;

fn line_world() -> World {
    let mut w = World::new(7, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 6_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    for s in [0, 1, 2] {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 300_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn remove_line_despawns_vehicles_and_drops_from_views() {
    let mut w = line_world();
    w.tick(50); // dispatch
    assert!(w.vehicles.len() >= 1, "vehicles dispatched");
    assert_eq!(w.stats_snapshot().line_count, 1);

    w.apply(&Command::RemoveLine { line: LineId(0) });
    w.tick(50); // dispatch rebuild despawns the removed line's vehicles
    assert_eq!(w.vehicles.len(), 0, "removed line's vehicles are despawned");
    let st = w.stats_snapshot();
    assert_eq!(st.line_count, 0, "removed line not counted");
    assert!(st.per_line.is_empty(), "removed line absent from per-line stats");
    assert!(w.lines_view()[0].removed, "line view flags removed (id slot kept)");
}

#[test]
fn remove_line_is_idempotent_and_rejects_unknown() {
    let mut w = line_world();
    let ev = w.apply(&Command::RemoveLine { line: LineId(0) });
    assert!(matches!(ev.as_slice(), [Event::LineRemoved { .. }]));
    let ev2 = w.apply(&Command::RemoveLine { line: LineId(0) }); // already removed
    assert!(matches!(ev2.as_slice(), [Event::Rejected { .. }]));
    let ev3 = w.apply(&Command::RemoveLine { line: LineId(99) }); // unknown
    assert!(matches!(ev3.as_slice(), [Event::Rejected { .. }]));
}

#[test]
fn remove_station_drops_it_from_lines_and_keeps_id_slots() {
    let mut w = line_world();
    assert_eq!(w.lines_view()[0].stops, vec![0, 1, 2]);

    w.apply(&Command::RemoveStation { station: StationId(1) });

    // The line skips the bulldozed middle stop; remaining ids are UNSHIFTED (0 and 2).
    assert_eq!(w.lines_view()[0].stops, vec![0, 2], "stop dropped, ids unshifted");
    assert!(w.stations_view()[1].removed, "station 1 tombstoned");
    assert!(!w.stations_view()[0].removed && !w.stations_view()[2].removed);
    assert_eq!(w.stats_snapshot().station_count, 2, "removed station not counted");
    // Station 2 is still id 2 (slot preserved) — re-adding it to a line still resolves.
    assert_eq!(w.stations_view()[2].id, 2);
}

#[test]
fn remove_station_frees_its_catchment() {
    // One demand cell near station 1; with 1 present it captures origin weight, gone it doesn't.
    let cells = vec![DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 5.0, dest_w: 5.0 }];
    let city = CityData { id: "t".into(), seed: 1, demand: DemandGrid { cell_m: 300.0, cells }, ..Default::default() };
    let mut w = World::new(1, city);
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None }); // on the cell
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50); // demand prepare captures
    let before: f32 = w.captured_origin.iter().sum();
    assert!(before > 0.0, "station captures the cell's origin demand");

    w.apply(&Command::RemoveStation { station: StationId(0) });
    w.tick(50); // demand_dirty ⇒ recompute capture, skipping the removed station
    let after: f32 = w.captured_origin.iter().sum();
    assert_eq!(after, 0.0, "a bulldozed station captures nothing");
}

#[test]
fn preview_line_cost_matches_a_committed_trainless_line() {
    let mut w = World::new(3, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    // Commit a line through both stops with NO trainset → capital_cost is track-only.
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    let committed = w.lines[0].capital_cost;
    assert!(committed > 0, "track costs capital");

    let preview = w.preview_line_cost(&[0, 1], 0, false);
    assert_eq!(preview, committed, "preview uses the same core cost formula (track only)");

    // Heavy rail (mode 4) is pricier than metro (mode 0) for the same route.
    assert!(w.preview_line_cost(&[0, 1], 4, false) > w.preview_line_cost(&[0, 1], 0, false));
    // Fewer than two valid stops ⇒ free.
    assert_eq!(w.preview_line_cost(&[0], 0, false), 0);
}

#[test]
fn determinism_holds_with_removes_in_the_log() {
    // A command log that builds, removes a station + a line, and ticks through it. Replaying
    // the identical log must reach an identical state_hash (the determinism gate, with removes).
    fn run() -> u64 {
        let mut w = World::new(42, CityData::default());
        for k in 0..5 {
            w.apply(&Command::PlaceStation { x_mm: 1_500_000 * k, y_mm: 0, name: None });
        }
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
        for s in [0, 1, 2, 3, 4] {
            w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
        }
        w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 1 });
        for s in [0, 2, 4] {
            w.apply(&Command::AddStop { line: LineId(1), station: StationId(s), after: None });
        }
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
        w.apply(&Command::AssignTrainset { line: LineId(1), spec: 0, count: 2 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..400 {
            w.tick(50);
        }
        w.apply(&Command::RemoveStation { station: StationId(2) }); // used by both lines
        w.apply(&Command::RemoveLine { line: LineId(1) });
        for _ in 0..400 {
            w.tick(50);
        }
        w.state_hash()
    }
    assert_eq!(run(), run(), "same seed + same log (incl. removes) ⇒ identical state_hash");
}
