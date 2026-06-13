//! P4 — junction conflict at branch divergence/convergence points (docs/capacity-roadmap.md).
//! A branched line's trains genuinely converge on the shared switch where a branch leaves/rejoins
//! the trunk. Two consists straddling the SAME physical switch point = a collision the mutex must
//! forbid. Written RED-first: the switch-collision invariant fails today (P3 follow-streams are
//! per-(line,path), so a trunk train and a branch train pass through each other at the switch) and
//! passes once the junction-group mutex (Phase A.1.5 + B.4) lands. Tested through Commands +
//! observable positions; the collision check RE-DERIVES the switch geometry from the line topology
//! (branches/paths/stop_arclen) — independent of P4's internal `junctions` set — so it is genuinely
//! red before the movement clamp exists.
use sim::*;

// --- topology-recomputed safety check (no dependency on P4 internals) -------------------------

/// Half-open consist/point overlap: does segment [tail,head] STRICTLY straddle point `p`? (A train
/// resting with head exactly on the switch gate does not straddle it — matches the engine's
/// half-open `group_overlap`.) Direction-agnostic (orders the endpoints).
fn straddles(tail: i64, head: i64, p: i64) -> bool {
    let (a, b) = if tail <= head { (tail, head) } else { (head, tail) };
    b > p && a < p
}

/// Is any individual branch SWITCH point straddled by two or more consists at once? Recomputed
/// purely from the public topology: for every line's divergence trunk-stop `d`, every service PATH
/// whose prefix reaches `d` carries the switch at its own `stop_arclen_mm[d]`; a vehicle on that
/// path straddles the switch when its consist segment [head − dir·len, head] contains that arclen.
/// Two straddlers on one switch is the head-on/convergence collision P4 forbids.
fn switch_collision(w: &World) -> bool {
    for (li, line) in w.lines.iter().enumerate() {
        if line.branches.is_empty() {
            continue;
        }
        // Unique divergence trunk-stop indices (dedups a 3-way sharing one diverge_at).
        let mut diverge: Vec<usize> = Vec::new();
        for b in &line.branches {
            let d = (b.diverge_at as usize).min(line.stops.len().saturating_sub(1));
            if !diverge.contains(&d) {
                diverge.push(d);
            }
        }
        let len = line.vehicle_spec().length_mm;
        for &d in &diverge {
            let station = line.stops[d];
            let mut occupants = 0;
            for i in 0..w.vehicles.len() {
                if w.vehicles.line[i].index() != li {
                    continue;
                }
                let path = match line.paths.get(w.vehicles.path[i] as usize) {
                    Some(p) => p,
                    None => continue,
                };
                // This path must actually pass through the switch as the same station.
                if path.stops.get(d).copied() != Some(station) {
                    continue;
                }
                let pt = path.stop_arclen_mm[d];
                let head = w.vehicles.s_mm[i];
                let tail = head - (w.vehicles.dir[i] as i64) * len;
                if straddles(tail, head, pt) {
                    occupants += 1;
                }
            }
            if occupants >= 2 {
                return true;
            }
        }
    }
    false
}

// --- fixture builders -------------------------------------------------------------------------

