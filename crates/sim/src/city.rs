//! The city manifest the core deserializes from `Sim::new(seed, city_json)`. The core
//! reads ONLY what it needs in sim-space (the demand grid in mm + seed); the frontend's
//! copy of CityData additionally carries lng/lat origin/bbox/center for the map. Unknown
//! JSON fields (originLngLat, bbox, ...) are ignored by serde, keeping lng/lat out of the
//! core. A future GTFS importer maps into the same types — additive, not a refactor.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CityData {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub seed: u64,
    /// Demand grid in sim millimetres (the frontend converts lon/lat -> mm before embedding).
    #[serde(default)]
    pub demand: DemandGrid,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DemandGrid {
    /// Grid cell size in metres (metadata; cells already carry mm positions).
    #[serde(default)]
    pub cell_m: f64,
    #[serde(default)]
    pub cells: Vec<DemandCell>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DemandCell {
    pub x_mm: i64,
    pub y_mm: i64,
    /// Trip-origin weight (residents) and trip-destination weight (jobs/POIs).
    pub origin_w: f32,
    pub dest_w: f32,
}

impl CityData {
    /// Parse from the JSON string passed across the wasm boundary.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
