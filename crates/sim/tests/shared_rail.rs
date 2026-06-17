//! Phase 2 — cross-line shared physical rail (shared-rail.md). On a GRID, two DISTINCT lines over the
//! same single-track edge must MEET (one consist on the edge at a time), not pass through each other.
//! Today every occupancy key leads with the line id (seg_key `line<<40`, junc_key `line<<32`), so two
//! lines on one physical edge get distinct keys and pass clean through — written RED-first here; the
//! cross-line `edge_key` mutex (Phase A.1.7 + B.6) closes it. Liveness is proven by never-freeze, never
//! by replay-equality (a deterministic cross-line deadlock replays green).
use sim::*;

const SINGLE: u8 = 1;

fn grid_world(cell_mm: i64, seed: u64) -> World {
    World::new(seed, CityData { grid_cell_mm: cell_mm, ..Default::default() })
}
fn place(w: &mut World, x: i64, y: i64) -> StationId {
    let id = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
    id
}
fn make_line(w: &mut World, stops: &[StationId]) -> LineId {
    let li = LineId(w.lines.len() as u32);
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for &s in stops {
        w.apply(&Command::AddStop { line: li, station: s, after: None });
    }
    li
}

/// Two consists of DIFFERENT lines, both on the shared single edge (x within the A–B span on the
/// x-axis trunk, |y| small), in OPPOSING directions and within a consist-length: a cross-line head-on.
fn cross_line_headon(w: &World, x_lo: i64, x_hi: i64) -> bool {
    let on: Vec<usize> = (0..w.vehicles.len())
        .filter(|&i| {
            w.vehicles.y_mm[i].abs() < 60_000 && w.vehicles.x_mm[i] > x_lo && w.vehicles.x_mm[i] < x_hi
        })
        .collect();
    for a in 0..on.len() {
        for b in (a + 1)..on.len() {
            let (i, j) = (on[a], on[b]);
            if w.vehicles.line[i] != w.vehicles.line[j] && w.vehicles.dir[i] != w.vehicles.dir[j] {
                let len = w.lines[w.vehicles.line[i].index()].vehicle_spec().length_mm;
                let dx = (w.vehicles.x_mm[i] - w.vehicles.x_mm[j]) as i128;
                let dy = (w.vehicles.y_mm[i] - w.vehicles.y_mm[j]) as i128;
                if ((dx * dx + dy * dy) as f64).sqrt() < len as f64 {
                    return true;
                }
            }
        }
    }
    false
}

