//! TTD L3 — the B/C "cliff" RED-first LIVENESS + IDENTITY gates (docs/ttd-l3-plan.md).
//!
//! Phase A (A0/A1/A2) is done: `TrackSegment` owns derived smoothed geometry and each `Path` carries a
//! derived `#[serde(skip)]` `segments` binding. The B/C unit (B2 meet re-key + B3 cross-line mutex/cap +
//! the `track_type`/`span_mode` ownership move + C1 hash flip) is irreducible — it MUST be shipped with
//! its liveness gates RED-FIRST, because **a gate-blind deadlock replays green** (AGENTS determinism
//! note): liveness can NEVER be inferred from replay-equality.
//!
//! STATUS (2026-06-17): these are GREEN at HEAD — the existing `derive_cross_blocks` union-find cap + the
//! `vehicle.rs` cross-line mutex already satisfy them — and proven RED when that cap is stubbed out (delete
//! `derive_cross_blocks` ⇒ 3 of these go red with cross-line head-ons). So they are the NON-VACUOUS gate the
//! eventual L3 B/C cliff's cap-REPLACEMENT must keep green; they are NOT evidence the cliff is done (B3's
//! segment-keyed mutex / PBS / segment-derived cyclic cap are NOT yet implemented). Landed now as liveness
//! REGRESSION guards so any future cliff attempt that drops the cap without a working replacement fails loudly.
//!
//! These three gates guard the exact properties the cliff must preserve while `derive_cross_blocks` (the
//! current cross-line cap) is eventually deleted and geometry/`track_type` move onto the segment:
//!   1. `shared_segment_meet_*`         — one physical single segment, opposing trains: no head-on AND
//!                                        cumulative ridership strictly rises (the B2 meet re-key).
//!   2. `over_provisioned_cyclic_ring_*`— a cyclic shared component, trains >> passing capacity: never
//!                                        deadlocks (the G2 cyclic-component capacity cap that must
//!                                        survive deleting `derive_cross_blocks`).
//!   3. `pbs_atomic_path_no_starvation_*`— a contended BIDIRECTIONAL multi-segment shared run: cumulative
//!                                        ridership strictly rises for BOTH lines (the G3 PBS atomic-path
//!                                        reservation + anti-starvation, not just mutual-exclusion).
//!
//! Crucially — UNLIKE the existing `single_track.rs`/`shared_rail.rs` never-freeze tests, which assert on
//! travelled DISTANCE over a demand-free grid — these gates assert on cumulative **ridership**. A consist
//! can shuttle back and forth (distance keeps rising) while every contended station STARVES; only a
//! ridership gate catches that. So each scenario lays a demand corridor along the shared run, and the
//! liveness assertion is "riders keep boarding," strictly stronger than "wheels keep turning."
//!
//! Modelled on `single_track.rs::{freezes,head_on,over_provisioned_single_track_never_freezes}` and
//! `shared_rail.rs::{cross_line_over_provisioned_never_freezes,cross_line_ring_never_deadlocks}`.
use sim::*;

const SINGLE: u8 = 1; // line::track::SINGLE
const CELL: i64 = 100_000; // grid cell (mm) — both lines snap to identical cells ⇒ identical edges

// ---------------------------------------------------------------------------------------------------
// Builders (grid + demand)
// ---------------------------------------------------------------------------------------------------

/// A demand corridor: an origin+dest cell at `(x, y)` for every `x` in `[0, span)` grid columns, on each
/// requested `y` row (mm). Without demand the ridership gate is vacuously RED forever, so every scenario
/// strings demand along the shared run AND along each line's private tails so per-line ridership develops.
fn corridor(span: i64, rows: &[i64]) -> Vec<DemandCell> {
    let mut cells = Vec::new();
    for k in 0..span {
        for &y in rows {
            cells.push(DemandCell {
                x_mm: k * CELL + 50_000,
                y_mm: y,
                origin_w: 4.0,
                dest_w: 4.0,
                commodity: 0,
            });
        }
    }
    cells
}

