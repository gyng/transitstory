//! Low-frequency structured readout (the wasm->ts query port). Numerics are f64/u32 so
//! they marshal as plain JS numbers, never BigInt. Ridership/waiting/coverage and the
//! passenger-lifecycle telemetry (avg journey/wait, denied boardings) are computed live in
//! `World::stats_snapshot`; per-line colour comes straight from the command-sourced state.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSnapshot {
    pub sim_clock_ms: f64,
    pub running: bool,
    pub station_count: u32,
    pub line_count: u32,
    pub vehicle_count: u32,
    pub ridership_total: f64,
    pub waiting_total: f64,
    /// Cumulative "left behind" = times a rider was passed by a full vehicle (== denied_boardings).
    pub left_behind: f64,
    pub denied_boardings: f64,
    /// Cumulative riders who gave up waiting (renege) — the frequency/coverage pressure signal.
    pub abandoned: f64,
    /// Average end-to-end trip time (ms) over completed trips; 0 before the first arrival.
    pub avg_journey_ms: f64,
    /// Average platform wait (ms) per boarding; 0 before the first boarding.
    pub avg_wait_ms: f64,
    pub avg_load_factor: f32,
    pub coverage_score: u8,
    /// Time-of-day: in-game hour [0,24), period label, and the current demand multiplier.
    pub sim_hour: f64,
    pub period: String,
    pub demand_multiplier: f64,
    /// In-game day index (clock / 24h, from 0) — the frontend's day-rollover beat keys off this
    /// instead of hand-mirroring HOUR_MS.
    pub sim_day: u32,
    /// Total origin demand across the WHOLE city grid right now — the coverage denominator.
    /// Grows under transit-oriented growth; the day report diffs it to say "the city grew".
    pub demand_origin_total: f64,
    /// Surface-rail build impact: 0 (all grade-separated / following ROW) .. 100 (heavy surface
    /// cutting through built-up land). Lower is better.
    pub build_difficulty: u8,
    /// Economy (dollars). `balance` = start budget + fares − capital; informational if economy off.
    pub economy_enabled: bool,
    pub balance: f64,
    pub capital_spent: f64,
    pub fare_revenue: f64,
    /// Cumulative recurring maintenance charged (opex); 0 unless the economy is enabled.
    pub opex_spent: f64,
    pub per_station: Vec<StationStat>,
    pub per_line: Vec<LineStat>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationStat {
    pub station_id: u32,
    pub boardings: f64,
    pub alightings: f64,
    pub waiting: f64,
    /// Captured gravity demand at this station: resident/trip-origin weight and job/destination
    /// weight pulled from the demand grid (the figures that drive `coverage_score`). Surfaced
    /// per-station so the map can show which stations actually *grab* demand vs sit on empty land.
    pub demand_origin: f64,
    pub demand_dest: f64,
    /// Operational lines serving this station (trainset + ≥2 stops). 0 = no service ("orphaned").
    pub serving: u32,
    /// Cumulative pressure AT THIS STATION: riders passed by a full vehicle (`denied`) and riders
    /// who gave up waiting (`abandoned`). The precise "this platform is failing" signal — the
    /// global `denied_boardings`/`abandoned` totals bucketed to where the loss actually happened.
    pub denied: f64,
    pub abandoned: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineStat {
    pub line_id: u32,
    pub name: String,
    pub mode: u8,
    pub color: u32,
    pub ridership: f64,
    pub stops: u32,
    pub trains: u32,
    /// The assigned roster entry (meaningful for AIR's aircraft ladder; 0 = the mode default).
    pub trainset_spec: u8,
    pub headway_ms: f64,
    pub disruption: f64,
    pub crosses_water: bool,
    pub capital_cost: f64,
    /// Mean load factor (onboard / capacity) across this line's vehicles, 0..~1+. The inspect
    /// strain readout — distinct from `ridership` (throughput): a line can move many riders and
    /// still be uncrowded, or move few and be at crush load. 0 when the line has no vehicles.
    pub load_factor: f32,
}

/// One OD "desire line" from a selected origin station to a destination it draws riders toward
/// (gravity pull). `weight` is normalized 0..1 against the strongest link, for the on-selection
/// ArcLayer overlay ("where do people here want to go"). mm coords as f64 (no BigInt; geo.ts maps).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OdLink {
    pub dest: u32,
    pub x_mm: f64,
    pub y_mm: f64,
    pub weight: f32,
}

/// One reachable station in the accessibility isochrone from a selected origin: how fast transit
/// gets there (`ms`, wait + ride + transfers via `Router::reachable`). For the opt-in "Reach"
/// overlay — colour stations green→amber→red by travel time from the pinned one.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessLink {
    pub station: u32,
    pub x_mm: f64,
    pub y_mm: f64,
    pub ms: f64,
}

/// One buildability cell reachable on foot from a selected station, for the lopsided walk-shed
/// overlay (cell centre in mm; `intensity` 0..1 is the distance-decay weight → fill alpha, so the
/// shed fades out toward its edge). Barriers (water, crossed corridors) simply omit cells, so the
/// rendered hexagon set IS the real catchment — not a circle. Empty when the city has no raster.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShedCell {
    pub x_mm: f64,
    pub y_mm: f64,
    pub intensity: f32,
}

// Geometry views (wasm->ts query port). mm coords are f64 (exact for city-scale ints,
// no BigInt at the boundary); the frontend converts mm -> lng/lat in coords/geo.ts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationView {
    pub id: u32,
    pub x_mm: f64,
    pub y_mm: f64,
    pub name: String,
    /// Tombstoned (bulldozed): kept for index-stable ids, but the frontend skips rendering it.
    pub removed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineView {
    pub id: u32,
    pub name: String,
    pub mode: u8,
    pub loop_line: bool,
    pub color: u32,
    pub stops: Vec<u32>,
    /// Polyline vertices in mm `[[x,y], ...]` in stop order.
    pub polyline_mm: Vec<[f64; 2]>,
    /// Tightest curve radius (mm) on the line; large value == effectively straight.
    pub min_radius_mm: f64,
    /// Build mode per inter-stop span (0=Surface,1=Elevated,2=Tunnel).
    pub span_modes: Vec<u8>,
    pub crosses_water_surface: bool,
    /// Tombstoned (bulldozed): kept for index-stable ids, but the frontend skips rendering it.
    pub removed: bool,
}