/// A Y-line: a 4-stop trunk along x (irregular spacing) with one 2-stop branch diverging at trunk
/// stop 2 and heading off in +y. `mode` picks the consist (0=rail 140 m, 4=heavy 200 m).
fn y_line(mode: u8, trains: u16) -> World {
    let mut w = World::new(7, CityData::default());
    // Trunk stops 0..3 on the x-axis (irregular spans break opposing-train resonance).
    let trunk_x = [0i64, 2_100_000, 4_300_000, 6_800_000];
    for &x in &trunk_x {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    // Branch stops 4,5 fork up-and-away from the junction (trunk stop 2).
    w.apply(&Command::PlaceStation { x_mm: 4_900_000, y_mm: 2_400_000, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_400_000, y_mm: 5_100_000, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode, literal: false });
    for s in 0..4u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    // New branch (branch index == current count 0) diverging at trunk stop 2.
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(4) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(5) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// A JRL-shaped 3-way: a 3-stop trunk with TWO branches both diverging at trunk stop 1 (Bahar
/// Junction). Three service paths share one switch point.
fn jrl_3way(trains: u16) -> World {
    let mut w = World::new(11, CityData::default());
    for &x in &[0i64, 2_300_000, 4_700_000] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    // Two divergent fingers off stop 1, going +y and -y.
    w.apply(&Command::PlaceStation { x_mm: 2_900_000, y_mm: 2_200_000, name: None }); // 3
    w.apply(&Command::PlaceStation { x_mm: 3_400_000, y_mm: 4_400_000, name: None }); // 4
    w.apply(&Command::PlaceStation { x_mm: 2_900_000, y_mm: -2_200_000, name: None }); // 5
    w.apply(&Command::PlaceStation { x_mm: 3_400_000, y_mm: -4_400_000, name: None }); // 6
    w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..3u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(3) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(4) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 1, station: StationId(5) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 1, station: StationId(6) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Two divergence points within ONE rail consist-length (140 m) on the trunk — the coupled-junction
/// case both design adversaries used to break a naive point-mutex (a 2-cycle deadlock). Trunk stops
/// 1 and 2 sit only 100 m apart; branch A forks at stop 1, branch B at stop 2. P4 coalesces the two
/// switches into one atomic group, so it must NOT deadlock however many trains are thrown at it.
fn coupled_junctions(trains: u16) -> World {
    let mut w = World::new(5, CityData::default());
    // Trunk: 0 — 1 ··2 — 3 — 4 ; stops 1 and 2 are 100 000 mm apart (< 140 000 mm consist).
    let trunk_x = [0i64, 2_000_000, 2_100_000, 4_300_000, 6_500_000];
    for &x in &trunk_x {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    w.apply(&Command::PlaceStation { x_mm: 2_300_000, y_mm: 2_300_000, name: None }); // 5  (branch A)
    w.apply(&Command::PlaceStation { x_mm: 2_600_000, y_mm: 4_600_000, name: None }); // 6  (branch A)
    w.apply(&Command::PlaceStation { x_mm: 2_400_000, y_mm: -2_300_000, name: None }); // 7 (branch B)
    w.apply(&Command::PlaceStation { x_mm: 2_700_000, y_mm: -4_600_000, name: None }); // 8 (branch B)
    w.apply(&Command::CreateLine { color: 3, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..5u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(5) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(6) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 2, station: StationId(7) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 2, station: StationId(8) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Total absolute arc-length each dispatched vehicle travels over `ticks` steps (the liveness
/// metric: a frozen/deadlocked train accrues ~0; a live one runs round trips).
fn run_traveled(w: &mut World, ticks: usize) -> Vec<i64> {
    w.tick(50); // dispatch
    assert!(!switch_collision(w), "two consists straddling one switch at DISPATCH (tick 0)");
    let nveh = w.vehicles.len();
    let mut last = w.vehicles.s_mm.clone();
    let mut traveled = vec![0i64; nveh];
    for t in 0..ticks {
        w.tick(50);
        assert!(!switch_collision(w), "two consists straddling one switch at tick {t}");
        for i in 0..nveh {
            traveled[i] += (w.vehicles.s_mm[i] - last[i]).abs();
            last[i] = w.vehicles.s_mm[i];
        }
    }
    traveled
}

// --- tests ------------------------------------------------------------------------------------

#[test]
fn mutual_exclusion_at_y_and_jrl_junctions() {
    // A Y-line and a 3-way junction, over-provisioned, run for a long window. SAFETY: never two
    // consists on one switch point (the convergence/divergence collision). RED until P4: per-path
    // follow-streams let a trunk train and a branch train pass through each other at the switch.
    for mut w in [y_line(0, 6), jrl_3way(6)] {
        let traveled = run_traveled(&mut w, 6000);
        // A branched line must run a REAL fleet through its junction (not be throttled to ~2 trains);
        // the junction only meters crossings, it doesn't cap the line.
        assert!(traveled.len() >= 3, "branched line under-dispatched (junction over-throttling?)");
        // LIVENESS: the junction must not freeze anyone — every dispatched train keeps running.
        let total = w.lines[0].length_mm();
        assert!(
            *traveled.iter().min().unwrap() > total,
            "a train froze at the junction (min traveled {} < line {total})",
            traveled.iter().min().unwrap()
        );
    }
}

/// Two switches that are coupled on a BRANCH path but NOT on the trunk (the trunk d1→d2 span bows
/// off-axis toward its post-junction continuation, so its arclen exceeds one consist-length, while a
/// branch that diverges at d2 continues STRAIGHT, keeping its d1→d2 arclen below it). A trunk-axis
/// coalescing rule leaves the two switches in SEPARATE groups, but a branch consist straddles both —
/// the exact 2-cycle deadlock coalescing exists to kill (the design's Residual Risk #2, found in
/// review). The fix coalesces on the MIN per-path gap, so they merge into one atomic group.
fn branch_coupled_junctions(trains: u16) -> World {
    let mut w = World::new(9, CityData::default());
    let (x1, gap) = (3_000_000i64, 138_000i64); // trunk d1→d2 ≈ 141.5k (>140k); branch-B d1→d2 = 138k
    // Trunk 0, 1(=d1), 2(=d2), 3 — stop 3 pulled OFF-AXIS (+y) to bow the trunk's d1→d2 span longer.
    for &(x, y) in &[(0i64, 0i64), (x1, 0), (x1 + gap, 0), (x1 + gap + 1_500_000, 2_000_000)] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
    }
    // Branch A off d1 (up).
    w.apply(&Command::PlaceStation { x_mm: x1 + 200_000, y_mm: 2_000_000, name: None }); // 4
    w.apply(&Command::PlaceStation { x_mm: x1 + 400_000, y_mm: 4_000_000, name: None }); // 5
    // Branch B off d2 continuing STRAIGHT along +x (collinear ⇒ short d1→d2 on this path).
    w.apply(&Command::PlaceStation { x_mm: x1 + gap + 1_500_000, y_mm: 0, name: None }); // 6
    w.apply(&Command::PlaceStation { x_mm: x1 + gap + 3_000_000, y_mm: 0, name: None }); // 7
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..4u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(4) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(5) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 2, station: StationId(6) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 2, station: StationId(7) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn branch_coupled_junctions_coalesce_and_run() {
    // The two switches are coupled on branch B (138k < 140k) though NOT on the trunk (141.5k > 140k),
    // so they MUST coalesce into ONE atomic group — else a branch consist straddling both gridlocks
    // the line (a deterministic deadlock the replay gate can't see). RED before the min-per-path-gap
    // fix: trunk-axis coalescing leaves 2 groups and the line freezes.
    let mut w = branch_coupled_junctions(10);
    w.tick(50);
    assert_eq!(w.junctions.len(), 1, "branch-coupled switches must coalesce into one group");
    let traveled = run_traveled(&mut w, 12000);
    let total = w.lines[0].length_mm();
    assert!(
        *traveled.iter().min().unwrap() > total,
        "branch-coupled junction gridlocked (min traveled {} < line {total}) — trunk-only coalescing?",
        traveled.iter().min().unwrap()
    );
}

#[test]
fn coupled_junctions_never_deadlock() {
    // Two switches within one consist-length: a naive point-mutex deadlocks (train A holds switch 1,
    // gated at switch 2; train B holds switch 2, gated at switch 1 — a 2-cycle invisible to the
    // replay gate). P4's junction-group COALESCING collapses both into one atomic mutex, so it can
    // never gridlock. Over-provision hard; assert no collision AND no freeze. RED on safety until the
    // mutex lands; RED on liveness if the mutex is built WITHOUT coalescing.
    let mut w = coupled_junctions(12);
    let traveled = run_traveled(&mut w, 8000);
    // The line runs a real fleet through the coalesced cluster (not throttled to ~2 trains).
    assert!(traveled.len() >= 3, "coupled-junction line under-dispatched (over-throttling?)");
    let total = w.lines[0].length_mm();
    assert!(
        *traveled.iter().min().unwrap() > total,
        "coupled junction deadlocked (min traveled {} < line {total}) — coalescing missing?",
        traveled.iter().min().unwrap()
    );
}

/// A LOOP trunk (Circle-Line-like) with a 2-stop spur diverging mid-loop, run at high density. The
/// loop spreads trains around the whole circuit, so dispatch is likely to place one straddling the
/// switch — and returning spur trains re-cross it — stressing both the tick-0 snap and the running
/// mutex. The trunk is a loop; the spur path is out-and-back.
fn loop_with_spur(trains: u16) -> World {
    let mut w = World::new(13, CityData::default());
    // Hexagon loop (6 stops).
    let ring = [
        (0i64, 0i64),
        (2_400_000, 600_000),
        (3_000_000, 3_000_000),
        (1_500_000, 4_800_000),
        (-900_000, 4_200_000),
        (-1_500_000, 1_800_000),
    ];
    for &(x, y) in &ring {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
    }
    // Spur off ring stop 2, heading further out.
    w.apply(&Command::PlaceStation { x_mm: 5_400_000, y_mm: 3_600_000, name: None }); // 6
    w.apply(&Command::PlaceStation { x_mm: 7_800_000, y_mm: 4_200_000, name: None }); // 7
    w.apply(&Command::CreateLine { color: 4, name: None, loop_line: true, mode: 0, literal: false });
    for s in 0..6u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(6) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(7) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// A short out-and-back trunk with the junction at stop 1 (early) and a heavy fleet — the geometry
/// that, WITHOUT the dispatch snap, places a trunk train and a returning-branch train straddling the
/// same switch at tick 0 (verified by sweep). The snap moves such placements to the junction-station
/// gate, so the switch is collision-free from dispatch.
fn dense_early_junction(trains: u16) -> World {
    let mut w = World::new(3, CityData::default());
    for k in 0..4u32 {
        // Irregular spans (break opposing-train resonance), ~1.9 km apart.
        w.apply(&Command::PlaceStation { x_mm: k as i64 * 1_900_000 + (k as i64 % 3) * 250_000, y_mm: 0, name: None });
    }
    w.apply(&Command::PlaceStation { x_mm: 2_300_000, y_mm: 2_000_000, name: None }); // 4
    w.apply(&Command::PlaceStation { x_mm: 2_600_000, y_mm: 4_000_000, name: None }); // 5
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..4u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(4) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 1, station: StationId(5) });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn dense_early_junction_clean_from_dispatch() {
    // The dispatch snap keeps the early, heavily-served switch collision-free from tick 0 (this exact
    // fixture straddles at dispatch WITHOUT the snap), and the running mutex holds it after.
    let mut w = dense_early_junction(22);
    let traveled = run_traveled(&mut w, 4000); // asserts no collision at dispatch AND every tick
    assert!(traveled.len() >= 6, "the dense line should run a heavy fleet");
    let total = w.lines[0].length_mm();
    assert!(*traveled.iter().min().unwrap() > total, "a train froze at the dense early junction");
}

#[test]
fn dense_loop_with_spur_holds_the_switch_from_dispatch() {
    // High-density Circle-Line-with-a-spur: the dispatch snap must keep the switch collision-free from
    // tick 0 (a loop spreads trains around the circuit, so some land near the switch), and the running
    // mutex keeps it clean while trunk and returning-spur trains contend. Runs a real fleet.
    let mut w = loop_with_spur(16);
    let traveled = run_traveled(&mut w, 6000); // asserts no collision at dispatch AND every tick
    assert!(traveled.len() >= 4, "the dense loop+spur should run a real fleet");
    let total = w.lines[0].length_mm();
    assert!(*traveled.iter().min().unwrap() > total, "a train froze at the loop+spur switch");
}

#[test]
fn single_train_through_junction_not_gated() {
    // ONE train on a branched line is the sole occupant of every switch it crosses → the mutex must
    // never gate it (no spurious self-block). It must complete many round trips freely, identical in
    // spirit to a non-branched line. Guards against P4 over-gating ordinary same-path running.
    let mut w = y_line(0, 1);
    let traveled = run_traveled(&mut w, 4000);
    assert_eq!(w.vehicles.len(), 1, "single-train fixture");
    let total = w.lines[0].length_mm();
    assert!(traveled[0] > 3 * total, "a lone train must run freely through its own switches");
}

#[test]
fn branched_line_replays_bit_for_bit() {
    // The junction mutex is deterministic: same seed + same branched command log ⇒ identical hashed
    // state, twice in one process. (Junction occupancy is per-tick scratch, never hashed — this
    // passes from the already-hashed line/path/position fields.)
    let mut a = jrl_3way(6);
    let mut b = jrl_3way(6);
    for _ in 0..2000 {
        a.tick(50);
    }
    for _ in 0..2000 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "same seed + branched log ⇒ identical hashed state");
}

#[test]
fn non_branched_line_has_no_junctions() {
    // PARITY: a line with no branches contributes ZERO junctions, so the mutex passes are inert and a
    // non-branched network behaves byte-identically to pre-P4 (verified at large by the unchanged
    // existing fixtures). Here: a plain out-and-back line derives an empty junction set.
    let mut w = World::new(7, CityData::default());
    for k in 0..5u32 {
        w.apply(&Command::PlaceStation { x_mm: k as i64 * 2_000_000, y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..5u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert!(w.junctions.is_empty(), "a non-branched line must derive no junctions");
}

#[test]
fn junction_keys_are_command_order_independent() {
    // The group key = min(member StationId) is order-independent: the SAME branched topology built
    // with branches added in a DIFFERENT command order derives the identical junction-group key set.
    // (The determinism adversary's lock #2 — keys must not depend on branch insertion order.)
    let key_set = |w: &World| -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> =
            w.junctions.iter().map(|j| (j.line.index() as u32, j.key_station.index() as u32)).collect();
        v.sort_unstable();
        v
    };
    // Build A: branch at stop 1 first, then branch at stop 2.
    let mut a = coupled_junctions(4);
    a.tick(50);
    // Build B: same stations + branches, but the two branches added in the opposite order.
    let mut b = World::new(5, CityData::default());
    let xs = [0i64, 2_000_000, 2_100_000, 4_300_000, 6_500_000];
    for &x in &xs {
        b.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    b.apply(&Command::PlaceStation { x_mm: 2_300_000, y_mm: 2_300_000, name: None }); // 5
    b.apply(&Command::PlaceStation { x_mm: 2_600_000, y_mm: 4_600_000, name: None }); // 6
    b.apply(&Command::PlaceStation { x_mm: 2_400_000, y_mm: -2_300_000, name: None }); // 7
    b.apply(&Command::PlaceStation { x_mm: 2_700_000, y_mm: -4_600_000, name: None }); // 8
    b.apply(&Command::CreateLine { color: 3, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..5u32 {
        b.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    // Branch B (at stop 2) FIRST this time, then branch A (at stop 1).
    b.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(7) });
    b.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(8) });
    b.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 1, station: StationId(5) });
    b.apply(&Command::AddBranchStop { line: LineId(0), branch: 1, diverge_at: 1, station: StationId(6) });
    b.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    b.apply(&Command::SetRunning { running: true });
    b.tick(50);
    assert_eq!(key_set(&a), key_set(&b), "junction group keys must be command-order-independent");
}

#[test]
fn grade_separation_does_not_dissolve_switch_mutex() {
    // §7: grade-separating a BRANCH (Elevated) changes cost/speed but does NOT remove a SAME-LINE
    // switch conflict — two trains of one line still physically converge on the switch at any level.
    // Assert mutual exclusion still holds (no collision) after elevating the branch, and trains still
    // run (liveness). The dual — Elevated removing an at-grade *crossing* between distinct lines — is
    // a deferred P5 seam (not tested here).
    let mut w = y_line(0, 6);
    w.apply(&Command::SetBranchTrack { line: LineId(0), branch: 0, mode: 1 }); // Elevated branch
    let traveled = run_traveled(&mut w, 5000); // asserts no switch_collision each tick
    let total = w.lines[0].length_mm();
    assert!(
        *traveled.iter().min().unwrap() > total,
        "elevating the branch must keep the switch mutex live, not dissolve it"
    );
}

/// P5 SEAM (documented known limitation — `#[ignore]`d, un-ignore when P5 lands). When a BRANCHED
/// line is single-tracked on its SHARED TRUNK prefix, P2's single-track meet keys occupancy per
/// (line, PATH, span) — so the trunk service path and a branch service path get DIFFERENT keys for
/// the SAME physical trunk rail and never mutually exclude: two opposing consists pass through each
/// other on the shared single track. This is a PRE-EXISTING P2×P3 interaction (present since P2+P3,
/// untouched by P4 — P4's junction mutex guards the divergence POINT, not the single-track span
/// leading into it). A correct fix is the P5 "shared-track" phase: key the meet reservation on the
/// PHYSICAL trunk span shared across paths, AND add a cross-path liveness cap (the per-path cap lets
/// one trunk + one branch train onto a fully-single shared trunk — without a cross-path cap the
/// physical-key fix turns this cosmetic pass-through into a WORSE deadlock). Found by the P4
/// adversarial review; logged in PROGRESS.md / capacity-roadmap.md.
#[test]
#[ignore = "P5 shared-physical-track seam (single-track on a branched line's shared trunk); see doc"]
fn shared_trunk_single_track_no_headon_is_p5() {
    const SINGLE: u8 = 1;
    let mut w = World::new(7, CityData::default());
    for &x in &[0i64, 2_100_000, 4_300_000, 6_800_000] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    w.apply(&Command::PlaceStation { x_mm: 4_900_000, y_mm: 2_400_000, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_400_000, y_mm: 5_100_000, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..4u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(4) });
    w.apply(&Command::AddBranchStop { line: LineId(0), branch: 0, diverge_at: 2, station: StationId(5) });
    w.apply(&Command::SetSegmentTrack { line: LineId(0), span: u32::MAX, track: SINGLE });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    let len = w.lines[0].vehicle_spec().length_mm;
    // Divergence arclen on the trunk path: a vehicle before it is on the SHARED physical trunk rail.
    let switch = w.lines[0].paths[0].stop_arclen_mm[2];
    w.tick(50);
    for t in 0..6000 {
        w.tick(50);
        for a in 0..w.vehicles.len() {
            for b in (a + 1)..w.vehicles.len() {
                let on_trunk = w.vehicles.s_mm[a] < switch && w.vehicles.s_mm[b] < switch;
                if on_trunk && w.vehicles.dir[a] != w.vehicles.dir[b] {
                    let dx = (w.vehicles.x_mm[a] - w.vehicles.x_mm[b]) as i128;
                    let dy = (w.vehicles.y_mm[a] - w.vehicles.y_mm[b]) as i128;
                    let d = ((dx * dx + dy * dy) as f64).sqrt() as i64;
                    assert!(d >= len, "shared-trunk head-on at tick {t}: opposing consists {d} mm < {len} mm apart");
                }
            }
        }
    }
}
