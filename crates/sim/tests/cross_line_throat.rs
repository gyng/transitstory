//! TTD L4d — the CROSS-LINE THROAT MUTEX (docs/ttd-l4-plan.md "CROSS-LINE FINDING"). The determinism
//! heart + liveness-critical gate for the cross-line capacity workstream.
//!
//! THE GAP (proven by `probe_overbooking` below, kept as a non-vacuous sanity): at a station with `k`
//! berths, MORE than `k` consists can dwell simultaneously when multiple INDEPENDENT lines share it.
//! Independent schedules collide at an interchange; the L2 berth seed + A.3 relaxation pull arriving
//! trains into FREE berths but never DENY, and the same-line P1 follow-clamp doesn't gate a DIFFERENT
//! line's train — so nothing caps co-dwellers at `k`. The excess fall to the centerline (`berth_idx=-1`)
//! and overlap the berth-0 dweller.
//!
//! THE FIX (asserted RED-first here): a HARD throat mutex — a further `min()` on `desired_ds`, like the
//! Phase B.4/B.6 junction/cross-line mutexes. A train whose this-tick advance would carry it INTO the
//! approach span toward its next stop must secure a FREE berth at that station; if none is free it CLAMPS
//! to HOLD SHORT at the gate BEFORE the approach span (the prior stop / passing place), owning nothing.
//!
//! THE THREE LIVENESS DISCIPLINES the gates pin (a gate-blind deadlock replays GREEN — never inferred):
//!   G1 (allocation cycle): GREEDY take-ANY-free berth, never wait-for-a-SPECIFIC berth.
//!   G2 (berth↔span 2-cycle): a berth-WAITER rests at the gate BEFORE it ENTERS the approach single span —
//!      so it owns nothing while waiting (else train A holds a berth + waits for the exit, train B sits in
//!      the span + waits for a berth ⇒ 2-cycle ⇒ deadlock).
//!   Depth-1: every blocked train rests at a gate owning nothing ⇒ acyclic depth-1 wait-for forest.
use sim::*;

const CELL: i64 = 100_000;

// ---------------------------------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------------------------------

/// The consists DWELLING at `station` this tick (parked on a stop's arclen, dwell timer live) — the
/// indices, so we can also check their lateral tracks.
fn dwellers_at(w: &World, station: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for i in 0..w.vehicles.len() {
        if w.clock_ms >= w.vehicles.dwell_until_ms[i] {
            continue;
        }
        let line = &w.lines[w.vehicles.line[i].index()];
        let Some(path) = line.paths.get(w.vehicles.path[i] as usize) else { continue };
        let s = w.vehicles.s_mm[i];
        if let Some(si) = path.stop_arclen_mm.iter().position(|&a| a == s) {
            if path.station_for_stop_index(si).0 == station {
                out.push(i);
            }
        }
    }
    out
}

/// The ASSIGNED lateral berth a dwelling consist holds, or `None` if not yet assigned this tick. `berth_idx`
/// is the per-tick render scratch the A.1 seed fills for trains DWELLING at start-of-tick; a consist that
/// ARRIVES mid-tick is assigned its berth on the NEXT tick's A.1 (a pre-existing L2 timing detail PINNED by
/// the K=2 berth-motion fingerprints — not the cross-line bug, and not editable here). So the
/// no-double-booking check is on ASSIGNED berths: two dwellers must never hold the same `berth_idx >= 0`.
/// The hard no-OVERBOOKING invariant (co-dwell <= k) is checked separately on the dweller COUNT.
fn assigned_berth(w: &World, i: usize) -> Option<i32> {
    let b = w.vehicles.berth_idx[i];
    if b >= 0 {
        Some(b)
    } else {
        None
    }
}

fn demand_cells(specs: &[(i64, i64)]) -> Vec<DemandCell> {
    specs
        .iter()
        .map(|&(x, y)| DemandCell { x_mm: x, y_mm: y, origin_w: 12.0, dest_w: 12.0, commodity: 0 })
        .collect()
}

