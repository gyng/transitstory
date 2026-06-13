//! P2 — single vs double track (docs/capacity-roadmap.md). On a SINGLE span, opposing-direction
//! trains cannot both be inside — they MEET at the bounding stations (passing places). Written
//! RED-first: the head-on invariant fails today (trains pass through each other on single track)
//! and passes once the two-phase meet protocol lands. Tested through Commands + observable positions.
use sim::*;

const SINGLE: u8 = 1; // line::track::SINGLE
const WHOLE: u32 = u32::MAX;

/// X of stop `k` — IRREGULAR spacing on purpose: with even spacing, evenly-dispatched opposing
/// trains stay in lockstep and cross exactly at stations (never mid-span), so a head-on can't occur
/// and the meet protocol is never exercised. Irregular spans break that resonance ⇒ trains meet
/// strictly inside spans, which is the case P2 must arbitrate.
fn x_of(k: u32) -> i64 {
    const SPANS: [i64; 5] = [1_700_000, 2_200_000, 2_500_000, 1_900_000, 2_400_000];
    (0..k as usize).map(|j| SPANS[j % SPANS.len()]).sum()
}

/// A straight out-and-back line of `stations` stops (irregularly spaced) with `trains` trainsets.
fn line_n(stations: u32, trains: u16) -> World {
    let mut w = World::new(7, CityData::default());
    for k in 0..stations {
        w.apply(&Command::PlaceStation { x_mm: x_of(k), y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..stations {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: trains });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Is any vehicle strictly inside `s`'s span? Returns the (line,path,span,dir) if so — used to detect
/// two opposing trains inside the SAME single span (a head-on), which the meet protocol must forbid.
fn head_on(w: &World) -> bool {
    // (line, path, span) -> dir of the train strictly inside that SINGLE span.
    let mut occ: Vec<((usize, u8, usize), i8)> = Vec::new();
    for i in 0..w.vehicles.len() {
        let li = w.vehicles.line[i].index();
        let pa = w.vehicles.path[i];
        let path = match w.lines[li].paths.get(pa as usize) {
            Some(p) => p,
            None => continue,
        };
        let s = w.vehicles.s_mm[i];
        let sp = path.span_of(s);
        let lo = path.stop_arclen_mm.get(sp).copied().unwrap_or(i64::MIN);
        let hi = path.stop_arclen_mm.get(sp + 1).copied().unwrap_or(i64::MAX);
        if !(s > lo && s < hi) {
            continue; // on a gate (station/passing place) — owns nothing
        }
        if path.track_type.get(sp).copied().unwrap_or(0) != SINGLE {
            continue; // double track: opposing trains may coexist
        }
        let dir = if path.loop_line { 1 } else { w.vehicles.dir[i] };
        let key = (li, pa, sp);
        if let Some(&(_, d0)) = occ.iter().find(|(k, _)| *k == key) {
            if d0 != dir {
                return true; // two opposing trains strictly inside one single span
            }
        } else {
            occ.push((key, dir));
        }
    }
    false
}

#[test]
fn no_head_on_with_multiple_trains_on_single_sections() {
    // Two SINGLE sections (spans 1 and 3) separated by double-track passing places, several trains
    // (within the single-track capacity cap) ⇒ opposing trains repeatedly meet on the single
    // sections. SAFETY: never two opposing trains inside one single span (head-on). LIVENESS: every
    // train keeps making progress (no deadlock). RED until P2: trains pass through each other.
    let mut w = line_n(6, 3);
    w.apply(&Command::SetSegmentTrack { line: LineId(0), span: 1, track: SINGLE });
    w.apply(&Command::SetSegmentTrack { line: LineId(0), span: 3, track: SINGLE });
    w.tick(50); // dispatch
    let nveh = w.vehicles.len();
    assert!(nveh >= 2, "need ≥2 trains to meet (got {nveh})");
    let total = w.lines[0].length_mm();
    let mut last = w.vehicles.s_mm.clone();
    let mut traveled = vec![0i64; nveh];
    for t in 0..6000 {
        w.tick(50);
        assert!(!head_on(&w), "head-on collision on a single span at tick {t}");
        for i in 0..nveh {
            traveled[i] += (w.vehicles.s_mm[i] - last[i]).abs();
            last[i] = w.vehicles.s_mm[i];
        }
    }
    let min_traveled = *traveled.iter().min().unwrap();
    assert!(
        min_traveled > total,
        "every train must keep moving (no deadlock): min traveled {min_traveled} mm < line {total} mm",
    );
}

/// Does a fully-single line of `stations` stops dispatched with `trains` PERMANENTLY freeze? Warm,
/// snapshot, then watch a long window — if no vehicle ever moves, the network is gridlocked.
fn freezes(stations: u32, trains: u16) -> bool {
    let mut w = line_n(stations, trains);
    w.apply(&Command::SetSegmentTrack { line: LineId(0), span: WHOLE, track: SINGLE });
    w.tick(50);
    for _ in 0..4000 {
        w.tick(50);
    }
    let snap = w.vehicles.s_mm.clone();
    for _ in 0..2000 {
        w.tick(50);
        if w.vehicles.s_mm != snap {
            return false; // something moved ⇒ not frozen
        }
    }
    true
}

#[test]
fn over_provisioned_single_track_never_freezes() {
    // A single-track line must NEVER deadlock, however many trains the player throws at it — the
    // surplus is held undispatched (single track is low-capacity: a fully-single out-and-back is a
    // one-train shuttle). RED until the dispatch single-track cap: trains >= stops gridlock.
    for (st, tr) in [(2u32, 2u16), (3, 3), (3, 4), (4, 4), (4, 5), (5, 6)] {
        assert!(!freezes(st, tr), "single-track line FROZE at {st} stops / {tr} trains");
    }
}

#[test]
fn single_track_replays_bit_for_bit() {
    // The meet protocol is deterministic, incl. a whole-line + a per-span SetSegmentTrack in the log.
    let build = || {
        let mut w = line_n(4, 3);
        w.apply(&Command::SetSegmentTrack { line: LineId(0), span: 1, track: SINGLE });
        w.apply(&Command::SetSegmentTrack { line: LineId(0), span: WHOLE, track: SINGLE });
        w
    };
    let mut a = build();
    let mut b = build();
    for _ in 0..1500 {
        a.tick(50);
    }
    for _ in 0..1500 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "same seed + log ⇒ identical hashed state");
}

#[test]
fn single_track_is_cheaper_to_build() {
    // Same span: single track costs less capital than double, and still > 0.
    let cost = |single: bool| -> i64 {
        let mut w = line_n(3, 1);
        if single {
            w.apply(&Command::SetSegmentTrack { line: LineId(0), span: WHOLE, track: SINGLE });
        }
        w.lines[0].capital_cost
    };
    let double = cost(false);
    let single = cost(true);
    assert!(single > 0 && single < double, "single ({single}) should be cheaper than double ({double})");
}

#[test]
fn opposing_trains_meet_at_a_middle_single_span() {
    // Only the MIDDLE span is single (double on both sides = passing loops). Opposing trains meet
    // there — assert NEVER a head-on, and both trains keep completing round trips (reversals): a
    // clean single-track section between passing places resolves the meet without deadlock.
    let mut w = line_n(5, 2);
    w.apply(&Command::SetSegmentTrack { line: LineId(0), span: 2, track: SINGLE });
    w.tick(50);
    let nveh = w.vehicles.len();
    let mut reversals = vec![0u32; nveh];
    let mut last_dir = w.vehicles.dir.clone();
    for t in 0..6000 {
        w.tick(50);
        assert!(!head_on(&w), "head-on at the middle single span, tick {t}");
        for i in 0..nveh {
            if w.vehicles.dir[i] != last_dir[i] {
                reversals[i] += 1;
                last_dir[i] = w.vehicles.dir[i];
            }
        }
    }
    assert!(*reversals.iter().min().unwrap() >= 4, "both trains complete multiple round trips through the meet");
}

#[test]
fn single_track_loop_does_not_bind() {
    // A single-track LOOP has no opposing direction ⇒ P2 is a PURE cost discount: identical motion to
    // a double loop (no meet gating, no dispatch-snap perturbation, no per-block serialisation).
    let build = |single: bool| -> World {
        let mut w = World::new(7, CityData::default());
        for (x, y) in [(0i64, 0i64), (2_000_000, 0), (2_000_000, 2_000_000), (0, 2_000_000), (0, 1_000_000)] {
            w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
        }
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: true, mode: 0, literal: false });
        for s in 0..5 {
            w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
        }
        if single {
            w.apply(&Command::SetSegmentTrack { line: LineId(0), span: WHOLE, track: SINGLE });
        }
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
        w.apply(&Command::SetRunning { running: true });
        w
    };
    let mut single = build(true);
    let mut dbl = build(false);
    single.tick(50);
    dbl.tick(50);
    // Dispatch must NOT perturb a single-track loop (no snap) ⇒ identical placement to double.
    assert_eq!(single.vehicles.s_mm, dbl.vehicles.s_mm, "single-track loop dispatch must equal double");
    let nveh = single.vehicles.len();
    let total = single.lines[0].length_mm();
    let mut last = single.vehicles.s_mm.clone();
    let mut traveled = vec![0i64; nveh];
    for _ in 0..3000 {
        single.tick(50);
        for i in 0..nveh {
            traveled[i] += (single.vehicles.s_mm[i] - last[i]).abs();
            last[i] = single.vehicles.s_mm[i];
        }
    }
    assert!(*traveled.iter().min().unwrap() > total, "single-track loop trains run freely (P2 exempt)");
}

#[test]
fn double_track_lets_opposing_trains_pass() {
    // The DEFAULT (double) imposes no meet — opposing trains coexist on a span (P1 behaviour intact).
    // This run never sets single track, so `head_on` (which only flags SINGLE spans) is vacuously
    // false; the real point is liveness identical to pre-P2: trains complete round trips freely.
    let mut w = line_n(4, 3);
    w.tick(50);
    let nveh = w.vehicles.len();
    let total = w.lines[0].length_mm();
    let mut last = w.vehicles.s_mm.clone();
    let mut traveled = vec![0i64; nveh];
    for _ in 0..3000 {
        w.tick(50);
        for i in 0..nveh {
            traveled[i] += (w.vehicles.s_mm[i] - last[i]).abs();
            last[i] = w.vehicles.s_mm[i];
        }
    }
    assert!(*traveled.iter().min().unwrap() > 2 * total, "double-track trains run freely");
}
