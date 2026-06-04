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

/// A trainset assignment on a line. `count` is clamped at the command boundary so the
/// pre-sized SoA vehicle capacity is never exceeded (AGENTS game-design lever rule).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TrainsetAssignment {
    pub spec: u8,
    pub count: u16,
}
