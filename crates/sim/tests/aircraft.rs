//! AIR aircraft roster — the `AssignTrainset{spec}` seam made real. The roster lets a player pick
//! stock per route, but it must satisfy two load-bearing invariants: (1) index 0 stays the locked
//! default so every existing globe save replays bit-for-bit and the determinism hash never moves;
//! (2) the capacity-vs-turnaround ladder is non-dominated (no strictly-best aircraft). These are
//! native core invariants; the picker UI is a separate frontend concern.
use sim::trainset::{spec_for, spec_for_mode, tmode, AIR_ROSTER};
use sim::*;

/// Index 0 is the historical single AIR preset, byte-for-byte. If this drifts, existing globe saves
/// stop replaying identically and the determinism hash moves — so it is pinned here explicitly.
#[test]
fn air_default_spec_is_locked() {
    let d = spec_for(tmode::AIR, 0);
    assert_eq!(d.capacity, 250);
    assert_eq!(d.v_max_mm_s, 1_800_000_000); // clock-frame (x30)
    assert_eq!(d.accel_mm_s2, 2_700_000_000); // clock-frame (x900)
    assert_eq!(d.decel_mm_s2, 2_700_000_000);
    assert_eq!(d.dwell_ms, 60_000);
}

/// spec id 0 == the mode default for EVERY mode (an unassigned/default-assigned route is unchanged).
/// This is the property that keeps the roster determinism-safe: today the frontend always sends 0.
#[test]
fn spec_zero_is_the_mode_default_for_all_modes() {
    for mode in [tmode::RAIL, tmode::BUS, tmode::FERRY, tmode::AIR, tmode::HEAVY] {
        let a = spec_for(mode, 0);
        let b = spec_for_mode(mode);
        assert_eq!(a.capacity, b.capacity, "capacity mode {mode}");
        assert_eq!(a.v_max_mm_s, b.v_max_mm_s, "v_max mode {mode}");
        assert_eq!(a.dwell_ms, b.dwell_ms, "dwell mode {mode}");
        assert_eq!(a.accel_mm_s2, b.accel_mm_s2, "accel mode {mode}");
        assert_eq!(a.decel_mm_s2, b.decel_mm_s2, "decel mode {mode}");
    }
}

/// The roster is NON-DOMINATED: ordered by capacity, ground turnaround (dwell) rises in lockstep, so
/// a bigger jet always pays for its seats with a slower turn — no aircraft is strictly best.
#[test]
fn air_roster_is_non_dominated() {
    let mut by_cap: Vec<TrainsetSpec> = AIR_ROSTER.to_vec();
    by_cap.sort_by_key(|s| s.capacity);
    for w in by_cap.windows(2) {
        assert!(w[1].capacity > w[0].capacity, "capacities are distinct & strictly increasing");
        assert!(
            w[1].dwell_ms >= w[0].dwell_ms,
            "more capacity costs >= ground turnaround (no strictly-best plane)"
        );
    }
    // …and at least one rung actually pays MORE dwell, else the tradeoff would be vacuous.
    assert!(
        by_cap.last().unwrap().dwell_ms > by_cap.first().unwrap().dwell_ms,
        "the biggest jet turns slower than the smallest"
    );
}

/// Out-of-range spec ids clamp to the last roster entry — the hot path never indexes OOB / panics.
#[test]
fn spec_id_clamps_in_range() {
    let last = AIR_ROSTER[AIR_ROSTER.len() - 1];
    let clamped = spec_for(tmode::AIR, 250);
    assert_eq!(clamped.capacity, last.capacity);
    assert_eq!(clamped.dwell_ms, last.dwell_ms);
}

/// Non-air modes have no roster yet: any spec id resolves to the mode preset, so a stray spec on a
/// rail/bus/ferry/heavy line is harmless.
#[test]
fn non_air_modes_ignore_spec_id() {
    for mode in [tmode::RAIL, tmode::BUS, tmode::FERRY, tmode::HEAVY] {
        assert_eq!(spec_for(mode, 3).capacity, spec_for_mode(mode).capacity, "mode {mode}");
    }
}

/// A globe-scale AIR line assigned a non-default aircraft (the widebody, spec 2). Two cities ~1000 km
/// apart in the world frame, two frames, a wide headway — enough to dispatch planes that fly.
fn air_world(spec: u8) -> World {
    let mut w = World::new(7, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_000_000_000, y_mm: 0, name: None }); // ~1000 km E
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: tmode::AIR, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 1_500_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// A non-default aircraft still replays bit-for-bit: same seed + same log => identical hashed state.
/// The roster must introduce no nondeterminism (it's pure integer table lookup on the hot path).
#[test]
fn non_default_aircraft_replays_deterministically() {
    let mut a = air_world(2); // widebody
    let mut b = air_world(2);
    for _ in 0..400 {
        a.tick(50);
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "non-default aircraft is determinism-safe");
    assert!(a.vehicles.len() >= 1, "air vehicles dispatched");
}

/// The assigned aircraft actually drives the per-vehicle capacity the sim boards against and the
/// render buffers expose: a widebody route carries the widebody ceiling, not the narrowbody default.
#[test]
fn assigned_aircraft_sets_vehicle_capacity() {
    let w = air_world(2); // widebody == AIR_ROSTER[2]
    let spec = w.lines[0].vehicle_spec();
    assert_eq!(spec.capacity, AIR_ROSTER[2].capacity);
    assert_ne!(spec.capacity, AIR_ROSTER[0].capacity, "differs from the default narrowbody");
}
