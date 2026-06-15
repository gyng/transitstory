//! Fantasy/design: `CityData.force_single_track` makes EVERY span SINGLE — one rail reads cleaner and forces
//! opposing trains to MEET at passing places (so signalling matters). Baked true for arcadia; the default
//! (false) keeps the shipped Double track, byte-identical (proven by the determinism/arcadia golden tests).
//! The P2 meet/cost mechanics are in single_track.rs; here we prove the FLAG enforces SINGLE everywhere.
use sim::line::track;
use sim::*;

fn build_line(force_single: bool) -> World {
    let city = CityData {
        id: "t".into(),
        seed: 1,
        force_single_track: force_single,
        demand: DemandGrid { cell_m: 500.0, cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 10.0, dest_w: 10.0, commodity: 0 }] },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    w
}

#[test]
fn force_single_track_makes_every_span_single() {
    let single = build_line(true).lines_view()[0].track_types.clone();
    assert!(!single.is_empty(), "the line has spans");
    assert!(single.iter().all(|&t| t == track::SINGLE), "every span is SINGLE under the flag: {single:?}");
    let normal = build_line(false).lines_view()[0].track_types.clone();
    assert!(normal.iter().all(|&t| t == track::DOUBLE), "default keeps DOUBLE: {normal:?}");
}

#[test]
fn forced_single_overrides_a_set_segment_double() {
    // Even an explicit SetSegmentTrack(Double) is overridden back to SINGLE — you cannot double-track.
    let mut w = build_line(true);
    w.apply(&Command::SetSegmentTrack { line: LineId(0), span: 0, track: track::DOUBLE });
    let tt = w.lines_view()[0].track_types.clone();
    assert!(tt.iter().all(|&t| t == track::SINGLE), "the flag wins over a Double command: {tt:?}");
}

#[test]
fn forced_single_track_is_cheaper_than_default_double() {
    let single = build_line(true).lines[0].capital_cost;
    let double = build_line(false).lines[0].capital_cost;
    assert!(single < double, "single track (55%) is cheaper than double ({single} vs {double})");
}

#[test]
fn forced_single_track_replays_deterministically() {
    let run = || {
        let city = CityData { id: "t".into(), seed: 3, force_single_track: true, demand: DemandGrid { cell_m: 500.0, cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 40.0, dest_w: 40.0, commodity: 0 }, DemandCell { x_mm: 5_000_000, y_mm: 0, origin_w: 40.0, dest_w: 40.0, commodity: 0 }] }, ..Default::default() };
        let mut w = World::new(9, city);
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..600 { w.tick(50); }
        w.state_hash()
    };
    assert_eq!(run(), run(), "a single-track realm replays bit-for-bit");
}
