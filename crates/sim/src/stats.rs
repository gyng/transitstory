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
    /// Surface-rail build impact: 0 (all grade-separated / following ROW) .. 100 (heavy surface
    /// cutting through built-up land). Lower is better.
    pub build_difficulty: u8,
    /// Economy (dollars). `balance` = start budget + fares − capital; informational if economy off.
    pub economy_enabled: bool,
    pub balance: f64,
    pub capital_spent: f64,
    pub fare_revenue: f64,
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
    pub headway_ms: f64,
    pub disruption: f64,
    pub crosses_water: bool,
    pub capital_cost: f64,
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
}
