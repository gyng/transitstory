//! TTD L5a — the player-placed block-signal STORE (docs/ttd-l5-plan.md). Distinct from `signals.rs`,
//! which tests the derived per-tick `signal_occupancy` render scratch; THIS pins the AUTHORITATIVE,
//! HASHED `world.signals` store. L5a records signals as replayable state but does NOT yet re-key
//! occupancy (that is L5b). Contract: validate-then-record, deterministic replay, command-order- AND
//! place/remove-INVARIANT hashing (the canonical sorted+deduped store), and rejection of bad placements.
use sim::*;

/// A straight 3-stop line (spans ~4_000_000 mm each), one trainset, so a signal at at_mm=2_000_000 is
/// strictly inside span 0. Not running — L5a needs no motion.
fn line3() -> World {
    let mut w = World::new(42, CityData::default());
    for x in [0_i64, 4_000_000, 8_000_000] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..3u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 1 });
    w
}

fn place(w: &mut World, span: u32, at_mm: i64) -> Vec<Event> {
    w.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span, at_mm })
}

#[test]
fn place_signal_records_and_validates() {
    let mut w = line3();
    assert_eq!(w.signals.len(), 0, "no signals to start");
    let ev = place(&mut w, 0, 2_000_000); // strictly inside span 0 ⇒ accepted
    assert!(matches!(ev.as_slice(), [Event::SignalPlaced { .. }]), "valid placement accepted: {ev:?}");
    assert_eq!(w.signals.len(), 1);
    assert_eq!(w.signals[0], Signal { line: LineId(0), path: 0, span: 0, at_mm: 2_000_000 });

    // ON a station gate (the span boundary) ⇒ rejected (must be STRICTLY inside).
    assert!(matches!(place(&mut w, 0, 0).as_slice(), [Event::Rejected { .. }]), "signal on a gate rejected");
    // nonexistent span ⇒ rejected.
    assert!(matches!(place(&mut w, 99, 1_000_000).as_slice(), [Event::Rejected { .. }]), "missing span rejected");
    // nonexistent line ⇒ rejected.
    let ev = w.apply(&Command::PlaceSignal { line: LineId(9), path: 0, span: 0, at_mm: 1_000_000 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "missing line rejected");
    assert_eq!(w.signals.len(), 1, "rejections must not mutate the store");
}

#[test]
fn a_signal_adds_capital_cost_refunded_on_removal() {
    // TTD L5d: a placed signal carries a small capital cost (so signalling is an economic tradeoff),
    // refunded on removal. The cost recompute is triggered directly from the dispatch-exempt apply arm.
    let mut w = line3();
    let base = w.lines[0].capital_cost;
    assert!(base > 0, "the line has a track capital cost to begin with");
    place(&mut w, 0, 2_000_000);
    let with_one = w.lines[0].capital_cost;
    assert!(with_one > base, "a placed signal must add capital cost: {base} -> {with_one}");
    place(&mut w, 0, 3_000_000); // a second signal on the same span
    assert!(w.lines[0].capital_cost > with_one, "each signal adds cost (two cost more than one)");
    // removing both refunds exactly back to the track-only base.
    w.apply(&Command::RemoveSignal { line: LineId(0), path: 0, span: 0, at_mm: 2_000_000 });
    w.apply(&Command::RemoveSignal { line: LineId(0), path: 0, span: 0, at_mm: 3_000_000 });
    assert_eq!(w.lines[0].capital_cost, base, "removing every signal refunds the capital exactly");
}

#[test]
fn place_then_remove_is_hash_neutral() {
    let mut w = line3();
    let h0 = w.state_hash();
    assert!(matches!(place(&mut w, 0, 2_000_000).as_slice(), [Event::SignalPlaced { .. }]));
    let h1 = w.state_hash();
    assert_ne!(h0, h1, "placing a signal must change the hash (it is authoritative, hashed state)");
    w.apply(&Command::RemoveSignal { line: LineId(0), path: 0, span: 0, at_mm: 2_000_000 });
    assert_eq!(w.signals.len(), 0);
    assert_eq!(w.state_hash(), h0, "remove must return the hash exactly to pre-placement (no residue)");
}

#[test]
fn signal_store_is_deduped_and_command_order_independent() {
    let build = |order: &[(u32, i64)]| -> u64 {
        let mut w = line3();
        for &(span, at) in order {
            w.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span, at_mm: at });
        }
        w.state_hash()
    };
    let a = build(&[(0, 1_000_000), (0, 3_000_000), (1, 2_000_000)]);
    let b = build(&[(1, 2_000_000), (0, 3_000_000), (0, 1_000_000)]); // shuffled
    assert_eq!(a, b, "signal-set hash must be independent of placement command order");

    let mut w = line3();
    place(&mut w, 0, 1_000_000);
    let h = w.state_hash();
    assert!(matches!(place(&mut w, 0, 1_000_000).as_slice(), [Event::SignalPlaced { .. }]), "dup echoes placed");
    assert_eq!(w.signals.len(), 1, "duplicate signal not stored twice");
    assert_eq!(w.state_hash(), h, "duplicate placement is hash-neutral (deduped)");
}

#[test]
fn signal_log_replays_bit_for_bit() {
    let run = || -> u64 {
        let mut w = line3();
        w.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span: 0, at_mm: 1_500_000 });
        w.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span: 1, at_mm: 6_000_000 });
        w.apply(&Command::RemoveSignal { line: LineId(0), path: 0, span: 0, at_mm: 1_500_000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..400 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "a signal-bearing log replays bit-for-bit");
}

#[test]
fn placing_a_signal_mid_run_does_not_redispatch_or_reset_trains() {
    // REGRESSION (L5b): placing a signal must NOT re-dispatch — that would teleport every running train
    // back to spawn (the BuildPlatforms exemption rationale). Proof: on a DEFAULT double-track line a
    // signal is behaviourally INERT (the relaxation only lifts a single-track meet denial), so a world
    // that places one mid-run must keep vehicle positions BYTE-IDENTICAL to a control that does not.
    let build = || {
        let mut w = line3();
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..400 {
            w.tick(50);
        }
        w
    };
    let mut control = build();
    let mut with_sig = build();
    // both are at the same point now
    assert_eq!(control.vehicles.s_mm, with_sig.vehicles.s_mm, "control + test diverged before the edit");
    // place a signal on the (double-track) span 0 mid-run — valid (strictly inside), behaviourally inert.
    let ev = with_sig.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span: 0, at_mm: 2_000_000 });
    assert!(matches!(ev.as_slice(), [Event::SignalPlaced { .. }]), "the mid-run placement is valid: {ev:?}");
    // the apply itself must not move trains (it mutates only the signals store)...
    assert_eq!(control.vehicles.s_mm, with_sig.vehicles.s_mm, "PlaceSignal apply must not reset/move trains");
    // ...and after a tick they must still match the control (no re-dispatch reset, signal inert on double track).
    control.tick(50);
    with_sig.tick(50);
    assert_eq!(
        control.vehicles.s_mm, with_sig.vehicles.s_mm,
        "a mid-run signal must not re-dispatch (trains would reset to spawn); it is inert on double track",
    );
}

#[test]
fn no_signal_run_is_deterministic_pre_l5b_path() {
    // L5a adds NO behaviour: a run that places no signal ticks via exactly the pre-L5a motion path
    // (proven byte-identical by the unchanged position fingerprints). Locally: two builds agree.
    let run = || -> u64 {
        let mut w = line3();
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..600 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "a signal-free run is deterministic (the pre-L5a path)");
}
