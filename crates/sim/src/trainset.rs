//! Trainset specs. The slice ships one fixed type; `spec: u8` selects from this table
//! so more types are an additive data change (not a structural one).
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TrainsetSpec {
    pub capacity: u16,
    pub v_max_mm_s: i64,  // top speed (mm/s) ~ 22 m/s ≈ 80 km/h
    pub accel_mm_s2: i64, // acceleration (mm/s^2)
    pub decel_mm_s2: i64, // braking (mm/s^2)
    pub dwell_ms: i64,    // station dwell time
}

pub const SPECS: &[TrainsetSpec] = &[TrainsetSpec {
    capacity: 200,
    v_max_mm_s: 22_000,
    accel_mm_s2: 900,
    decel_mm_s2: 1_200,
    dwell_ms: 20_000,
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
pub fn spec_for_mode(mode: u8) -> TrainsetSpec {
    match mode {
        tmode::BUS => TrainsetSpec { capacity: 80, v_max_mm_s: 14_000, accel_mm_s2: 1100, decel_mm_s2: 1500, dwell_ms: 12_000 },
        tmode::FERRY => TrainsetSpec { capacity: 400, v_max_mm_s: 11_000, accel_mm_s2: 400, decel_mm_s2: 600, dwell_ms: 40_000 },
        // Globe-scale jet (cities sit thousands of km apart, so AIR cruises far faster than ground
        // modes). The mode default is roster index 0 — the narrowbody — and AIR additionally offers
        // a roster of alternate aircraft (regional/widebody/jumbo) selectable per route via `spec_for`.
        tmode::AIR => AIR_ROSTER[0],
        // Heavy / high-speed rail: ~300 km/h, big intercity trains, longer station dwell.
        tmode::HEAVY => TrainsetSpec { capacity: 550, v_max_mm_s: 83_000, accel_mm_s2: 700, decel_mm_s2: 900, dwell_ms: 45_000 },
        _ => SPECS[0], // rail
    }
}

/// AIR aircraft roster — the stock a player picks from per route via `AssignTrainset{spec}`.
/// Index 0 is the locked default (byte-identical to the historical single AIR preset) so existing
/// saves and the determinism hash are unchanged. Higher indices trade **capacity** against ground
/// **turnaround** (`dwell_ms`): a bigger jet fills more of a fat city-pair per departure but sits
/// longer at the gate, widening effective headway for a fixed fleet — so no aircraft is strictly
/// best (capacity and dwell rise in lockstep). Speed is secondary flavor only (a hop is near-instant
/// at globe scale). Names/icons for the picker live in the frontend `shared.ts` roster mirror.
pub const AIR_ROSTER: &[TrainsetSpec] = &[
    // 0 Narrowbody Jet (default) — A321/737 class: the all-round trunk single-aisle.
    TrainsetSpec { capacity: 250, v_max_mm_s: 60_000_000, accel_mm_s2: 3_000_000, decel_mm_s2: 3_000_000, dwell_ms: 60_000 },
    // 1 Regional Jet — E175/CRJ class: fewest seats, fastest single-door turn → keep a thin spoke frequent.
    TrainsetSpec { capacity: 88, v_max_mm_s: 52_000_000, accel_mm_s2: 3_000_000, decel_mm_s2: 3_000_000, dwell_ms: 45_000 },
    // 2 Widebody — 777/A350 class: big ceiling for a fat pair, slower two-aisle load (90s turn).
    TrainsetSpec { capacity: 410, v_max_mm_s: 72_000_000, accel_mm_s2: 3_000_000, decel_mm_s2: 3_000_000, dwell_ms: 90_000 },
    // 3 Jumbo — 747-8/A380 class: max seats for your single fattest trunk, slowest multi-deck turn (120s).
    TrainsetSpec { capacity: 525, v_max_mm_s: 70_000_000, accel_mm_s2: 3_000_000, decel_mm_s2: 3_000_000, dwell_ms: 120_000 },
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
