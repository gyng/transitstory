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
        Command::CreateLine { color: 0x3366cc, name: None, loop_line: false, mode: 0, literal: false },
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

/// The GOLDEN PIN (fantasy-build-plan.md S0). `replay_equality` proves `run()==run()` but is
/// structurally BLIND to a uniform hash shift — any change that perturbs every hash identically
/// (a Canonical field reorder, an rng-draw-order change during the fantasy carve, a postcard
/// bump) sails through it. This literal is the one anchor that catches that class: it is the
/// exact `state_hash` of the canonical transit slice today, pinned. If a refactor that claims to
/// be behaviour-preserving changes this value, it is NOT behaviour-preserving — STOP. The pin is
/// re-blessed as a reviewed single commit at every deliberate Canonical shape change (S7/S8/S10).
// Re-pinned at S7 (binding condition #1) as fantasy state joined `Canonical` (all 0/empty for transit,
// so transit stays byte-identical — only the appended bytes shift the hash):
//   0xdeeb_747a_eb78_c6a1 — S0–S6 (no fantasy fields)
//   0x42dd_8dde_1e39_8393 — S7a (the empty `forge_stock` slice appended)
//   0xd7fb_a36d_5bba_92c9 — S7d (the `tribute` i64 appended)
//   0x45f8_da5f_19af_73f3 — S8a (the 7 empty army-SoA field slices appended)
//   0xd747_5260_98d0_0aeb — S8b (the empty `town_value` slice + `towns_captured` i64 appended)
//   0x9e3b_e523_a982_8d51 — S8 PlaceBarracks (the empty `is_barracks` slice appended)
//   0x6253_ac99_08d6_20a3 — S8 PostBounty (the empty `bounty` slice appended)
//   0xea4e_eb0a_03d9_74f9 — S9 decadence (the `decadence` i64 appended)
//   0xfd8e_5b04_8a81_c31b — S10b decadence CA (the empty `decadence_cells` slice appended)
//   0x5aa7_c3b7_5a7e_86e1 — S11 tech (the `tech_unlocked` u32 appended, 0 for transit)
//   0xcd39_4898_bd9f_1d09 — S11 economy split (the `mana`+`manpower` i64s appended, 0 for transit)
//   0x9c0d_0265_845a_38b3 — S11 rival (the raider SoA slices + spawn-accum/cursor/breach/breach-heal-accum
//                           appended, all empty/0 for transit — no reservoir ⇒ no raiders)
//   0x8453_c57f_e54e_5829 — S11 spell arm (the `spells_cast` u32 appended, 0 for transit — no SPELLCRAFT)
//   0x28b0_c152_a41f_cdab — war-batch rail-attack (the `line_disabled_until_ms` slice appended, EMPTY for
//                           transit — no raiders ⇒ no raid ⇒ no cut lines; the re-pin is the length-0 byte)
//   0xd753_a804_17cc_9163 — war-batch saboteur targeting (the `raider_tx_mm`+`raider_ty_mm` slices appended,
//                           EMPTY for transit — no reservoir ⇒ no raiders; the re-pin is two length-0 bytes)
//   0x2f16_02bb_65d4_68ca — TTD L2 multi-platform (the `Station.platform_count: u8` field appended, = 1 for
//                           every station; behaviour-byte-identical — default K=1 has one berth, the
//                           follow-clamp relaxation never fires, dispatch untouched; the re-pin is one byte
//                           per station). If a behaviour-PRESERVING refactor changes this, STOP — a real
//                           drift is a determinism bug, not a re-pin.
const GOLDEN_TRANSIT_HASH: u64 = 0x2f16_02bb_65d4_68ca;

#[test]
fn golden_transit_hash_pinned() {
    let h = run(42, &sample_log(), 600, 50);
    assert_eq!(
        h, GOLDEN_TRANSIT_HASH,
        "transit golden state_hash drifted: 0x{h:016x} != 0x{GOLDEN_TRANSIT_HASH:016x}. \
         If this was an intentional Canonical change, re-pin in a reviewed commit; otherwise a \
         supposedly behaviour-preserving refactor broke determinism."
    );
}

/// The save→postcard→replay→tick pipeline must ALSO reach the golden hash, not just the in-memory
/// `run()`. Guards that serializing the command log and reconstructing from it is byte-faithful.
#[test]
fn golden_transit_hash_via_save_replay() {
    let mut w = World::new(42, CityData::default());
    for c in &sample_log() {
        w.apply(c);
    }
    let bytes = postcard::to_allocvec(&w.save()).expect("postcard encode");
    let save: SaveGame = postcard::from_bytes(&bytes).expect("postcard decode");
    let mut r = replay(&save, CityData::default());
    for _ in 0..600 {
        r.tick(50);
    }
    assert_eq!(r.state_hash(), GOLDEN_TRANSIT_HASH, "save→replay→tick must reach the golden pin");
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

/// The disjoint-save guard (S3): `""` (the `CityData::default()` tag) and `"transit"` are the SAME
/// mode (canonicalised), so a save written with the explicit `"transit"` tag must replay cleanly
/// onto a default (`""`) city — the guard compares modes, not spellings, and must NOT false-trip.
#[test]
fn disjoint_save_guard_treats_empty_and_transit_as_one_mode() {
    let mut w = World::new(7, CityData { ruleset: "transit".into(), ..Default::default() });
    for c in &sample_log() {
        w.apply(c);
    }
    let save = w.save();
    assert_eq!(save.ruleset, "transit", "save carries the city's ruleset tag");
    // city default tag is "" — canon("")==canon("transit") ⇒ no panic, replay reconstructs state.
    let replayed = replay(&save, CityData::default());
    assert_eq!(w.state_hash(), replayed.state_hash(), "same-mode replay reconstructs identical state");
}

/// The guard's teeth: a save from a DIFFERENT mode replayed onto a transit city must abort, not
/// silently run a foreign command vocab through the wrong `apply` (the divergence class the golden
/// pin can't see). RED-first via `should_panic`.
#[test]
#[should_panic(expected = "disjoint-save guard")]
fn disjoint_save_guard_rejects_cross_mode_replay() {
    let mut w = World::new(7, CityData::default());
    for c in &sample_log() {
        w.apply(c);
    }
    let mut save = w.save();
    save.ruleset = "arcadia".into(); // a save from the (future) fantasy mode
    let _ = replay(&save, CityData::default()); // onto a transit city ⇒ panic
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
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 1 }); // below floor
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 9999 }); // above cap
    let l = &w.lines[0];
    assert_eq!(l.headway_ms, MIN_HEADWAY_MS);
    assert_eq!(l.trainset.unwrap().count, MAX_TRAINS_PER_LINE);
}