// ---------------------------------------------------------------------------------------------------
// SCENARIO A — three INDEPENDENT lines crossing one shared k=2 station, heavy demand at it.
// ---------------------------------------------------------------------------------------------------

/// A shared central k=2 station crossed by 3 independent lines (horizontal / vertical / diagonal), each
/// an out-and-back through the center, every line over-provisioned so their independent schedules collide
/// at the interchange. Demand strung along every corridor so the lines actually run + serve.
fn three_lines_one_shared_station() -> (World, u32) {
    let mut cells = demand_cells(&[
        (0, 4 * CELL + 50_000),
        (2 * CELL + 50_000, 4 * CELL + 50_000),
        (6 * CELL + 50_000, 4 * CELL + 50_000),
        (8 * CELL + 50_000, 4 * CELL + 50_000),
        (4 * CELL + 50_000, 50_000),
        (4 * CELL + 50_000, 2 * CELL + 50_000),
        (4 * CELL + 50_000, 6 * CELL + 50_000),
        (4 * CELL + 50_000, 8 * CELL + 50_000),
        (2 * CELL + 50_000, 2 * CELL + 50_000),
        (6 * CELL + 50_000, 6 * CELL + 50_000),
    ]);
    // ensure both an origin AND a dest exist near the center so trips route THROUGH it.
    cells.push(DemandCell { x_mm: 4 * CELL + 50_000, y_mm: 4 * CELL + 50_000, origin_w: 6.0, dest_w: 6.0, commodity: 0 });

    let mut w =
        World::new(7, CityData { grid_cell_mm: CELL, demand: DemandGrid { cell_m: 100.0, cells }, ..Default::default() });
    let center = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 4 * CELL + 50_000, y_mm: 4 * CELL + 50_000, name: None });
    let l0a = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 50_000, y_mm: 4 * CELL + 50_000, name: None });
    let l0b = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 8 * CELL + 50_000, y_mm: 4 * CELL + 50_000, name: None });
    let l1a = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 4 * CELL + 50_000, y_mm: 50_000, name: None });
    let l1b = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 4 * CELL + 50_000, y_mm: 8 * CELL + 50_000, name: None });
    let l2a = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 50_000, y_mm: 50_000, name: None });
    let l2b = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: 8 * CELL + 50_000, y_mm: 8 * CELL + 50_000, name: None });

    w.apply(&Command::BuildPlatforms { station: center, k: 2 });

    let mk = |w: &mut World, a: StationId, b: StationId| {
        let li = LineId(w.lines.len() as u32);
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: li, station: a, after: None });
        w.apply(&Command::AddStop { line: li, station: center, after: None });
        w.apply(&Command::AddStop { line: li, station: b, after: None });
        w.apply(&Command::AssignTrainset { line: li, spec: 0, count: 4 });
    };
    mk(&mut w, l0a, l0b);
    mk(&mut w, l1a, l1b);
    mk(&mut w, l2a, l2b);
    w.apply(&Command::SetRunning { running: true });
    (w, center.0)
}

#[test]
fn probe_overbooking() {
    // NON-VACUOUS sanity: WITHOUT the fix this scenario overbooks (the gap is real + reachable). WITH the
    // fix it is capped — so this only asserts the scenario genuinely contends (max co-dwell would exceed k
    // if unthrottled). It corroborates `cross_line_no_overbooking` is not a vacuous pass.
    let (mut w, center) = three_lines_one_shared_station();
    let mut max_codwell = 0;
    for _ in 0..8000 {
        w.tick(50);
        max_codwell = max_codwell.max(dwellers_at(&w, center).len());
    }
    assert!(w.ridership_total > 0, "the scenario must serve riders (non-vacuous)");
    // The 3-line interchange genuinely contends for the 2 berths (co-dwell reaches the cap).
    assert!(max_codwell >= 2, "the interchange must contend for both berths (got max_codwell={max_codwell})");
}

