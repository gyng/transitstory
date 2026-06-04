//! Low-frequency structured readout (the wasm->ts query port). Numerics are f64/u32 so
//! they marshal as plain JS numbers, never BigInt. Ridership/waiting/coverage stay 0
//! until T16; counts + per-line colour are populated now so the UI has something to bind.
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
    pub left_behind: f64,
    pub avg_load_factor: f32,
    pub coverage_score: u8,
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
    pub color: u32,
    pub ridership: f64,
    pub stops: u32,
    pub trains: u32,
    pub headway_ms: f64,
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
    pub color: u32,
    pub stops: Vec<u32>,
    /// Polyline vertices in mm `[[x,y], ...]` in stop order.
    pub polyline_mm: Vec<[f64; 2]>,
}
