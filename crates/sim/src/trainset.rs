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
    /// Physical consist length (mm). NOT a player lever — derived per mode/aircraft so the block
    /// (P1, see docs/capacity-roadmap.md) is grounded in real geometry: the follow gap is measured
    /// head-to-tail, so a longer consist occupies more of a line and (later) clears a junction
    /// slower. A 6-car metro is ~140 m; a bus a few metres; an intercity train far longer.
    pub length_mm: i64,
}

pub const SPECS: &[TrainsetSpec] = &[TrainsetSpec {
    // Metro: 80 km/h (clock), 0→top in ~24 clock-s, 21 clock-s dwell, ~140 m 6-car consist.
    capacity: 7,
    v_max_mm_s: 660_000,
    accel_mm_s2: 810_000,
    decel_mm_s2: 1_080_000,
    dwell_ms: 700,
    length_mm: 140_000,
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
        // Bus: 50 km/h (clock), nimble, 12 clock-s stops, ~12 m vehicle.
        tmode::BUS => TrainsetSpec { capacity: 3, v_max_mm_s: 420_000, accel_mm_s2: 990_000, decel_mm_s2: 1_350_000, dwell_ms: 400, length_mm: 12_000 },
        // Ferry: 40 km/h (clock), sluggish, 39 clock-s berthing, ~30 m hull.
        tmode::FERRY => TrainsetSpec { capacity: 13, v_max_mm_s: 330_000, accel_mm_s2: 360_000, decel_mm_s2: 540_000, dwell_ms: 1_300, length_mm: 30_000 },
        // Globe-scale jet. Speeds ride the same CLOCK frame as every other mode (x30/x900 in the
        // unification) so the global decay/patience constants stay coherent across modes — flights
        // remain "near-instant" by design (a half-globe hop ~ a few sim-seconds). Gate turnarounds
        // (45–120 sim-s) read as plausible 22–60 clock-minute turns; capacities keep real seat
        // counts (the globe's demand economy is its own scale).
        tmode::AIR => AIR_ROSTER[0],
        // Heavy / high-speed rail: ~300 km/h (clock), big intercity trains, 45 clock-s dwell, ~200 m.
        tmode::HEAVY => TrainsetSpec { capacity: 18, v_max_mm_s: 2_490_000, accel_mm_s2: 630_000, decel_mm_s2: 810_000, dwell_ms: 1_500, length_mm: 200_000 },
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
// AIR `length_mm` is flavour only — at globe scale (cities thousands of km apart) the block gap
// never binds, so the value is a plausible airframe length and nothing more.
pub const AIR_ROSTER: &[TrainsetSpec] = &[
    // 0 Narrowbody Jet (default) — A321/737 class: the all-round trunk single-aisle.
    TrainsetSpec { capacity: 250, v_max_mm_s: 1_800_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 60_000, length_mm: 45_000 },
    // 1 Regional Jet — E175/CRJ class: fewest seats, fastest single-door turn → keep a thin spoke frequent.
    TrainsetSpec { capacity: 88, v_max_mm_s: 1_560_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 45_000, length_mm: 30_000 },
    // 2 Widebody — 777/A350 class: big ceiling for a fat pair, slower two-aisle load (90s turn).
    TrainsetSpec { capacity: 410, v_max_mm_s: 2_160_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 90_000, length_mm: 65_000 },
    // 3 Jumbo — 747-8/A380 class: max seats for your single fattest trunk, slowest multi-deck turn (120s).
    TrainsetSpec { capacity: 525, v_max_mm_s: 2_100_000_000, accel_mm_s2: 2_700_000_000, decel_mm_s2: 2_700_000_000, dwell_ms: 120_000, length_mm: 73_000 },
];

/// RAIL rolling-stock roster — the train MODELS a player buys per line via `AssignTrainset{spec}`
/// (the depot rework's catalog, Stage 1). Index 0 is the default and is **byte-identical to `SPECS[0]`**
/// (the shipped metro), so a default route — and every existing save + both golden fixtures — replays
/// bit-for-bit (the state hash is unaffected; `rail_roster_default_is_byte_identical` pins this). Higher
/// indices trade **capacity ⇄ speed ⇄ cost** so no model is strictly best: Heavy hauls far more per trip
/// but is slower + pricier (the bulk-freight workhorse); Express is fast + cheap but light (rush a thin
/// spoke). Per-model build cost lives in `RAIL_COST` (parallel). Names/icons for the picker live in
/// the frontend `shared.ts` mirror. Other modes keep their single preset (no roster yet).
pub const RAIL_ROSTER: &[TrainsetSpec] = &[
    // 0 Standard — the shipped metro (MUST equal SPECS[0] exactly; golden-locked).
    TrainsetSpec { capacity: 7, v_max_mm_s: 660_000, accel_mm_s2: 810_000, decel_mm_s2: 1_080_000, dwell_ms: 700, length_mm: 140_000 },
    // 1 Heavy — bulk hauler: ~2× capacity, slower top speed + longer dwell (loading), longer consist.
    TrainsetSpec { capacity: 15, v_max_mm_s: 480_000, accel_mm_s2: 600_000, decel_mm_s2: 900_000, dwell_ms: 1_000, length_mm: 210_000 },
    // 2 Express — fast + light: higher top speed, short dwell, short consist, but ~half the capacity.
    TrainsetSpec { capacity: 4, v_max_mm_s: 900_000, accel_mm_s2: 1_020_000, decel_mm_s2: 1_260_000, dwell_ms: 500, length_mm: 90_000 },
];

/// Per-RAIL-model build cost ($/vehicle), parallel to `RAIL_ROSTER`. Index 0 MUST equal the global
/// `TRAIN_COST` (world.rs) so a default route's capital — and the goldens — are byte-identical. Heavy
/// costs more (you pay for the haul), Express less (light stock). Read by `recompute_line_buildability`.
pub const RAIL_COST: &[i64] = &[15_000, 27_000, 11_000];

/// Build cost ($/vehicle) for a line's chosen model. RAIL reads its roster; every other mode keeps the
/// flat `TRAIN_COST` (byte-identical — `default_train_cost` 15M passed by the caller). Out-of-range
/// spec ids clamp to the last entry (never panics).
#[inline]
pub fn train_cost(mode: u8, spec_id: u8, default_train_cost: i64) -> i64 {
    match mode {
        tmode::RAIL => RAIL_COST[(spec_id as usize).min(RAIL_COST.len() - 1)],
        _ => default_train_cost,
    }
}

/// Resolve the concrete vehicle spec for a line: its mode default, or the assigned model within
/// the mode's roster. `spec_id == 0` is ALWAYS the mode default (so a default route — and every
/// existing save — replays bit-for-bit and the state hash is unaffected). RAIL + AIR offer alternate
/// stock; other modes keep their single preset. Out-of-range ids clamp to the last roster entry.
#[inline]
pub fn spec_for(mode: u8, spec_id: u8) -> TrainsetSpec {
    match mode {
        tmode::AIR => AIR_ROSTER[(spec_id as usize).min(AIR_ROSTER.len() - 1)],
        tmode::RAIL => RAIL_ROSTER[(spec_id as usize).min(RAIL_ROSTER.len() - 1)],
        // Other modes have no roster yet — the mode preset regardless of spec id (always 0 today).
        _ => spec_for_mode(mode),
    }
}

/// Fixed block standoff (mm) a follower keeps behind the leader's TAIL on top of the dynamic
/// braking distance — the signal-reaction / safety margin even at a crawl. P1 block model
/// (docs/capacity-roadmap.md): the head-to-tail gap a train holds is `brake_distance + this`.
pub const BLOCK_MARGIN_MM: i64 = 60_000;

/// Braking distance (mm) from speed `v_mm_s` at deceleration `decel_mm_s2` = `v²/2a`. Integer
/// (i128 intermediate to avoid overflow at globe speeds); deterministic; never divides by zero.
#[inline]
pub fn brake_distance_mm(v_mm_s: i64, decel_mm_s2: i64) -> i64 {
    let v = v_mm_s.max(0) as i128;
    (v * v / (2 * decel_mm_s2.max(1) as i128)) as i64
}

/// Minimum head-to-tail block gap (mm) a train moving at `v_mm_s` must keep behind the leader's
/// tail: enough to brake to a stop, plus the fixed standoff. Used by the move-phase follow clamp
/// (dynamic, at the train's current speed) and the dispatch density cap (static, at `v_max`).
#[inline]
pub fn block_gap_mm(v_mm_s: i64, decel_mm_s2: i64) -> i64 {
    brake_distance_mm(v_mm_s, decel_mm_s2) + BLOCK_MARGIN_MM
}

/// A trainset assignment on a line. `count` is clamped at the command boundary so the
/// pre-sized SoA vehicle capacity is never exceeded (AGENTS game-design lever rule).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TrainsetAssignment {
    pub spec: u8,
    pub count: u16,
}