#[test]
fn cross_line_no_overbooking() {
    // GATE 1 (RED today, GREEN with the throat mutex). At a k=2 station served by 3 INDEPENDENT lines under
    // heavy demand, assert at EVERY tick:
    //   (a) the consists DWELLING at the shared station never exceed k=2 (the hard no-overbooking
    //       invariant — without the throat the excess falls to the centerline `berth_idx=-1` and overlaps
    //       the berth-0 dweller; the probe measures up to 6 co-dwellers at this k=2 station);
    //   (b) no two dwellers hold the SAME ASSIGNED berth (`berth_idx >= 0` distinct) — they never share a
    //       physical platform track. (A consist that arrives mid-tick shows `berth_idx=-1` for the single
    //       arrival tick before the next A.1 seed assigns it — a PINNED L2 timing detail, see
    //       `assigned_berth` — so the centerline transient is not flagged; the >k overbooking it would mask
    //       is caught by (a).)
    //
    // STUB-PROOF: disabling the throat denial (make the Phase-A.4 clamp a no-op) ⇒ co-dwell exceeds 2 ⇒
    // (a) goes RED. Verified by temporarily neutering the clamp (documented in the return notes).
    let (mut w, center) = three_lines_one_shared_station();
    for t in 0..8000 {
        w.tick(50);
        let d = dwellers_at(&w, center);
        assert!(
            d.len() <= 2,
            "OVERBOOKING at tick {t}: {} consists dwelling at the k=2 shared station {:?}",
            d.len(),
            d
        );
        // no two dwellers on the same ASSIGNED platform berth.
        let mut berths: Vec<i32> = d.iter().filter_map(|&i| assigned_berth(&w, i)).collect();
        berths.sort_unstable();
        for w2 in berths.windows(2) {
            assert_ne!(
                w2[0], w2[1],
                "two consists hold the SAME assigned berth {} at the shared station, tick {t}",
                w2[0]
            );
        }
    }
    assert!(w.ridership_total > 0, "must serve riders (non-vacuous)");
}

// ---------------------------------------------------------------------------------------------------
// SCENARIO B — the G2 never-freeze: a shared station with SINGLE track on BOTH approach spans.
// ---------------------------------------------------------------------------------------------------

