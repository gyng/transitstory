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
        tmode::AIR => TrainsetSpec { capacity: 250, v_max_mm_s: 200_000, accel_mm_s2: 3000, decel_mm_s2: 3000, dwell_ms: 60_000 },
        // Heavy / high-speed rail: ~300 km/h, big intercity trains, longer station dwell.
        tmode::HEAVY => TrainsetSpec { capacity: 550, v_max_mm_s: 83_000, accel_mm_s2: 700, decel_mm_s2: 900, dwell_ms: 45_000 },
        _ => SPECS[0], // rail
    }
}

/// A trainset assignment on a line. `count` is clamped at the command boundary so the
/// pre-sized SoA vehicle capacity is never exceeded (AGENTS game-design lever rule).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TrainsetAssignment {
    pub spec: u8,
    pub count: u16,
}
