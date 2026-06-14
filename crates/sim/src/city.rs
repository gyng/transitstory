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
    /// Which ruleset the engine constructs for this city — the fantasy-fork seam
    /// (fantasy-fork.md). Frozen at construction, NEVER a Command. `"transit"` (the serde
    /// default for any city JSON omitting it) selects the classic gravity/agent transit game;
    /// future `"arcadia"` selects the hex 4X-logistics ruleset. `CityData::default()` leaves it
    /// empty (`""`), which `World::new` treats identically to `"transit"`, so every existing
    /// city and native test is byte-identical (zero re-pins). NOT part of `state_hash` — it is a
    /// construction-time selector, not evolving state.
    #[serde(default = "default_ruleset")]
    pub ruleset: String,
    #[serde(default)]
    pub seed: u64,
    /// Demand grid in sim millimetres (the frontend converts lon/lat -> mm before embedding).
    #[serde(default)]
    pub demand: DemandGrid,
    /// Coarse buildability grid (mm) for the surface-rail cost signal. Additive/optional.
    #[serde(default)]
    pub buildability: BuildabilityGrid,
    /// GRID geometry mode (fantasy-fork.md §10 / shared-rail.md). When > 0, track is built on a
    /// `grid_cell_mm` lattice (crisp octilinear, integer-exact) instead of the continuous Catmull-Rom
    /// curve, so two lines over the same physical corridor produce byte-identical edges (the
    /// foundation for cross-line shared track). A bake property frozen at construction, NOT a Command.
    /// **0 = off (continuous)** ⇒ every existing city builds the exact same geometry (zero re-pins).
    #[serde(default)]
    pub grid_cell_mm: i64,
    /// Fantasy (arcadia): the baked STARTING decadence — the ambient corruption the realm begins with,
    /// seeded by `scripts/build_world.py` S4 from the per-town floors (a more-corrupt continent starts
    /// further up the lose meter ⇒ more urgency). A bake property frozen at construction, NOT a Command;
    /// `World::new` seeds `world.decadence` from it. **0 (the serde + `Default` value) ⇒ the realm starts
    /// clean** — every existing city, the arcadia golden fixture, and every native test stay byte-identical
    /// (zero re-pins); only a baked world that sets it starts corrupt.
    #[serde(default)]
    pub initial_decadence: i64,
    /// Fantasy (arcadia) S7e/balance: decadence GROWTH per sim-second (the lose-meter fill rate). A
    /// per-city knob (the large baked continent presses far gentler than the small demo). **0 (serde +
    /// `Default`) ⇒ the `decadence::BASE_GROWTH_PER_S` default**, so every existing city + the golden
    /// fixtures + native tests keep the shipped (demo) balance — byte-identical.
    #[serde(default)]
    pub decadence_growth_per_s: i64,
    /// Fantasy (arcadia) balance: legion MARCH speed in mm per sim-second (a legion's pace riding the
    /// rails). A per-city knob — the large baked continent (towns 60+ km from the capital) needs a far
    /// faster legion than the 1.5 km demo, or conquest can't reach a town before the rot overruns the
    /// realm. **0 (serde + `Default`) ⇒ the `army::ARMY_SPEED_MM_S` default**, so the demo, the arcadia
    /// golden fixture, and every native test keep the shipped (demo) balance — byte-identical (with the
    /// default the marching `s_mm` trajectory is unchanged; only a baked world that sets it diverges).
    #[serde(default)]
    pub army_speed_mm_s: i64,
    /// Fantasy (arcadia) S10: the baked CAPITAL cell in sim mm (the seat the decadence tide races toward
    /// — the lose target + the creep-gradient origin). Seeded by `scripts/build_world.py` from the
    /// capital town. **(0, 0) (the serde + `Default` value) ⇒ no capital ⇒ no decadence CA** — every
    /// transit city, the arcadia golden fixture, and native tests build an empty `DecadenceField`
    /// (golden-neutral; the field is static/un-hashed anyway). A pure construction-time map property,
    /// NOT a Command.
    #[serde(default)]
    pub capital_x_mm: i64,
    #[serde(default)]
    pub capital_y_mm: i64,
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
    /// Demand growth per in-game day, in basis points (250 = +2.5%), applied to cells within a
    /// catchment of a SERVED station — transit-oriented development: the city grows where (and
    /// because) you serve it. Cells outside any catchment grow at a third of this rate (ambient
    /// sprawl), so a network you stop extending slowly falls behind the city. The pressure this
    /// creates is the one-more-day engine: growth is good news (riders, coverage) that creates
    /// problems (queues, crush load). Cities loaded from JSON without the field get
    /// `default_growth_bp`; `CityData::default()` leaves it 0, which DISABLES growth (native
    /// tests opt in explicitly).
    #[serde(default = "default_growth_bp")]
    pub growth_bp_per_day: i64,
}

/// Default transit-adjacent demand growth: +2.5% per in-game day (ambient = a third of this).
fn default_growth_bp() -> i64 {
    250
}

/// The ruleset a city JSON selects when it omits `ruleset` — the classic transit game. Shared by
/// both `CityData` and `SaveGame` so a save and the city it was played on agree on the seam. Note
/// `CityData::default()` (the native-test constructor) still yields `""` via `#[derive(Default)]`;
/// `World::new` canonicalises `"" | "transit"` to the transit ruleset, so both spell the same game.
pub fn default_ruleset() -> String {
    "transit".to_string()
}

/// Default rider patience for cities that don't specify one: 10 CLOCK-minutes (20_000 sim-ms in
/// the unified frame) — about two missed trains at a typical 3–6 clock-minute headway. Patience
/// arms the renege ("gave up waiting") pressure signal, the game's primary difficulty source; a
/// patience several times the longest sane headway would silently disable it.
fn default_patience_ms() -> i64 {
    20_000
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
    /// Fantasy (arcadia) S7e: which Forge-Line commodity this cell produces/consumes (ORE=0, GRAIN=1,
    /// AETHER=2, FUEL=3 …). A source station's output commodity is the dominant origin-commodity of its
    /// captured cells. **0 (the serde + `Default` value) ⇒ ORE** — every transit city + the golden
    /// fixtures stay byte-identical (transit has no forge), so this is a transit-neutral addition.
    #[serde(default)]
    pub commodity: u8,
}

impl CityData {
    /// Parse from the JSON string passed across the wasm boundary.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
