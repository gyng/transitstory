//! TTD L2 — multi-platform stations. L2a (this file's first tests) pins the `BuildPlatforms` Command:
//! it sets/clamps/rejects the per-station berth count, replays deterministically, and — crucially — does
//! NOT re-dispatch (building a platform must not reset running trains, nor perturb the K=1 hash beyond the
//! one appended `platform_count` byte). The berth BEHAVIOUR (parallel dwell + never-freeze) lands in L2b.
use sim::station::MAX_PLATFORMS;
use sim::*;

/// A minimal running transit line: 3 stations in a row, one trainset, set running.
fn running_line() -> World {
    let mut w = World::new(42, CityData::default());
    for x in [0_i64, 4_000_000, 8_000_000] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..3u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    w
}

#[test]
fn build_platforms_sets_clamps_and_rejects() {
    let mut w = running_line();
    // sets the count
    let ev = w.apply(&Command::BuildPlatforms { station: StationId(1), k: 3 });
    assert_eq!(w.stations[1].platform_count, 3);
    assert!(matches!(ev.as_slice(), [Event::PlatformsBuilt { station: StationId(1), k: 3 }]));
    // clamps above MAX_PLATFORMS and below 1
    w.apply(&Command::BuildPlatforms { station: StationId(1), k: 99 });
    assert_eq!(w.stations[1].platform_count, MAX_PLATFORMS);
    w.apply(&Command::BuildPlatforms { station: StationId(1), k: 0 });
    assert_eq!(w.stations[1].platform_count, 1, "k=0 clamps to the always-present single berth");
    // rejects an unknown station without mutating
    let ev = w.apply(&Command::BuildPlatforms { station: StationId(99), k: 2 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]));
}

#[test]
fn build_platforms_does_not_redispatch_running_trains() {
    // Building a platform must NOT invalidate dispatch — otherwise every train resets to spawn (a gameplay
    // bug) and the K=1 golden would shift for more than the appended byte.
    let mut w = running_line();
    for _ in 0..120 {
        w.tick(50);
    }
    let before: Vec<i64> = w.vehicles.s_mm.clone();
    assert!(before.iter().any(|&s| s > 0), "trains should have moved off spawn");
    w.apply(&Command::BuildPlatforms { station: StationId(1), k: 4 });
    let after: Vec<i64> = w.vehicles.s_mm.clone();
    assert_eq!(before, after, "BuildPlatforms must not re-dispatch / reset train positions");
}

#[test]
fn build_platforms_replays_deterministically() {
    // Same seed + same log (incl. a BuildPlatforms) ⇒ identical state_hash twice in one process.
    let run = || -> u64 {
        let mut w = running_line();
        w.apply(&Command::BuildPlatforms { station: StationId(1), k: 3 });
        for _ in 0..200 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "a BuildPlatforms log replays bit-for-bit");
}

#[test]
fn k1_build_platforms_is_hash_neutral_mid_run() {
    // Applying BuildPlatforms{k:1} (the no-op default) at any point leaves the hash identical to never
    // issuing it — proving K=1 is inert at runtime (the field is the only difference, already = 1).
    let run = |issue: bool| -> u64 {
        let mut w = running_line();
        for _ in 0..80 {
            w.tick(50);
        }
        if issue {
            w.apply(&Command::BuildPlatforms { station: StationId(0), k: 1 });
        }
        for _ in 0..80 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(false), run(true), "BuildPlatforms{{k:1}} is a runtime no-op");
}

// --- L2b: the berth behaviour (parallel dwell + the follow-clamp relaxation + liveness) --------------

