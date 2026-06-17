//! TTD L5b — SAME-DIRECTION sub-block following on a SIGNALLED single span (docs/ttd-l5-plan.md, the
//! CORRECTED MODEL). A player signal subdivides a single span into sub-blocks; a same-direction follower
//! may enter the span behind a same-direction leader PROVIDED its target sub-block is free (the leader is
//! in a sub-block ahead). OPPOSING exclusion stays WHOLE-SPAN (a signal is NOT an opposing passing point —
//! that is a loop). These gates are RED-FIRST: a gate-blind deadlock replays green, so liveness is asserted
//! on a DEMAND corridor (ridership rises), never inferred from replay. The no-signals neutrality (goldens +
//! K=1/K=2 fingerprints byte-identical) is pinned by determinism.rs/arcadia.rs/position_fingerprint.rs.
//!
//! SCENARIO (the deadlock-safe demonstration): a line that is MOSTLY DOUBLE with ONE long SINGLE span in
//! the middle. The double sections are abundant passing places, so opposing meets always resolve and the
//! line never over-provisions into an opposing deadlock — the single span is the SOLE bottleneck. Without
//! a signal it holds ONE consist at a time (any direction): same-direction followers queue at its gate.
//! WITH signals it holds one consist per SUB-BLOCK in a given direction, so a same-direction CONVOY transits
//! it — the win is at the SAME train count (the dispatch cap is unchanged), purely from the move-phase
//! relaxation. (An out-and-back line CANNOT raise its train count via signals — its binding constraint is
//! the opposing meet, which a signal does not relax; over-admitting deadlocks. So the cap stays `doubles+1`
//! and the throughput gain is tighter same-direction spacing, not more trains. See the plan's CORRECTED MODEL.)
use sim::*;

const SINGLE: u8 = 1; // line::track::SINGLE
const CELL: i64 = 100_000;

/// A demand corridor: an origin+dest cell at each grid column in `[0, span)` on the trunk row. Without
/// demand the ridership gate is vacuously RED, so the scenarios string demand along the whole run.
fn corridor(span: i64) -> Vec<DemandCell> {
    (0..span).map(|k| DemandCell { x_mm: k * CELL + 50_000, y_mm: 50_000, origin_w: 8.0, dest_w: 8.0, commodity: 0 }).collect()
}

fn grid_demand_world(seed: u64, cells: Vec<DemandCell>) -> World {
    World::new(seed, CityData { grid_cell_mm: CELL, demand: DemandGrid { cell_m: 100.0, cells }, ..Default::default() })
}

/// A 7-stop out-and-back line, all spans DOUBLE except span 3 (the long single span between stops at
/// x=9 and x=25). `sigs` evenly-spaced signals subdivide span 3. The double spans flanking it are the
/// passing places, so the train count caps at `doubles + 1 = 6` regardless of signals (a signal does not
/// raise opposing capacity) and never deadlocks. `trains` is clamped by that cap on dispatch.
const SINGLE_SPAN: usize = 3;
fn mostly_double_one_single(sigs: u32, trains: u16) -> World {
    let mut w = grid_demand_world(7, corridor(60));
    let xs = [0i64, 3, 6, 9, 25, 28, 31].map(|c| c * CELL + 50_000); // span 3 = [9,25], a long single run
    for x in xs {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 50_000, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..7u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::SetSegmentTrack { line: LineId(0), seg: TrackSegmentId(SINGLE_SPAN as u32), track: SINGLE });
    let lo = w.lines[0].paths[0].stop_arclen_mm[SINGLE_SPAN];
    let hi = w.lines[0].paths[0].stop_arclen_mm[SINGLE_SPAN + 1];
    for g in 0..sigs {
        let at = lo + (hi - lo) * (g as i64 + 1) / (sigs as i64 + 1);
        w.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span: SINGLE_SPAN as u32, at_mm: at });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Count of consists strictly inside the named single span (the throughput probe).
fn occupants_in_single_span(w: &World) -> usize {
    let p = &w.lines[0].paths[0];
    let lo = p.stop_arclen_mm[SINGLE_SPAN];
    let hi = p.stop_arclen_mm[SINGLE_SPAN + 1];
    (0..w.vehicles.len()).filter(|&i| w.vehicles.s_mm[i] > lo && w.vehicles.s_mm[i] < hi).count()
}