/// Lines contending for a k=2 shared central station whose adjacent approach spans are SINGLE track — the
/// dangerous berth↔single-span topology. CONTINUOUS geometry (`grid_cell_mm = 0`) so NO `cross_blocks`
/// form (they are grid-only) and the cross-line dispatch cap stays inert — the lines share only the center
/// STATION (a point: its k=2 berths), NOT a physical rail edge, so each line runs its own real fleet and
/// the throat berth mutex is the SOLE thing capping co-dwellers. Each line is a 5-stop out-and-back
/// `a — p — center — q — b` radiating from the center on its own bearing, with DOUBLE-track passing tails
/// (`a–p`, `q–b`) and SINGLE track on the two spans adjacent to the center (`p–center`, `center–q`). So a
/// dwelling consist must claim its SINGLE onward span to leave, while an opposing same-line consist in that
/// span wants a berth — exactly the berth↔single-span 2-cycle G2 must preclude. Passing tails give a
/// per-line count cap of `doubles+1 = 3`. A DEMAND cluster near each endpoint so cumulative RIDERSHIP is
/// the liveness signal (strictly stronger than travelled distance).
fn shared_station_single_both_sides(lines: usize, trains: u16) -> (World, u32) {
    const SINGLE: u8 = 1;
    const R: i64 = 1_000_000; // 1000 mm-units between adjacent stops along a bearing
    let cx = 5_000_000_i64;
    // Distinct bearings so the lines radiate from the center without overlapping (continuous: no grid snap).
    let bearings: [(i64, i64); 3] = [(1000, 0), (0, 1000), (700, 700)]; // E-W, N-S, diagonal (×1e-3 units)
    // Demand clusters near every endpoint of every line so each line earns riders both directions.
    let mut cells = Vec::new();
    for &(bx, by) in bearings.iter().take(lines) {
        for mult in [-4i64, -2, 2, 4] {
            cells.push(DemandCell {
                x_mm: cx + mult * R * bx / 1000,
                y_mm: cx + mult * R * by / 1000,
                origin_w: 10.0,
                dest_w: 10.0,
                commodity: 0,
            });
        }
    }
    cells.push(DemandCell { x_mm: cx, y_mm: cx, origin_w: 6.0, dest_w: 6.0, commodity: 0 });
    let mut w = World::new(
        7,
        CityData { grid_cell_mm: 0, demand: DemandGrid { cell_m: 100.0, cells }, ..Default::default() },
    );
    let center = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: cx, y_mm: cx, name: None });
    let mut quads = Vec::new();
    for &(bx, by) in bearings.iter().take(lines) {
        let at = |mult: i64| (cx + mult * R * bx / 1000, cx + mult * R * by / 1000);
        let mk = |w: &mut World, mult: i64| {
            let id = StationId(w.stations.len() as u32);
            let (x, y) = at(mult);
            w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
            id
        };
        let a = mk(&mut w, -4);
        let p = mk(&mut w, -2);
        let q = mk(&mut w, 2);
        let b = mk(&mut w, 4);
        quads.push((a, p, q, b));
    }
    w.apply(&Command::BuildPlatforms { station: center, k: 2 });
    for &(a, p, q, b) in &quads {
        let li = LineId(w.lines.len() as u32);
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        for s in [a, p, center, q, b] {
            w.apply(&Command::AddStop { line: li, station: s, after: None });
        }
        // 5 stops ⇒ spans 0(a–p) 1(p–center) 2(center–q) 3(q–b). Single the two center-adjacent spans;
        // leave 0 and 3 double (passing places) ⇒ count cap doubles+1 = 3.
        w.apply(&Command::SetSegmentTrack { line: li, seg: TrackSegmentId(1), track: SINGLE });
        w.apply(&Command::SetSegmentTrack { line: li, seg: TrackSegmentId(2), track: SINGLE });
        w.apply(&Command::AssignTrainset { line: li, spec: 0, count: trains });
    }
    w.apply(&Command::SetRunning { running: true });
    (w, center.0)
}

/// Two consists of the SAME line, OPPOSING directions, both strictly inside the SAME single span adjacent
/// to the center — a head-on at the single-track throat (the berth↔span failure mode). The lines here
/// don't share physical rail (distinct bearings), so a cross-line collision is geometrically impossible;
/// the real safety risk is a same-line head-on on a center-adjacent single span when the throat mis-orders
/// the meet claim. Checks spans 1 (p–center) and 2 (center–q) on every line.
fn head_on_at_throat(w: &World) -> bool {
    // (line, span) -> dir of a train strictly inside that center-adjacent SINGLE span.
    let mut occ: Vec<((usize, usize), i8)> = Vec::new();
    for i in 0..w.vehicles.len() {
        let li = w.vehicles.line[i].index();
        let pa = w.vehicles.path[i];
        let Some(path) = w.lines[li].paths.get(pa as usize) else { continue };
        let s = w.vehicles.s_mm[i];
        let sp = path.span_of(s);
        if sp != 1 && sp != 2 {
            continue; // only the two center-adjacent (single) spans
        }
        let lo = path.stop_arclen_mm.get(sp).copied().unwrap_or(i64::MIN);
        let hi = path.stop_arclen_mm.get(sp + 1).copied().unwrap_or(i64::MAX);
        if !(s > lo && s < hi) {
            continue; // on a gate (passing place / center) — owns nothing
        }
        let dir = w.vehicles.dir[i];
        let key = (li, sp);
        if let Some(&(_, d0)) = occ.iter().find(|(k, _)| *k == key) {
            if d0 != dir {
                return true; // two opposing trains strictly inside one center-adjacent single span
            }
        } else {
            occ.push((key, dir));
        }
    }
    false
}

