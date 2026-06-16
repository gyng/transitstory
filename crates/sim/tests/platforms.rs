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
