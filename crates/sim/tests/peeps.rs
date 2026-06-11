//! "Peeps" (individual rider dots) are a purely RENDER-DERIVED read-out: `render_buf::fill_peeps`
//! reads the un-hashed in-transit passenger set, and the walk-out breadcrumb written in
//! `board_alight` (recent_alight) is excluded from `Canonical`. So the whole feature must be
//! determinism-free: it neither reads nor writes any state the commit gate hashes.
use sim::*;

fn city() -> CityData {
    // Demand strung along the corridor so the stations capture trips.
    let cells = (0..20).map(|k| DemandCell { x_mm: 300_000 * k, y_mm: 0, origin_w: 4.0, dest_w: 4.0 }).collect();
    CityData {
        id: "t".into(),
        seed: 7,
        demand: DemandGrid { cell_m: 300.0, cells },
        ..Default::default()
    }
}

fn serviced_world() -> World {
    let mut w = World::new(7, city());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 4_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 0x00ccff, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..3 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 200_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn peeps_readout_is_determinism_free() {
    let mut w = serviced_world();
    for _ in 0..4000 {
        w.tick(50); // ~200 s — riders spawn, board, ride, alight (recent_alight breadcrumbs accrue)
    }
    assert!(w.stats_snapshot().ridership_total > 0.0, "scenario must actually move riders");

    // Reading the peep buffer must NOT change the hashed state in any way.
    let before = w.state_hash();
    let (xy, col, cit) = render_buf::fill_peeps(&w, 0.5, 30.0);
    let after = w.state_hash();
    assert_eq!(before, after, "fill_peeps is a pure read — it must not perturb state_hash");

    // Buffer is well-formed: paired x/y and rgba, capped, with one citizen id per peep.
    assert_eq!(xy.len() % 2, 0, "positions are interleaved [x,y,...]");
    assert_eq!(col.len() % 4, 0, "colours are RGBA");
    assert_eq!(xy.len() / 2, col.len() / 4, "one colour per peep");
    assert_eq!(cit.len(), xy.len() / 2, "one citizen id per peep (index-aligned for click-to-inspect)");
    assert!(xy.len() / 2 <= render_buf::MAX_VISIBLE_PEEPS, "peep count is capped");
    assert!(xy.len() > 0, "a serviced, demand-fed network has in-transit peeps to draw");

    // All peep coordinates are finite (no NaN/inf leaking from interpolation/jitter).
    assert!(xy.iter().all(|v| v.is_finite()), "peep positions are finite");
}

#[test]
fn peep_breadcrumb_does_not_break_replay() {
    // The recent_alight write in board_alight runs on the hashed tick path, but is excluded from
    // Canonical — so two identical replays (which both write it) still produce the identical hash,
    // AND interleaving peep reads during one of them changes nothing.
    let mut a = serviced_world();
    let mut b = serviced_world();
    let mut saw_breadcrumb = false;
    for _ in 0..5000 {
        // ~250 s: avg journey is ~150 s, so completions (and thus walk-out breadcrumbs) accrue.
        a.tick(50);
        let _ = render_buf::fill_peeps(&a, 0.3, 30.0); // peep readout every tick on `a` only
        saw_breadcrumb |= !a.recent_alight.is_empty(); // age-pruned to a 6 s window, so check live
        b.tick(50); // `b` never reads peeps
    }
    assert_eq!(a.state_hash(), b.state_hash(), "peep reads + breadcrumbs are determinism-free");
    assert!(saw_breadcrumb, "the walk-out breadcrumb buffer is actually exercised during the run");
}