fn grid_demand_world(seed: u64, cells: Vec<DemandCell>) -> World {
    World::new(
        seed,
        CityData {
            grid_cell_mm: CELL,
            demand: DemandGrid { cell_m: 100.0, cells },
            ..Default::default()
        },
    )
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

// ---------------------------------------------------------------------------------------------------
// Cross-line head-on detector (shared physical segment, opposing dirs, within a consist-length).
// Mirrors shared_rail.rs::cross_line_headon — used for the SAFETY half of the meet gate.
// ---------------------------------------------------------------------------------------------------

/// Two consists of DIFFERENT lines, both strictly inside the shared single edge (x within `x_lo..x_hi`
/// on the x-axis trunk, |y| small), OPPOSING, within a consist-length: a cross-line head-on.
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
                if dx * dx + dy * dy < (len as i128) * (len as i128) {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------------
// Ridership probes — the LIVENESS signal (strictly stronger than travelled-distance).
// ---------------------------------------------------------------------------------------------------

/// Cumulative boardings credited to each line (sum over its stops; a never-dispatched/starved line
/// accrues nothing). The per-line vector is the anti-starvation signal for the PBS gate.
fn per_line_ridership(w: &World) -> Vec<f64> {
    w.stats_snapshot().per_line.iter().map(|l| l.ridership).collect()
}

/// Tick `n` steps. Returns the GLOBAL cumulative ridership at the end (monotone non-decreasing).
fn run(w: &mut World, n: usize) -> u64 {
    for _ in 0..n {
        w.tick(50);
    }
    w.ridership_total
}

// ===================================================================================================
// GATE 1 — shared single SEGMENT meet (B2): no head-on AND ridership strictly increases.
// ===================================================================================================

/// Two service paths sharing ONE physical single-track segment (the x-axis trunk A–B), each an
/// out-and-back to its own tail with passing places off-trunk. Demand strung along both lines.
fn two_lines_shared_single(trains: u16) -> World {
    // Demand on both lines' tails (y=0 trunk row + y=3cell second-line row) so ridership develops.
    let mut w = grid_demand_world(7, corridor(10, &[50_000, 3 * CELL + 50_000]));
    let a = place(&mut w, 3 * CELL + 50_000, 50_000); // cell (3,0)
    let b = place(&mut w, 6 * CELL + 50_000, 50_000); // cell (6,0)
    let s1 = place(&mut w, 50_000, 50_000); // (0,0)
    let e1 = place(&mut w, 9 * CELL + 50_000, 50_000); // (9,0)
    let s2 = place(&mut w, 50_000, 3 * CELL + 50_000); // (0,3)
    let e2 = place(&mut w, 9 * CELL + 50_000, 3 * CELL + 50_000); // (9,3)
    let l1 = make_line(&mut w, &[s1, a, b, e1]);
    let l2 = make_line(&mut w, &[s2, a, b, e2]);
    // A–B is span index 1 ([s,a,b,e]); single-track it on BOTH ⇒ ONE shared single physical segment.
    w.apply(&Command::SetSegmentTrack { line: l1, seg: TrackSegmentId(1), track: SINGLE });
    w.apply(&Command::SetSegmentTrack { line: l2, seg: TrackSegmentId(1), track: SINGLE });
    w.apply(&Command::AssignTrainset { line: l1, spec: 0, count: trains });
    w.apply(&Command::AssignTrainset { line: l2, spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn shared_segment_meet_no_headon_and_ridership_rises() {
    // The B2 meet re-key to `TrackSegmentId`: opposing trains of two lines on ONE physical single
    // segment must MEET (never both inside), and the meet must RESOLVE so riders keep boarding. The
    // distance-only never-freeze in shared_rail can't see a starving meet (a consist shuttling its own
    // half forever) — this asserts cumulative ridership STRICTLY increases across the run.
    let mut w = two_lines_shared_single(3);
    w.tick(50); // dispatch
    let nveh = w.vehicles.len();
    assert!(nveh >= 2, "need trains on both lines to contend (got {nveh})");
    assert!(
        w.vehicles.line.iter().any(|l| l.index() == 0) && w.vehicles.line.iter().any(|l| l.index() == 1),
        "both lines must dispatch onto the shared single segment",
    );
    let (x_lo, x_hi) = (3 * CELL + 60_000, 6 * CELL + 40_000); // strictly inside A–B

    // Warm so demand is captured + queues form, then snapshot ridership.
    for t in 0..2000 {
        w.tick(50);
        assert!(!cross_line_headon(&w, x_lo, x_hi), "cross-line head-on on the shared single segment, tick {t}");
    }
    let r0 = w.ridership_total;
    for t in 2000..8000 {
        w.tick(50);
        assert!(!cross_line_headon(&w, x_lo, x_hi), "cross-line head-on on the shared single segment, tick {t}");
    }
    let r1 = w.ridership_total;
    assert!(
        r1 > r0,
        "the shared single-segment meet must keep serving riders (no starving deadlock): ridership {r0} → {r1}",
    );
}

// ===================================================================================================
// GATE 2 — OVER-PROVISIONED cyclic ring (G2): trains >> passing capacity, never deadlocks.
// Guards the cyclic-component capacity cap that must survive deleting derive_cross_blocks.
// ===================================================================================================

/// Two single-track LOOP lines over the SAME physical square ring (a cyclic shared component), each
/// heavily over-provisioned (`trains` >> the 1-train ring capacity). Demand strung around the square so
/// the shuttle actually boards riders.
fn two_loops_shared_ring(trains: u16) -> World {
    // Demand around the square perimeter rows (y=0 and y=5cell) so the ring shuttle has riders.
    let mut w = grid_demand_world(13, corridor(6, &[50_000, 5 * CELL + 50_000]));
    let p0 = place(&mut w, 50_000, 50_000); // (0,0)
    let p1 = place(&mut w, 5 * CELL + 50_000, 50_000); // (5,0)
    let p2 = place(&mut w, 5 * CELL + 50_000, 5 * CELL + 50_000); // (5,5)
    let p3 = place(&mut w, 50_000, 5 * CELL + 50_000); // (0,5)
    let mk_loop = |w: &mut World| -> LineId {
        let li = LineId(w.lines.len() as u32);
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: true, mode: 0, literal: false });
        for &s in &[p0, p1, p2, p3] {
            w.apply(&Command::AddStop { line: li, station: s, after: None });
        }
        w.apply(&Command::SetSegmentTrack { line: li, seg: TrackSegmentId(u32::MAX), track: SINGLE });
        w.apply(&Command::AssignTrainset { line: li, spec: 0, count: trains });
        li
    };
    mk_loop(&mut w);
    mk_loop(&mut w);
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn over_provisioned_cyclic_ring_never_deadlocks() {
    // G2: the depth-1-forest liveness argument FAILS on a ring, so the cyclic-component cap clamps the
    // shared loop to a one-train shuttle however heavy the fleet. With `derive_cross_blocks` deleted in
    // B3 the cap must be re-derived from the segment graph's component structure — RED-first here with an
    // OVER-PROVISIONED ring (trains ≫ 1), not merely a meet. Assert: it derives cyclic, caps the fleet,
    // never deadlocks, and the shuttle keeps SERVING (ridership rises), not just turning wheels.
    let mut w = two_loops_shared_ring(8); // 8+8 trains thrown at a capacity-1 ring
    w.tick(50);
    assert!(
        w.cross_blocks.iter().any(|b| b.cyclic),
        "a shared single-track loop is a cyclic cross-line component (the cap must detect it)",
    );
    let nveh = w.vehicles.len();
    assert!(nveh >= 1, "the ring must dispatch at least its shuttle");
    assert!(
        nveh <= 4,
        "an over-provisioned single-track ring must be capped to a shuttle, not flooded (got {nveh})",
    );

    let r0 = run(&mut w, 3000); // warm + capture demand
    let r1 = run(&mut w, 4000);
    assert!(
        r1 > r0,
        "the over-provisioned cyclic ring must never deadlock — the shuttle keeps serving: ridership {r0} → {r1}",
    );
}

// ===================================================================================================
// GATE 3 — PBS atomic-path + ANTI-STARVATION (G3): a contended BIDIRECTIONAL multi-segment shared run;
// cumulative ridership strictly rises for BOTH lines (no starvation), not just mutual-exclusion.
// ===================================================================================================

/// Two out-and-back lines sharing a MULTI-SEGMENT single run (trunk cells C0..C3, i.e. ≥2 single spans
/// separated by a station gate), each with its OWN demand-bearing tail so per-line ridership is a clean
/// per-line starvation signal. The multi-segment atomic claim is where lowest-index `occ_claim` is
/// deadlock-free but NOT starvation-free (two trains each holding one segment of the other's path = a
/// classic standoff), so the PBS atomic-path reservation + aging tiebreak must be exercised.
fn two_lines_shared_multi_segment(trains: u16) -> World {
    // Demand on both lines' tails (y=0 and y=3cell rows) so each line earns distinguishable ridership.
    let mut w = grid_demand_world(29, corridor(10, &[50_000, 3 * CELL + 50_000]));
    // Trunk stations C0..C3 — three inter-stop spans, each single ⇒ a multi-segment shared run.
    let c0 = place(&mut w, 2 * CELL + 50_000, 50_000); // (2,0)
    let c1 = place(&mut w, 4 * CELL + 50_000, 50_000); // (4,0)
    let c2 = place(&mut w, 6 * CELL + 50_000, 50_000); // (6,0)
    let c3 = place(&mut w, 8 * CELL + 50_000, 50_000); // (8,0)
    // Private demand-bearing tails: line 1 below the trunk, line 2 above it.
    let s1 = place(&mut w, 50_000, 50_000); // (0,0)
    let e1 = place(&mut w, 11 * CELL + 50_000, 50_000); // (11,0)  — extend demand corridor below
    let s2 = place(&mut w, 50_000, 3 * CELL + 50_000); // (0,3)
    let e2 = place(&mut w, 11 * CELL + 50_000, 3 * CELL + 50_000); // (11,3)
    let l1 = make_line(&mut w, &[s1, c0, c1, c2, c3, e1]);
    let l2 = make_line(&mut w, &[s2, c0, c1, c2, c3, e2]);
    // Spans 1,2,3 ([s,c0,c1,c2,c3,e]) are the shared trunk: single-track ALL of them on BOTH lines.
    for l in [l1, l2] {
        for span in [1u32, 2, 3] {
            w.apply(&Command::SetSegmentTrack { line: l, seg: TrackSegmentId(span), track: SINGLE });
        }
        w.apply(&Command::AssignTrainset { line: l, spec: 0, count: trains });
    }
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn pbs_atomic_path_no_starvation_both_lines_served() {
    // G3 (PBS anti-starvation): the multi-segment atomic-path reservation is net-new liveness machinery.
    // Mutual-exclusion alone (lowest-index `occ_claim`) is deadlock-free but NOT starvation-free across a
    // multi-segment atomic claim — two trains each holding one segment of the other's path is a standoff,
    // or one line can monopolise the run and starve the other. So the gate is per-line: BOTH lines'
    // cumulative ridership must STRICTLY increase across the window (not just total, not just distance).
    let mut w = two_lines_shared_multi_segment(3);
    w.tick(50);
    assert!(
        w.vehicles.line.iter().any(|l| l.index() == 0) && w.vehicles.line.iter().any(|l| l.index() == 1),
        "both lines must dispatch onto the contended multi-segment run",
    );
    let (x_lo, x_hi) = (2 * CELL + 60_000, 8 * CELL + 40_000); // strictly inside the trunk C0..C3

    // Warm so both lines' queues form, then measure per-line ridership over a fair window.
    for t in 0..3000 {
        w.tick(50);
        assert!(!cross_line_headon(&w, x_lo, x_hi), "cross-line head-on on the multi-segment trunk, tick {t}");
    }
    let r0 = per_line_ridership(&w);
    for t in 3000..11000 {
        w.tick(50);
        assert!(!cross_line_headon(&w, x_lo, x_hi), "cross-line head-on on the multi-segment trunk, tick {t}");
    }
    let r1 = per_line_ridership(&w);
    assert_eq!(r0.len(), 2, "two lines");
    assert_eq!(r1.len(), 2, "two lines");
    for li in 0..2 {
        assert!(
            r1[li] > r0[li],
            "line {li} STARVED across the multi-segment atomic claim (anti-starvation gate): ridership {} → {} \
             (the other line {} → {})",
            r0[li],
            r1[li],
            r0[1 - li],
            r1[1 - li],
        );
    }
}

// ===================================================================================================
// IDENTITY scaffold — `positions_byte_identical(seedlog)`: the implementer's tool to PROVE the
// geometry-ownership move (A2→C1) shifts ONLY serialization, never the integrator's `s_mm`/ridership.
// ===================================================================================================

/// A replayable scenario: a seed + a command log + a tick schedule. `replay`s the commands the same way
/// the frontend does (the only write path), then ticks the fixed schedule. The IDENTITY gate runs this
/// pre/post the geometry-ownership flip and asserts every hashed motion field is bit-identical.
pub struct SeedLog {
    pub seed: u64,
    pub city: CityData,
    pub commands: Vec<Command>,
    /// Number of fixed 50ms ticks to advance after replaying the log.
    pub ticks: usize,
}

/// The bit-identity oracle for the C1 geometry-ownership move. Replays `seedlog` to a `World` and returns
/// the tuple the move must NOT perturb: the integrator's per-vehicle arc-length `s_mm`, the vehicle
/// `(line,path,dir)` keys, AND cumulative ridership. C1 moves geometry onto `TrackSegment` and re-pins the
/// goldens *because serialization changed* — but `advance()`'s integrator is untouched (the plan's
/// migration-(a) invariant), so this tuple is INVARIANT. The implementer captures it on the A2 HEAD, lands
/// the flip, captures it again, and asserts equality: a single helper that pins "only serialization moved."
///
/// Returned (not asserted in isolation) so the implementer can diff pre/post across two builds of the crate
/// — there is no second geometry source to compare against within ONE build. The companion
/// `assert_positions_byte_identical` asserts equality of two captures from the SAME build (the
/// determinism-style twice-in-one-process check, already meaningful today).
pub fn positions_byte_identical(seedlog: &SeedLog) -> (Vec<i64>, Vec<(u32, u8, i8)>, u64) {
    let mut w = World::new(seedlog.seed, seedlog.city.clone());
    for cmd in &seedlog.commands {
        w.apply(cmd);
    }
    for _ in 0..seedlog.ticks {
        w.tick(50);
    }
    let s_mm = w.vehicles.s_mm.clone();
    let keys: Vec<(u32, u8, i8)> = (0..w.vehicles.len())
        .map(|i| (w.vehicles.line[i].0, w.vehicles.path[i], w.vehicles.dir[i]))
        .collect();
    (s_mm, keys, w.ridership_total)
}

/// Assert two captures of the SAME `seedlog` are byte-identical (vehicle `s_mm` + keys + ridership). The
/// implementer uses this across the geometry-ownership flip; today it doubles as a determinism check that
/// the capture itself is stable twice-in-one-process.
pub fn assert_positions_byte_identical(seedlog: &SeedLog) {
    let a = positions_byte_identical(seedlog);
    let b = positions_byte_identical(seedlog);
    assert_eq!(a.0, b.0, "vehicle s_mm must be bit-identical (only serialization may move at C1)");
    assert_eq!(a.1, b.1, "vehicle (line,path,dir) keys must be bit-identical");
    assert_eq!(a.2, b.2, "cumulative ridership must be bit-identical");
}

/// A representative shared-single-segment `SeedLog` for the identity gate (the same contended geometry the
/// liveness gates use — exactly the case the C1 flip is riskiest for).
fn identity_seedlog() -> SeedLog {
    let city = CityData {
        grid_cell_mm: CELL,
        demand: DemandGrid { cell_m: 100.0, cells: corridor(10, &[50_000, 3 * CELL + 50_000]) },
        ..Default::default()
    };
    // Re-derive the same station/line/track command log two_lines_shared_single emits, captured as data.
    let mut log: Vec<Command> = Vec::new();
    let st = |x: i64, y: i64| Command::PlaceStation { x_mm: x, y_mm: y, name: None };
    log.push(st(3 * CELL + 50_000, 50_000)); // a = 0
    log.push(st(6 * CELL + 50_000, 50_000)); // b = 1
    log.push(st(50_000, 50_000)); // s1 = 2
    log.push(st(9 * CELL + 50_000, 50_000)); // e1 = 3
    log.push(st(50_000, 3 * CELL + 50_000)); // s2 = 4
    log.push(st(9 * CELL + 50_000, 3 * CELL + 50_000)); // e2 = 5
    log.push(Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in [2u32, 0, 1, 3] {
        log.push(Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    log.push(Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in [4u32, 0, 1, 5] {
        log.push(Command::AddStop { line: LineId(1), station: StationId(s), after: None });
    }
    log.push(Command::SetSegmentTrack { line: LineId(0), seg: TrackSegmentId(1), track: SINGLE });
    log.push(Command::SetSegmentTrack { line: LineId(1), seg: TrackSegmentId(1), track: SINGLE });
    log.push(Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    log.push(Command::AssignTrainset { line: LineId(1), spec: 0, count: 3 });
    log.push(Command::SetRunning { running: true });
    SeedLog { seed: 7, city, commands: log, ticks: 4000 }
}

#[test]
fn positions_byte_identical_scaffold_is_stable_today() {
    // The identity scaffold is meaningful at HEAD: capturing the same seed+log twice in one process MUST
    // yield bit-identical motion (the determinism contract, restated through the geometry-ownership lens).
    // After the C1 flip the implementer captures pre/post across two crate builds and asserts the SAME
    // tuple is unchanged — proving "only serialization moved," the whole justification for the re-pin.
    let sl = identity_seedlog();
    assert_positions_byte_identical(&sl);
    // Sanity: the scenario actually moves trains + serves riders, so the gate isn't vacuous.
    let (s_mm, keys, ridership) = positions_byte_identical(&sl);
    assert!(!s_mm.is_empty() && !keys.is_empty(), "the identity scenario must dispatch vehicles");
    assert!(ridership > 0, "the identity scenario must serve riders (a non-vacuous identity fixture)");
}