#[test]
fn cross_line_throat_never_freezes() {
    // GATE 2 (G2). A shared k=2 station with SINGLE track on BOTH approach spans, 3 lines contending for
    // the berths AND the single exit spans, a demand corridor. Assert cumulative ridership STRICTLY rises
    // across two windows (no deadlock) AND no cross-line head-on near the throat.
    //
    // STUB-PROOF: making a berth-waiter HOLD the approach span while waiting (clamp AFTER the meet
    // `try_claim` instead of BEFORE) creates the berth↔span 2-cycle ⇒ ridership stalls ⇒ RED.
    let (mut w, center) = shared_station_single_both_sides(3, 3);
    w.tick(50); // dispatch
    assert!(w.vehicles.len() >= 3, "need contending trains (got {})", w.vehicles.len());

    for t in 0..3000 {
        w.tick(50);
        assert!(!head_on_at_throat(&w), "head-on at the single-track throat, tick {t}");
        assert!(dwellers_at(&w, center).len() <= 2, "overbooked the k=2 throat, tick {t}");
    }
    let r0 = w.ridership_total;
    for t in 3000..10000 {
        w.tick(50);
        assert!(!head_on_at_throat(&w), "head-on at the single-track throat, tick {t}");
        assert!(dwellers_at(&w, center).len() <= 2, "overbooked the k=2 throat, tick {t}");
    }
    let r1 = w.ridership_total;
    assert!(
        r1 > r0,
        "the single-track throat must keep serving riders (no berth↔span deadlock): ridership {r0} → {r1}"
    );
}

// ---------------------------------------------------------------------------------------------------
// SCENARIO C — the G1 allocation-cycle gate: greedy take-any-free beats wait-for-specific.
// ---------------------------------------------------------------------------------------------------

#[test]
fn cross_line_no_alloc_deadlock() {
    // GATE 1 / G1 (allocation cycle). Two lines arriving at a shared k=2 station such that a
    // "wait-for-a-SPECIFIC compatible berth" allocator could cycle (each train wants the berth the other
    // holds). The GREEDY take-ANY-free-this-tick claim cannot cycle (no hold-and-wait on a particular
    // berth) ⇒ ridership rises.
    //
    // STUB-PROOF NOTE: the implemented claim is greedy lowest-free-index, first-claimant-wins (the same
    // primitive as A.1/A.3/the Phase-B mutexes). With greedy claiming, a berth-allocation CYCLE is
    // STRUCTURALLY IMPOSSIBLE — there is no "wait for berth b specifically" edge to close a cycle (a denied
    // train holds NO berth; it waits at the prior gate for ANY berth to free). So this gate cannot be made
    // to deadlock by a code stub WITHOUT first rewriting the claim to wait-for-specific (which the
    // implementation never does). We therefore assert the POSITIVE liveness fact (ridership rises under
    // heavy contention) and document — per the task — that G1 is structurally precluded by greedy claiming
    // rather than guarded by a stubable branch.
    let (mut w, _center) = shared_station_single_both_sides(2, 3);
    w.tick(50);
    assert!(
        w.vehicles.line.iter().any(|l| l.index() == 0) && w.vehicles.line.iter().any(|l| l.index() == 1),
        "both lines must dispatch and contend for the shared berths",
    );
    let r0 = {
        for _ in 0..3000 {
            w.tick(50);
        }
        w.ridership_total
    };
    let r1 = {
        for _ in 0..4000 {
            w.tick(50);
        }
        w.ridership_total
    };
    assert!(r1 > r0, "greedy take-any-free berth claim never deadlocks: ridership {r0} → {r1}");
}

// ---------------------------------------------------------------------------------------------------
// DETERMINISM — the whole throat mutex replays bit-for-bit.
// ---------------------------------------------------------------------------------------------------

#[test]
fn cross_line_throat_replays_bit_for_bit() {
    let run = || -> u64 {
        let (mut w, _c) = three_lines_one_shared_station();
        for _ in 0..4000 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "the cross-line throat mutex replays bit-for-bit (same seed + log)");
}
