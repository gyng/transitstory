//! Trainset specs. The slice ships one fixed type; `spec: u8` selects from this table
//! so more types are an additive data change (not a structural one).
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TrainsetSpec {
    /// Seats per vehicle. CLOCK-FRAME SCALE (Mini-Metro-style): rescaled ÷CLOCK_SCALE alongside
    /// the speed unification, so trips/day × riders/trip — and with them load factors, queues,
    /// fares and opex trajectories — match the pre-unification tuning without touching spawn
    /// rates. A "full train" is ~7 riders; denied-boarding pressure works the same.
    pub capacity: u16,
    /// Top speed (mm per sim-second), CLOCK-FRAME: ×CLOCK_SCALE vs the real-world figure, so the
    /// nameplate speed is what the in-game clock observes (660_000 ⇒ 80 km per CLOCK hour).
    pub v_max_mm_s: i64,
    /// Acceleration/braking (mm/s²), ×CLOCK_SCALE² — braking distance v²/2a is frame-invariant,
    /// so stopping behaviour over real metres is unchanged.
    pub accel_mm_s2: i64,
    pub decel_mm_s2: i64,
    /// Station dwell (sim-ms) — reads true on the clock (700 sim-ms = 21 clock-seconds).
    pub dwell_ms: i64,
}

pub const SPECS: &[TrainsetSpec] = &[TrainsetSpec {
    // Metro: 80 km/h (clock), 0→top in ~24 clock-s, 21 clock-s dwell.
    capacity: 7,
    v_max_mm_s: 660_000,
    accel_mm_s2: 810_000,
    decel_mm_s2: 1_080_000,
    dwell_ms: 700,
}];

#[inline]
pub fn spec(spec_id: u8) -> TrainsetSpec {
    SPECS[(spec_id as usize).min(SPECS.len() - 1)]
}

/// Transport modes (distinct from per-span BUILD modes in line::mode). RAIL is regular/metro
/// rapid transit; HEAVY is mainline / high-speed rail (HSR) — fast, high-capacity, intercity.
pub mod tmode {
    pub const RAIL: u8 = 0;
    pub const BUS: u8 = 1;
    pub const FERRY: u8 = 2;
    pub const AIR: u8 = 3;
    pub const HEAVY: u8 = 4;
}

/// Vehicle preset per transport mode (speed/capacity/dwell differ; the sim path is shared).
/// All ground/water modes are CLOCK-FRAME (see `TrainsetSpec` field docs): speeds ×CLOCK_SCALE,
/// accel ×CLOCK_SCALE², dwell and capacity ÷CLOCK_SCALE vs their pre-unification values.
pub fn spec_for_mode(mode: u8) -> TrainsetSpec {
    match mode {
        // Bus: 50 km/h (clock), nimble, 12 clock-s stops.
        tmode::BUS => TrainsetSpec { capacity: 3, v_max_mm_s: 420_000, accel_mm_s2: 990_000, decel_mm_s2: 1_350_000, dwell_ms: 400 },
        // Ferry: 40 km/h (clock), sluggish, 39 clock-s berthing.
        tmode::FERRY => TrainsetSpec { capacity: 13, v_max_mm_s: 330_000, accel_mm_s2: 360_000, decel_mm_s2: 540_000, dwell_ms: 1_300 },
        // Globe-scale jet. Speeds ride the same CLOCK frame as every other mode (x30/x900 in the
        // unification) so the global decay/patience constants stay coherent across modes — flights
        // remain "near-instant" by design (a half-globe hop ~ a few sim-seconds). Gate turnarounds
        // (45–120 sim-s) read as plausible 22–60 clock-minute turns; capacities keep real seat
        // counts (the globe's demand economy is its own scale).
        tmode::AIR => AIR_ROSTER[0],
        // Heavy / high-speed rail: ~300 km/h (clock), big intercity trains, 45 clock-s dwell.
        tmode::HEAVY => TrainsetSpec { capacity: 18, v_max_mm_s: 2_490_000, accel_mm_s2: 630_000, decel_mm_s2: 810_000, dwell_ms: 1_500 },
        _ => SPECS[0], // rail
    }
}

/// AIR aircraft roster — the stock a player picks from per route via `AssignTrainset{spec}`.
/// Index 0 is the default. (The pre-unification byte-lock on this entry was superseded by the
/// clock-frame migration, which is an explicit sim-version bump for saves.) Higher indices trade **capacity** against ground
/// **turnaround** (`dwell_ms`): a bigger jet fills more of a fat city-pair per departure but sits
/// longer at the gate, widening effective headway for a fixed fleet — so no aircraft is strictly
/// best (capacity and dwell rise in lockstep). Speed is secondary flavor only (a hop is near-instant
/// at globe scale). Names/icons for the picker live in the frontend `shared.ts` roster mirror.
pub const AIR_ROSTER: &[TrainsetSpec] = &[
    // 0 Narrowbody Jet (default) — A321/737 class: the all-round trunk single-aisle.
    TrainsetSpec { capacity: 250, v_max_mm_s: 1_800_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 60_000 },
    // 1 Regional Jet — E175/CRJ class: fewest seats, fastest single-door turn → keep a thin spoke frequent.
    TrainsetSpec { capacity: 88, v_max_mm_s: 1_560_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 45_000 },
    // 2 Widebody — 777/A350 class: big ceiling for a fat pair, slower two-aisle load (90s turn).
    TrainsetSpec { capacity: 410, v_max_mm_s: 2_160_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 90_000 },
    // 3 Jumbo — 747-8/A380 class: max seats for your single fattest trunk, slowest multi-deck turn (120s).
    TrainsetSpec { capacity: 525, v_max_mm_s: 2_100_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 120_000 },
];

/// Resolve the concrete vehicle spec for a line: its mode default, or the assigned aircraft within
/// the mode's roster. `spec_id == 0` is ALWAYS the mode default (so a default route — and every
/// existing save — replays bit-for-bit and the state hash is unaffected); only AIR currently offers
/// alternate stock. Out-of-range ids clamp to the last roster entry (never panics on the hot path).
#[inline]
pub fn spec_for(mode: u8, spec_id: u8) -> TrainsetSpec {
    match mode {
        tmode::AIR => AIR_ROSTER[(spec_id as usize).min(AIR_ROSTER.len() - 1)],
        // Other modes have no roster yet — the mode preset regardless of spec id (always 0 today).
        _ => spec_for_mode(mode),
    }
}

/// A trainset assignment on a line. `count` is clamped at the command boundary so the
/// pre-sized SoA vehicle capacity is never exceeded (AGENTS game-design lever rule).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TrainsetAssignment {
    pub spec: u8,
    pub count: u16,
}
