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
    w.apply(&Command::SetSegmentTrack { line: l1, span: 1, track: SINGLE });
    w.apply(&Command::SetSegmentTrack { line: l2, span: 1, track: SINGLE });
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
    b.apply(&Command::SetSegmentTrack { line: l1, span: 1, track: SINGLE });
    b.apply(&Command::SetSegmentTrack { line: l2, span: 1, track: SINGLE });
    b.apply(&Command::AssignTrainset { line: l1, spec: 0, count: 2 });
    b.apply(&Command::AssignTrainset { line: l2, spec: 0, count: 2 });
    b.apply(&Command::SetRunning { running: true });
    b.tick(50);
    assert_eq!(snap(&a), snap(&b), "the cross-line block set is command-order-independent");
}

// Phase-2 RED-first target (`#[ignore]`d until the cross-line `edge_key` mutex + liveness stack lands,
// shared-rail.md Step 2). Confirmed RED today: cross-line head-on on the shared single edge at tick 37
// (two lines pass clean through each other — line-scoped keys). Un-ignore when Phase 2 ships.
#[test]
#[ignore = "Phase 2: needs the cross-line edge_key mutex + liveness stack (shared-rail.md Step 2)"]
fn cross_line_single_section_meet_no_headon() {
    // SAFETY: never two opposing consists of different lines on the shared single A–B section. RED
    // today (line-scoped keys ⇒ they pass through each other). LIVENESS: every dispatched train keeps
    // moving (the meet resolves, no freeze) — proven here, not inferred from the determinism gate.
    let mut w = two_lines_shared_single(3);
    w.tick(50);
    let nveh = w.vehicles.len();
    assert!(nveh >= 2, "need trains on both lines to contend");
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