/// Two distinct grid lines sharing a single-track section A–B (the same consecutive stop-cells), each
/// an out-and-back with passing places on its own side. The shared A–B span is single on BOTH lines.
fn two_lines_shared_single(trains: u16) -> World {
    let cell = 100_000i64;
    let mut w = grid_world(cell, 7);
    // Shared trunk stations on the x-axis (so both lines snap to the SAME cells ⇒ identical edges).
    let a = place(&mut w, 3 * cell + 50_000, 50_000); // cell (3,0)
    let b = place(&mut w, 6 * cell + 50_000, 50_000); // cell (6,0)
    // Line 1 ends below-left / above-right; line 2 the mirror — distinct circuits, shared A–B.
    let s1 = place(&mut w, 50_000, 50_000); // (0,0)
    let e1 = place(&mut w, 9 * cell + 50_000, 50_000); // (9,0)
    let s2 = place(&mut w, 50_000, 3 * cell + 50_000); // (0,3)
    let e2 = place(&mut w, 9 * cell + 50_000, 3 * cell + 50_000); // (9,3)
    let l1 = make_line(&mut w, &[s1, a, b, e1]);
    let l2 = make_line(&mut w, &[s2, a, b, e2]);
    // A–B is span index 1 on each line ([s,a,b,e]); single-track it on BOTH ⇒ a shared single edge run.
    w.apply(&Command::SetSegmentTrack { line: l1, seg: TrackSegmentId(1), track: SINGLE });
    w.apply(&Command::SetSegmentTrack { line: l2, seg: TrackSegmentId(1), track: SINGLE });
    w.apply(&Command::AssignTrainset { line: l1, spec: 0, count: trains });
    w.apply(&Command::AssignTrainset { line: l2, spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn cross_blocks_derive_one_shared_section() {
    // Step 1 (the derivation): the shared single A–B run is ONE cross-line block with both lines as
    // lanes, line-independent, acyclic. Inert (derived, not yet used for movement).
    let mut w = two_lines_shared_single(2);
    w.tick(50); // dispatch derives world.cross_blocks
    assert_eq!(w.cross_blocks.len(), 1, "the shared A–B single section is exactly one cross-line block");
    let blk = &w.cross_blocks[0];
    let lanes: std::collections::BTreeSet<u32> = blk.by_lane.iter().map(|&(l, _, _, _)| l).collect();
    assert_eq!(lanes, std::collections::BTreeSet::from([0, 1]), "both lines are lanes of the block");
    assert!(!blk.cyclic, "a linear shared section is acyclic");
    for &(_, _, lo, hi) in &blk.by_lane {
        assert!(hi > lo, "each lane's window covers the shared run");
    }
}

#[test]
fn cross_blocks_are_command_order_independent() {
    // The block set (ids + lanes) is a pure function of geometry/topology, not of which line was
    // created first — load-bearing for determinism + the future block-id ordering.
    let snap = |w: &World| -> Vec<(bool, u32, Vec<u32>)> {
        let mut v: Vec<(bool, u32, Vec<u32>)> = w
            .cross_blocks
            .iter()
            .map(|b| {
                let mut lanes: Vec<u32> = b.by_lane.iter().map(|&(l, _, _, _)| l).collect();
                lanes.sort_unstable();
                lanes.dedup();
                (b.cyclic, b.passing_places, lanes)
            })
            .collect();
        v.sort();
        v
    };
    let mut a = two_lines_shared_single(2);
    a.tick(50);
    // Build B with the two lines created in the OPPOSITE order (same geometry).
    let cell = 100_000i64;
    let mut b = grid_world(cell, 7);
    let sa = place(&mut b, 3 * cell + 50_000, 50_000);
    let sb = place(&mut b, 6 * cell + 50_000, 50_000);
    let s1 = place(&mut b, 50_000, 50_000);
    let e1 = place(&mut b, 9 * cell + 50_000, 50_000);
    let s2 = place(&mut b, 50_000, 3 * cell + 50_000);
    let e2 = place(&mut b, 9 * cell + 50_000, 3 * cell + 50_000);
    let l2 = make_line(&mut b, &[s2, sa, sb, e2]); // line 2 FIRST
    let l1 = make_line(&mut b, &[s1, sa, sb, e1]);
    b.apply(&Command::SetSegmentTrack { line: l1, seg: TrackSegmentId(1), track: SINGLE });
    b.apply(&Command::SetSegmentTrack { line: l2, seg: TrackSegmentId(1), track: SINGLE });
    b.apply(&Command::AssignTrainset { line: l1, spec: 0, count: 2 });
    b.apply(&Command::AssignTrainset { line: l2, spec: 0, count: 2 });
    b.apply(&Command::SetRunning { running: true });
    b.tick(50);
    assert_eq!(snap(&a), snap(&b), "the cross-line block set is command-order-independent");
}

#[test]
fn cross_line_single_section_meet_no_headon() {
    // SAFETY: never two opposing consists of different lines on the shared single A–B section. (RED
    // before Phase 2: line-scoped keys ⇒ they pass through each other, tick 37.) LIVENESS: every
    // dispatched train keeps moving (the meet resolves, no freeze) — proven here, NEVER inferred from
    // the determinism gate. NO-STARVATION: BOTH lines are served (the cross-line cap is round-robin).
    let mut w = two_lines_shared_single(3);
    w.tick(50);
    let nveh = w.vehicles.len();
    assert!(nveh >= 2, "need trains on both lines to contend");
    assert!(w.vehicles.line.iter().any(|l| l.index() == 0) && w.vehicles.line.iter().any(|l| l.index() == 1),
        "both lines must be served (cross-line cap must not starve a line)");
    let (x_lo, x_hi) = (3 * 100_000 + 60_000, 6 * 100_000 + 40_000); // strictly inside A–B
    let mut last = w.vehicles.s_mm.clone();
    let mut traveled = vec![0i64; nveh];
    for t in 0..8000 {
        w.tick(50);
        assert!(!cross_line_headon(&w, x_lo, x_hi), "cross-line head-on on the shared single edge, tick {t}");
        for i in 0..nveh {
            traveled[i] += (w.vehicles.s_mm[i] - last[i]).abs();
            last[i] = w.vehicles.s_mm[i];
        }
    }
    let total = w.lines[0].length_mm();
    assert!(*traveled.iter().min().unwrap() > total, "a consist froze at the cross-line meet");
}

/// Warm up then return the min steady-state travel (a frozen/deadlocked consist accrues ~0).
fn steady_min_travel(w: &mut World, warmup: usize, window: usize) -> i64 {
    w.tick(50);
    for _ in 0..warmup {
        w.tick(50);
    }
    let n = w.vehicles.len();
    let mut last = w.vehicles.s_mm.clone();
    let mut tr = vec![0i64; n];
    for _ in 0..window {
        w.tick(50);
        for i in 0..n {
            tr[i] += (w.vehicles.s_mm[i] - last[i]).abs();
            last[i] = w.vehicles.s_mm[i];
        }
    }
    tr.into_iter().min().unwrap_or(0)
}

#[test]
fn cross_line_short_passing_place_coalesces_no_deadlock() {
    // Cross-line review bug 1: two lines share block1 + a SHORT (sub-consist) double + block2. The short
    // double must NOT count as a passing place (else the 2nd admitted consist breaks the depth-1 forest
    // ⇒ a multi-block cross-line deadlock). The fix coalesces all shared edges into ONE block. Trunk
    // cells 2,3,4,5 ; spans 1 & 3 SINGLE, span 2 a 1-cell DOUBLE (< consist).
    let cell = 100_000i64;
    let mut w = grid_world(cell, 21);
    let c0 = place(&mut w, 2 * cell + 50_000, 50_000);
    let c1 = place(&mut w, 3 * cell + 50_000, 50_000);
    let c2 = place(&mut w, 4 * cell + 50_000, 50_000);
    let c3 = place(&mut w, 5 * cell + 50_000, 50_000);
    let a0 = place(&mut w, 50_000, 50_000);
    let a1 = place(&mut w, 8 * cell + 50_000, 50_000);
    let b0 = place(&mut w, 50_000, 3 * cell + 50_000);
    let b1 = place(&mut w, 8 * cell + 50_000, 3 * cell + 50_000);
    let l1 = make_line(&mut w, &[a0, c0, c1, c2, c3, a1]);
    let l2 = make_line(&mut w, &[b0, c0, c1, c2, c3, b1]);
    for l in [l1, l2] {
        w.apply(&Command::SetSegmentTrack { line: l, seg: TrackSegmentId(1), track: SINGLE });
        w.apply(&Command::SetSegmentTrack { line: l, seg: TrackSegmentId(3), track: SINGLE });
        w.apply(&Command::AssignTrainset { line: l, spec: 0, count: 6 });
    }
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert_eq!(w.cross_blocks.len(), 1, "a sub-consist double must not split the cross-line block");
    let total = w.lines[0].length_mm();
    assert!(steady_min_travel(&mut w, 6000, 6000) > total, "coalesced short-passing-place section froze");
}

#[test]
fn cross_line_single_block_with_passing_place_no_overadmit() {
    // Cross-line review bug 2: two out-and-back lines share a single block (D–A) with a passing place
    // (P–D double). `passing_places + 2` round-robin'd one line 2 trains that met head-on INSIDE. The
    // fix caps 1 train per line ⇒ no same-line meet inside; both lines run, the cross-line meet resolves.
    let cell = 100_000i64;
    let mut w = grid_world(cell, 23);
    let p = place(&mut w, 2 * cell + 50_000, 50_000); // (2,0)
    let d = place(&mut w, 4 * cell + 50_000, 50_000); // (4,0)
    let a = place(&mut w, 7 * cell + 50_000, 50_000); // (7,0)
    let s0 = place(&mut w, 50_000, 50_000);
    let e0 = place(&mut w, 9 * cell + 50_000, 50_000);
    let s1 = place(&mut w, 50_000, 4 * cell + 50_000);
    let e1 = place(&mut w, 9 * cell + 50_000, 4 * cell + 50_000);
    let l1 = make_line(&mut w, &[s0, p, d, a, e0]);
    let l2 = make_line(&mut w, &[s1, p, d, a, e1]);
    for l in [l1, l2] {
        w.apply(&Command::SetSegmentTrack { line: l, seg: TrackSegmentId(2), track: SINGLE }); // D–A single (the block)
        w.apply(&Command::AssignTrainset { line: l, spec: 0, count: 4 });
    }
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert!(w.vehicles.line.iter().any(|l| l.index() == 0) && w.vehicles.line.iter().any(|l| l.index() == 1),
        "both lines served");
    let total = w.lines[0].length_mm();
    assert!(steady_min_travel(&mut w, 6000, 6000) > total, "single block over-admitted ⇒ head-on inside ⇒ froze");
}

#[test]
fn cross_line_over_provisioned_never_freezes() {
    // Throw a heavy fleet at the shared single section: the cross-line cap bounds the COMBINED fleet so
    // the capacity-1 block can't be over-provisioned into a (gate-blind) deadlock. Both lines stay
    // served and every dispatched consist keeps moving.
    let mut w = two_lines_shared_single(16);
    w.tick(50);
    assert!(w.vehicles.line.iter().any(|l| l.index() == 0) && w.vehicles.line.iter().any(|l| l.index() == 1),
        "both lines served under over-provision");
    let (x_lo, x_hi) = (3 * 100_000 + 60_000, 6 * 100_000 + 40_000);
    let nveh = w.vehicles.len();
    let mut last = w.vehicles.s_mm.clone();
    let mut traveled = vec![0i64; nveh];
    for t in 0..8000 {
        w.tick(50);
        assert!(!cross_line_headon(&w, x_lo, x_hi), "head-on under over-provision, tick {t}");
        for i in 0..nveh {
            traveled[i] += (w.vehicles.s_mm[i] - last[i]).abs();
            last[i] = w.vehicles.s_mm[i];
        }
    }
    let total = w.lines[0].length_mm();
    assert!(*traveled.iter().min().unwrap() > total, "a consist froze under cross-line over-provision");
}

#[test]
fn cross_line_ring_never_deadlocks() {
    // A CYCLIC shared component (two LOOP lines over the same single-track square): the depth-1-forest
    // liveness argument fails on a ring, so the cross-line cap clamps a cyclic block to ONE train total
    // (a single-track ring is a one-train shuttle). Assert it derives cyclic, caps to a shuttle, and
    // the dispatched consist keeps moving (no opposing train ⇒ no deadlock).
    let cell = 100_000i64;
    let mut w = grid_world(cell, 13);
    // A square loop: 4 corner stations both lines run as a loop.
    let p0 = place(&mut w, 50_000, 50_000); // (0,0)
    let p1 = place(&mut w, 5 * cell + 50_000, 50_000); // (5,0)
    let p2 = place(&mut w, 5 * cell + 50_000, 5 * cell + 50_000); // (5,5)
    let p3 = place(&mut w, 50_000, 5 * cell + 50_000); // (0,5)
    let mk_loop = |w: &mut World| -> LineId {
        let li = LineId(w.lines.len() as u32);
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: true, mode: 0, literal: false });
        for &s in &[p0, p1, p2, p3] {
            w.apply(&Command::AddStop { line: li, station: s, after: None });
        }
        w.apply(&Command::SetSegmentTrack { line: li, seg: TrackSegmentId(u32::MAX), track: SINGLE });
        w.apply(&Command::AssignTrainset { line: li, spec: 0, count: 3 });
        li
    };
    mk_loop(&mut w);
    mk_loop(&mut w);
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert!(w.cross_blocks.iter().any(|b| b.cyclic), "a shared loop is a cyclic cross-line block");
    let nveh = w.vehicles.len();
    assert_eq!(nveh, 1, "a single-track shared ring is a ONE-train shuttle (capacity 1)");
    let start = w.vehicles.s_mm.clone();
    for _ in 0..4000 {
        w.tick(50);
    }
    assert!(w.vehicles.s_mm[0] != start[0], "the ring shuttle keeps moving (no cyclic deadlock)");
}
