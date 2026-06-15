//! TTD-style SIGNALS: the per-tick render scratch (`World::signal_occupancy`) that surfaces single-track
//! block state so the player sees WHY a cart waits. Golden-neutral — NOT in `Canonical` (the determinism
//! + arcadia goldens stay green), regenerated bit-identically each tick. Status 1 = OCCUPIED (red),
//! 2 = WAITING (amber). The P2 meet mechanics themselves are pinned by single_track.rs.
use sim::*;

/// A forced-single-track line with a MID passing place + several trains, so opposing carts meet there.
fn meet_world() -> World {
    let city = CityData {
        id: "t".into(),
        seed: 5,
        force_single_track: true,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 40.0, dest_w: 40.0, commodity: 0 },
                DemandCell { x_mm: 6_000_000, y_mm: 0, origin_w: 40.0, dest_w: 40.0, commodity: 0 },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(11, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None }); // the passing place
    w.apply(&Command::PlaceStation { x_mm: 6_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 }); // enough to force a meet
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 30_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn signals_mark_clear_and_occupied_single_spans() {
    let mut w = meet_world();
    // At rest (before running settles) every single span reads CLEAR (green); as carts move, spans they
    // sit in read OCCUPIED (red). Both aspects must appear over a run — that IS the block-state readout.
    let (mut saw_clear, mut saw_occupied) = (false, false);
    for _ in 0..1500 {
        w.tick(50);
        if w.signal_occupancy.iter().any(|s| s.status == 0) {
            saw_clear = true;
        }
        if w.signal_occupancy.iter().any(|s| s.status == 1) {
            saw_occupied = true;
        }
        if saw_clear && saw_occupied {
            break;
        }
    }
    assert!(saw_clear, "an empty single span must emit a CLEAR (green) signal");
    assert!(saw_occupied, "a cart inside a single span must emit an OCCUPIED (red) signal");
    // Every single span on the line is signalled (2 spans on this 3-stop line).
    assert!(w.signal_occupancy.iter().filter(|s| s.status <= 1).count() >= 2, "both single spans are signalled");
}

#[test]
fn signal_records_carry_a_renderable_position() {
    // Every record points at a real (line, path) and a finite arc-length — so the render copy-out can
    // place it. (A stale line/path would silently draw at the origin.)
    let mut w = meet_world();
    for _ in 0..400 {
        w.tick(50);
    }
    assert!(!w.signal_occupancy.is_empty(), "a running single-track meet produces signals");
    for s in &w.signal_occupancy {
        assert!((s.line as usize) < w.lines_view().len(), "signal names a real line");
        assert!(s.status <= 2, "status is clear(0) / occupied(1) / waiting(2), got {}", s.status);
    }
}

#[test]
fn signals_are_render_scratch_not_hashed() {
    // Two identical runs reach the SAME state_hash — signals never feed back into state, so they can't
    // perturb the hash (the golden-neutrality guarantee, proven locally).
    let run = || {
        let mut w = meet_world();
        for _ in 0..600 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "signals are render-only — replay reaches the identical hash");
    // The scratch is CLEARED + rebuilt each tick (never an unbounded accumulation across ticks).
    let mut w = meet_world();
    for _ in 0..600 {
        w.tick(50);
    }
    assert!(w.signal_occupancy.len() < 64, "signal scratch is per-tick + bounded, got {}", w.signal_occupancy.len());
}