/// A short line packed with enough trains that they catch up to a dwelling leader (the jam), with `k`
/// berths on every station. `loop_line` runs them ALL one direction (no opposing meets — so a parallel
/// dwell can ONLY be the relaxation pulling a same-direction follower into a free berth).
fn bunched_line(k: u16, count: u16, loop_line: bool) -> World {
    let mut w = World::new(7, CityData::default());
    let xs = [0_i64, 600_000, 1_200_000, 1_800_000];
    for &x in &xs {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: if loop_line { x / 3 } else { 0 }, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line, mode: 0, literal: false });
    for s in 0..xs.len() as u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    if k > 1 {
        for s in 0..xs.len() as u32 {
            w.apply(&Command::BuildPlatforms { station: StationId(s), k });
        }
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// The station a consist is DWELLING at this tick (parked on a stop's arclen, dwell timer live), or None.
fn dwell_station_of(w: &World, i: usize) -> Option<u32> {
    if w.clock_ms >= w.vehicles.dwell_until_ms[i] {
        return None;
    }
    let line = &w.lines[w.vehicles.line[i].index()];
    let path = line.paths.get(w.vehicles.path[i] as usize)?;
    let s = w.vehicles.s_mm[i];
    let stop_idx = path.stop_arclen_mm.iter().position(|&a| a == s)?;
    Some(path.station_for_stop_index(stop_idx).0)
}

/// Run `ticks` and report (saw_relaxer, max_berth_idx). A "relaxer" is a consist with a berth claimed
/// while it is NOT dwelling — i.e. the follow-clamp relaxation pulling it INTO a free berth behind a
/// dwelling leader (the jam relief). Asserts the berth mutex every tick: no two consists hold the same
/// `(station, berth)`. (Liveness — no relaxation-induced deadlock — is the separate `platforms_never_freeze`
/// test; the relaxation can't overrun a leader on open track by construction, as it only un-clamps toward a
/// dwelling leader's stop and still brakes to halt there.)
fn run_and_measure(w: &mut World, ticks: usize) -> (bool, i32) {
    let mut saw_relaxer = false;
    let mut max_berth = -1;
    for _ in 0..ticks {
        w.tick(50);
        let mut berths: std::collections::BTreeSet<(u32, i32)> = std::collections::BTreeSet::new();
        for i in 0..w.vehicles.len() {
            let b = w.vehicles.berth_idx[i];
            max_berth = max_berth.max(b);
            let dwelling = dwell_station_of(w, i).is_some();
            if b >= 0 && !dwelling {
                saw_relaxer = true; // a berth claimed while still approaching ⇒ the relaxation fired
            }
            // berth-mutex exclusion, keyed at the consist's CURRENT station (dwelling or pulling in).
            if b >= 0 {
                if let Some(st) = station_at(w, i) {
                    assert!(berths.insert((st, b)), "two consists hold berth (station {st}, berth {b})");
                }
            }
        }
    }
    (saw_relaxer, max_berth)
}

/// The station a consist is AT or heading to its stop for (its current next-stop station) — used to key
/// the berth it holds. Falls back to the dwelling station.
fn station_at(w: &World, i: usize) -> Option<u32> {
    let line = &w.lines[w.vehicles.line[i].index()];
    let path = line.paths.get(w.vehicles.path[i] as usize)?;
    let s = w.vehicles.s_mm[i];
    // exact stop match (dwelling) or the next stop in travel direction (pulling in).
    if let Some(si) = path.stop_arclen_mm.iter().position(|&a| a == s) {
        return Some(path.station_for_stop_index(si).0);
    }
    None
}

#[test]
fn k2_relaxation_pulls_followers_into_berths_but_k1_does_not() {
    // LOOP line ⇒ all trains run one direction (no opposing meets), so a claimed berth on a NON-dwelling
    // consist can ONLY be the relaxation pulling a same-direction follower toward a free platform.
    let (relax_k1, max_b_k1) = run_and_measure(&mut bunched_line(1, 5, true), 4000);
    assert!(!relax_k1, "K=1: no free berth behind a dwelling leader ⇒ the relaxation never fires");
    assert_eq!(max_b_k1, 0, "K=1: only the single berth 0 is ever used");
    let (relax_k2, max_b_k2) = run_and_measure(&mut bunched_line(2, 5, true), 4000);
    assert!(relax_k2, "K=2: a follower should pull into a free berth behind a dwelling leader");
    assert!(max_b_k2 >= 1, "K=2: the second berth gets used (max berth {max_b_k2})");
}

#[test]
fn platforms_never_freeze() {
    // Liveness safeguard (a deterministic deadlock replays GREEN — only this catches a berth-mutex freeze).
    // Over-provision hard with K=2 berths on an out-and-back line; every consist must complete a circuit.
    let mut w = bunched_line(2, 6, false);
    let one_way = {
        let p = &w.lines[0].paths[0];
        p.length_mm()
    };
    let n = w.vehicles.len();
    let start: Vec<i64> = w.vehicles.s_mm.clone();
    // accumulate per-train absolute travel across the run (sum of |Δp| on the monotone loop coord proxy).
    let mut traveled = vec![0i64; n];
    let mut prev = start.clone();
    for _ in 0..8000 {
        w.tick(50);
        for i in 0..n {
            traveled[i] += (w.vehicles.s_mm[i] - prev[i]).abs();
            prev[i] = w.vehicles.s_mm[i];
        }
    }
    for i in 0..n {
        assert!(traveled[i] > one_way, "consist {i} froze (traveled {} <= one-way {one_way})", traveled[i]);
    }
}

#[test]
fn k2_run_replays_bit_for_bit() {
    // The whole berth behaviour is deterministic (same seed + log incl. BuildPlatforms{k:2} ⇒ same hash).
    let run = || -> u64 {
        let mut w = bunched_line(2, 5, true);
        for _ in 0..3000 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "K=2 parallel-dwell behaviour replays bit-for-bit");
}
