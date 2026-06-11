//! P1 capacity — block following + train length (see docs/capacity-roadmap.md).
//!
//! A line's trains respect a block: they keep a braking-distance + train-length gap, so an
//! over-provisioned line is capped at its physical block density (the throughput ceiling) rather
//! than the arbitrary `MAX_TRAINS_PER_LINE`, and no two trains overlap. Written RED-first: these
//! fail against today's independent pass-through motion and pass once the movement-authority layer
//! lands. Tested through Commands + observable vehicle positions, never by poking internals.
use sim::*;

/// A short out-and-back line (2 km each way ⇒ 4 km round trip) crammed with `count` trains.
fn crammed_line(count: u16) -> World {
    let mut w = World::new(7, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None }); // 2 km E
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Monotone round-trip arc-length `p` and the circuit length `round` — the sim's follow coordinate,
/// mirrored here so the test reads positions the same way the authority layer reasons about them.
fn p_and_round(w: &World, i: usize) -> (i64, i64) {
    let li = w.vehicles.line[i].index();
    let total = w.lines[li].length_mm();
    let s = w.vehicles.s_mm[i];
    if w.lines[li].loop_line {
        (s, total)
    } else if w.vehicles.dir[i] > 0 {
        (s, 2 * total)
    } else {
        (2 * total - s, 2 * total)
    }
}

fn line0_indices(w: &World) -> Vec<usize> {
    (0..w.vehicles.len()).filter(|&i| w.vehicles.line[i].index() == 0).collect()
}

/// Minimum center-to-center spacing (in `p`) between consecutive trains on line 0.
fn min_spacing(w: &World) -> i64 {
    let idx = line0_indices(w);
    let mut round = 0;
    let mut ps: Vec<i64> = idx
        .iter()
        .map(|&i| {
            let (p, r) = p_and_round(w, i);
            round = r;
            p
        })
        .collect();
    ps.sort_unstable();
    let n = ps.len();
    let mut min = i64::MAX;
    for k in 0..n {
        let gap = if k + 1 < n { ps[k + 1] - ps[k] } else { ps[0] + round - ps[n - 1] };
        min = min.min(gap);
    }
    min
}

#[test]
fn over_provisioned_line_is_capped_at_block_density() {
    // 24 trains on a 4 km round-trip line cannot all run while keeping a braking-distance + length
    // block — the physical block density caps the running fleet well below the requested 24.
    // RED until P1: today dispatch runs all 24 and they pass through each other.
    let mut w = crammed_line(24);
    w.tick(50); // dispatch
    for _ in 0..40 {
        w.tick(50);
    }
    let running = line0_indices(&w).len();
    assert!(running < 24, "block density should cap the fleet below the requested 24 (got {running})");
}

#[test]
fn consecutive_trains_keep_a_block_gap() {
    // No two trains overlap or tailgate inside the block. The tightest legitimate spacing is a
    // follower pulled right up behind a STOPPED leader: head-to-tail = BLOCK_MARGIN (60 m), so
    // center-to-center ≥ leader length (140 m) + margin = ~200 m. Assert comfortably under that
    // floor but far above today's behaviour. RED until P1: trains overlap (min spacing 0 m — they
    // pass through each other at the terminus).
    let mut w = crammed_line(24);
    w.tick(50);
    for _ in 0..40 {
        w.tick(50);
    }
    let min = min_spacing(&w);
    assert!(min >= 180_000, "consecutive trains must keep a block gap (min spacing {min} mm)");
}

#[test]
fn block_limited_line_replays_bit_for_bit() {
    // The capacity layer stays deterministic: same seed + same command log ⇒ identical state.
    let mut a = crammed_line(24);
    let mut b = crammed_line(24);
    for _ in 0..300 {
        a.tick(50);
    }
    for _ in 0..300 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "same seed + log ⇒ identical hashed state");
}
