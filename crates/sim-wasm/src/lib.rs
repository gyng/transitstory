//! transitstory — wasm-bindgen facade (the ONLY wasm-aware crate). A thin translation
//! membrane over `sim`: decode JSON commands, drive ticks, copy-out SoA buffers, marshal
//! stats/geometry. NO game logic, validation, or scoring lives here — those are in `sim`.
//!
//! Boundary conventions (PLAN §0): commands cross as JSON (postcard is Rust-only save);
//! i64/u64 are kept off the JS boundary (state hash as hex string, mm geometry as f64).
use sim::{CityData, Command, World};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Sim {
    world: World,
}

#[wasm_bindgen]
impl Sim {
    /// `seed` is a JS number (cast to u64) so callers never deal with BigInt. `city_json`
    /// is the committed CityData manifest (demand grid already in mm); a parse failure
    /// falls back to an empty city rather than trapping the module.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: f64, city_json: &str) -> Sim {
        let city = CityData::from_json(city_json).unwrap_or_default();
        Sim {
            world: World::new(seed as u64, city),
        }
    }

    /// Apply one JSON-encoded Command (the only write path). Returns the emitted events
    /// (assigned ids, auto-names, rejections) as a JS value. Bad JSON => thrown JS error.
    #[wasm_bindgen(js_name = applyCommandJson)]
    pub fn apply_command_json(&mut self, json: &str) -> Result<JsValue, JsValue> {
        let cmd: Command = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("invalid command JSON: {e}")))?;
        let events = self.world.apply(&cmd);
        serde_wasm_bindgen::to_value(&events).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Advance one fixed logical step (ms as a JS number; cast to i64 internally).
    pub fn tick(&mut self, dt_ms: f64) {
        self.world.tick(dt_ms as i64);
    }

    /// Canonical state hash as a hex string (avoids BigInt). The determinism oracle.
    #[wasm_bindgen(js_name = stateHash)]
    pub fn state_hash(&self) -> String {
        format!("{:016x}", self.world.state_hash())
    }

    // --- render copy-out (wasm->ts state port) ---

    #[wasm_bindgen(js_name = vehicleCount)]
    pub fn vehicle_count(&self) -> usize {
        self.world.vehicles.len()
    }

    /// Interleaved current vehicle positions `[x0,y0,x1,y1,...]` in metres (Float32Array).
    #[wasm_bindgen(js_name = vehiclePositions)]
    pub fn vehicle_positions(&self) -> Vec<f32> {
        sim::render_buf::vehicle_positions_m(&self.world)
    }

    /// Interleaved previous-tick positions in metres (for alpha interpolation).
    #[wasm_bindgen(js_name = vehiclePrevPositions)]
    pub fn vehicle_prev_positions(&self) -> Vec<f32> {
        sim::render_buf::vehicle_prev_positions_m(&self.world)
    }

    #[wasm_bindgen(js_name = vehicleAngles)]
    pub fn vehicle_angles(&self) -> Vec<f32> {
        sim::render_buf::vehicle_angles(&self.world)
    }

    #[wasm_bindgen(js_name = vehicleLineIds)]
    pub fn vehicle_line_ids(&self) -> Vec<u32> {
        sim::render_buf::vehicle_line_ids(&self.world)
    }

    /// Interleaved `[onboard, capacity]` per vehicle (Uint16Array) — the train inspector's load.
    #[wasm_bindgen(js_name = vehicleLoads)]
    pub fn vehicle_loads(&self) -> Vec<u16> {
        sim::render_buf::vehicle_loads(&self.world)
    }

    // --- structured queries (wasm->ts query port; low frequency) ---

    /// Stats readout for the bottom bar / panels (camelCase JS object, numbers not BigInt).
    pub fn stats(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.world.stats_snapshot())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Authoritative station geometry for rendering.
    #[wasm_bindgen(js_name = stationsView)]
    pub fn stations_view(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.world.stations_view())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Authoritative line geometry (ordered stops + polyline).
    #[wasm_bindgen(js_name = linesView)]
    pub fn lines_view(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.world.lines_view())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// OD "desire lines" for a selected station — its top destinations by gravity pull, for the
    /// on-selection flow overlay (read-only). Returns up to 10 `OdLink`s as a JS array.
    #[wasm_bindgen(js_name = stationOd)]
    pub fn station_od(&self, origin: u32) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.world.station_od(origin, 10))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Accessibility isochrone for a selected station — every reachable station + transit travel
    /// time, for the opt-in "Reach" overlay (read-only). Returns a JS array of `AccessLink`.
    #[wasm_bindgen(js_name = stationAccess)]
    pub fn station_access(&self, origin: u32) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.world.station_access(origin))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Inspect the `nth` waiting rider at a station — a named commuter (agent demand) or anonymous
    /// gravity trip, with their route + home/work. Returns a `JourneyView` (or null if none).
    #[wasm_bindgen(js_name = sampleJourney)]
    pub fn sample_journey(&self, station: u32, nth: u32) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&sim::journey::sample(&self.world, station, nth as usize))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Live state of a followed citizen (where they are now + journey progress), or null if they're
    /// not currently in transit. Read on the ~3 Hz tick while a citizen is being followed.
    #[wasm_bindgen(js_name = followCitizen)]
    pub fn follow_citizen(&self, citizen_id: u32) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&sim::journey::follow(&self.world, citizen_id))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Construction-cost preview ($, track only — no trains) for a hypothetical line through
    /// `station_ids` in `mode`. The build HUD's authoritative figure (same core formula as a
    /// committed line). `loop_line` closes the route. Returns a plain JS number.
    #[wasm_bindgen(js_name = previewLineCost)]
    pub fn preview_line_cost(&self, station_ids: &[u32], mode: u8, loop_line: bool) -> f64 {
        self.world.preview_line_cost(station_ids, mode, loop_line) as f64
    }
}