/// Head-on detector on the SINGLE span: two OPPOSING consists strictly inside it (same idiom as
/// single_track.rs::head_on). The relaxation must NEVER admit this (opposing exclusion stays whole-span).
fn head_on_in_single_span(w: &World) -> bool {
    let p = &w.lines[0].paths[0];
    let lo = p.stop_arclen_mm[SINGLE_SPAN];
    let hi = p.stop_arclen_mm[SINGLE_SPAN + 1];
    let dirs: Vec<i8> = (0..w.vehicles.len())
        .filter(|&i| w.vehicles.s_mm[i] > lo && w.vehicles.s_mm[i] < hi)
        .map(|i| w.vehicles.dir[i])
        .collect();
    dirs.iter().any(|&d| d > 0) && dirs.iter().any(|&d| d < 0)
}

// ===================================================================================================
// GATE 1 — signals RAISE same-direction single-track throughput (RED before the impl).
// ===================================================================================================

#[test]
fn signals_raise_same_direction_single_track_throughput() {
    // WITHOUT a signal: the single span is one block ⇒ at most ONE consist inside it at a time; a
    // same-direction follower waits a whole span behind the leader. WITH signals: sub-block following
    // lets a same-direction CONVOY occupy the span at once ⇒ strictly more concurrent occupancy AND
    // strictly higher ridership — at the SAME train count (the dispatch cap is unchanged by signals).
    let measure = |sigs: u32| -> (usize, u64, usize) {
        let mut w = mostly_double_one_single(sigs, 8);
        w.tick(50); // dispatch
        let dispatched = w.vehicles.len();
        let mut max_occ = 0usize;
        for _ in 0..10_000 {
            w.tick(50);
            assert!(!head_on_in_single_span(&w), "no opposing head-on may occur inside the single span");
            max_occ = max_occ.max(occupants_in_single_span(&w));
        }
        (max_occ, w.ridership_total, dispatched)
    };
    let (occ0, ride0, disp0) = measure(0); // no signal — one-consist-per-span bottleneck
    let (occ3, ride3, disp3) = measure(3); // three signals — four sub-blocks
    assert_eq!(disp0, disp3, "the train COUNT must be identical — signals raise throughput, not the fleet (cap unchanged): {disp0} vs {disp3}");
    assert!(
        occ3 > occ0,
        "signals must let MORE same-direction consists occupy the single span concurrently: {occ0} (no signal) vs {occ3} (3 signals)",
    );
    assert!(
        ride3 > ride0,
        "sub-block following must raise throughput ⇒ strictly higher ridership at the SAME fleet: {ride0} (no signal) vs {ride3} (3 signals)",
    );
}

// ===================================================================================================
// GATE 2 — NO head-on with signals. Opposing trains on a signalled single span: NEVER two OPPOSING
// consists inside the span. Proves the relaxation never admits opposing (a signal is not a loop).
// ===================================================================================================

#[test]
fn no_head_on_with_signals() {
    // Opposing trains must still MEET at a station passing place (a double span), never inside the
    // signalled single span (the whole-span opposing exclusion survives sub-block keying).
    let mut w = mostly_double_one_single(2, 6);
    w.tick(50);
    let nveh = w.vehicles.len();
    assert!(nveh >= 2, "need >=2 trains to meet (got {nveh})");
    for t in 0..8000 {
        w.tick(50);
        assert!(!head_on_in_single_span(&w), "head-on on a SIGNALLED single span at tick {t} — a signal must not admit opposing");
    }
}

// ===================================================================================================
// GATE 3 — a signalled single line NEVER freezes (over-provisioned; depth-1-forest no-rest at gates).
// ===================================================================================================

#[test]
fn signalled_single_track_never_freezes() {
    // Throw far more trains than the line can hold; the dispatch cap clamps the fleet AND the depth-1-
    // forest no-rest holds across signal gates (a denied follower rests AT a signal gate owning nothing)
    // ⇒ never deadlocks ⇒ ridership keeps rising.
    let mut w = mostly_double_one_single(3, 20);
    w.tick(50);
    let mut r = [0u64; 2];
    for _ in 0..4000 {
        w.tick(50);
        assert!(!head_on_in_single_span(&w), "never a head-on, even over-provisioned");
    }
    r[0] = w.ridership_total;
    for _ in 0..4000 {
        w.tick(50);
        assert!(!head_on_in_single_span(&w), "never a head-on, even over-provisioned");
    }
    r[1] = w.ridership_total;
    assert!(
        r[1] > r[0],
        "an over-provisioned SIGNALLED single line must never freeze — riders keep boarding: {} -> {}",
        r[0],
        r[1],
    );
}

// ===================================================================================================
// Deterministic replay of a signal-bearing MOTION log (the relaxation is bit-for-bit reproducible).
// ===================================================================================================

#[test]
fn signalled_following_replays_bit_for_bit() {
    let run = || -> u64 {
        let mut w = mostly_double_one_single(3, 8);
        for _ in 0..3000 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "the signal sub-block following motion replays bit-for-bit");
}
