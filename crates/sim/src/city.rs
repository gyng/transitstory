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
    /// Coarse buildability grid (mm) for the surface-rail cost signal. Additive/optional.
    #[serde(default)]
    pub buildability: BuildabilityGrid,
    /// How long (sim ms) a waiting rider tolerates before giving up (renege). A per-city
    /// demand knob, NOT a hardcoded constant. Cities loaded from JSON without the field get
    /// `default_patience_ms`; `CityData::default()` leaves it 0, which DISABLES renege (handy
    /// for native tests that don't want abandonment).
    #[serde(default = "default_patience_ms")]
    pub patience_ms: i64,
    /// Maximum legs (transfers + 1) a routed trip may use. 0 (the `Default` value) means "use
    /// the routing default" — so it's a per-city knob a future RAPTOR can raise without a core
    /// change, while `CityData::default()` keeps the shipped behaviour.
    #[serde(default)]
    pub max_legs: usize,
}

/// Default rider patience for cities that don't specify one: 30 sim-minutes.
fn default_patience_ms() -> i64 {
    1_800_000
}

/// Coarse classified grid: each cell carries a class `c` (1=RoadROW 2=RailROW 3=Built
/// 4=Water 5=Park; absent cells are Open=0). Built offline from OSM (scripts/build_buildability.py).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BuildabilityGrid {
    #[serde(default)]
    pub cell_m: f64,
    #[serde(default)]
    pub cells: Vec<BuildCell>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildCell {
    pub x_mm: i64,
    pub y_mm: i64,
    pub c: u8,
}

/// Surface-rail cost class codes (shared by the grid + the sim).
pub mod class {
    pub const OPEN: u8 = 0;
    pub const ROAD: u8 = 1;
    pub const RAIL: u8 = 2;
    pub const BUILT: u8 = 3;
    pub const WATER: u8 = 4;
    pub const PARK: u8 = 5;
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
