//! The load-bearing determinism contract (written first, AGENTS testing): same seed +
//! same ordered command log + same tick schedule => identical state_hash, twice in one
//! process. Plus the command-codec round-trips (JSON wire, postcard save) the contract
//! is defined against.
use sim::*;

/// A representative slice command log: 3 stations, one line through them, a trainset and headway.
fn sample_log() -> Vec<Command> {
    vec![
        Command::PlaceStation { x_mm: 0, y_mm: 0, name: None },
        Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None },
        Command::PlaceStation { x_mm: 10_000_000, y_mm: 2_000_000, name: Some("Marina".into()) },
        Command::CreateLine { color: 0x3366cc, name: None, loop_line: false, mode: 0 },
        Command::AddStop { line: LineId(0), station: StationId(0), after: None },
        Command::AddStop { line: LineId(0), station: StationId(1), after: None },
        Command::AddStop { line: LineId(0), station: StationId(2), after: None },
        Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 },
        Command::SetHeadway { line: LineId(0), headway_ms: 240_000 },
        Command::SetRunning { running: true },
    ]
}

fn run(seed: u64, log: &[Command], ticks: usize, dt_ms: i64) -> u64 {
    let mut w = World::new(seed, CityData::default());
    for c in log {
        w.apply(c);
    }
    for _ in 0..ticks {
        w.tick(dt_ms);
    }
    w.state_hash()
}

#[test]
fn replay_equality() {
    let log = sample_log();
    let a = run(42, &log, 600, 50);
    let b = run(42, &log, 600, 50);
    assert_eq!(a, b, "same seed + command log + ticks must yield identical state_hash");
}

#[test]
fn distinct_inputs_differ() {
    // Sanity: a different command log should (almost surely) hash differently, so the
    // determinism test isn't passing by hashing nothing.
    let log = sample_log();
    let mut shorter = log.clone();
    shorter.truncate(2);
    assert_ne!(run(1, &log, 10, 50), run(1, &shorter, 10, 50));
}

#[test]
fn command_json_roundtrip() {
    for c in sample_log() {
        let json = serde_json::to_string(&c).expect("serialize");
        let back: Command = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(c, back, "Command must round-trip through JSON (the live wire format)");
    }
}

#[test]
fn save_postcard_roundtrip_and_replay() {
    let log = sample_log();
    let mut w = World::new(7, CityData::default());
    for c in &log {
        w.apply(c);
    }
    let save = w.save();

    // postcard round-trip (the save artifact format).
    let bytes = postcard::to_allocvec(&save).expect("postcard encode");
    let back: SaveGame = postcard::from_bytes(&bytes).expect("postcard decode");
    assert_eq!(back.commands, log);

    // Replaying the saved log reproduces identical pre-tick state.
    let replayed = replay(&back, CityData::default());
    assert_eq!(w.state_hash(), replayed.state_hash());
}

#[test]
fn invalid_commands_are_rejected_not_panicking() {
    let mut w = World::new(0, CityData::default());
    // AddStop referencing nonexistent line/station -> Rejected, no panic, no mutation.
    let ev = w.apply(&Command::AddStop {
        line: LineId(9),
        station: StationId(9),
        after: None,
    });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]));
}

#[test]
fn headway_and_count_are_clamped() {
    let mut w = World::new(0, CityData::default());
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 1 }); // below floor
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 9999 }); // above cap
    let l = &w.lines[0];
    assert_eq!(l.headway_ms, MIN_HEADWAY_MS);
    assert_eq!(l.trainset.unwrap().count, MAX_TRAINS_PER_LINE);
}
