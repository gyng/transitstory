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
    /// Scratch cache: the RGBA bytes produced alongside the last `peepPositions` sweep, handed back
    /// by the paired `peepColors()` call (so one sweep feeds both the position + colour attributes).
    peep_rgba: Vec<u8>,
    /// Scratch cache: the citizen id per peep from the last sweep (paired `peepCitizens()`), so a
    /// clicked peep maps back to a rider to inspect.
    peep_cit: Vec<u32>,
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
            peep_rgba: Vec::new(),
            peep_cit: Vec::new(),
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

    /// Per-vehicle dominant CARGO commodity `[k0,k1,...]` (#in-world-cargo), aligned with vehiclePositions:
    /// the good each cart hauls (0 ore / 1 grain / 2 aether / 3 fuel / 4-7 processed), or 255 if empty /
    /// a transit rider. Lets the 3D cargo block be coloured by its goods.
    #[wasm_bindgen(js_name = vehicleCargo)]
    pub fn vehicle_cargo(&self) -> Vec<f32> {
        sim::render_buf::vehicle_cargo_m(&self.world)
    }

    /// Interleaved previous-tick positions in metres (for alpha interpolation).
    #[wasm_bindgen(js_name = vehiclePrevPositions)]
    pub fn vehicle_prev_positions(&self) -> Vec<f32> {
        sim::render_buf::vehicle_prev_positions_m(&self.world)
    }

    /// Trailing CARGO CARS pulled by rail trains (#multi-car), flat across all vehicles — 6 f32 per car
    /// `[x_m,y_m,angle,commodity,load,line_id]`. Drawn as a string of cars curving behind each loco along
    /// the track. Bus/ferry/air emit none. Pair with `vehicleCarsPrev` for alpha interpolation.
    #[wasm_bindgen(js_name = vehicleCars)]
    pub fn vehicle_cars(&self) -> Vec<f32> {
        sim::render_buf::vehicle_cars_m(&self.world)
    }

    /// Previous-tick positions of the trailing cargo cars `[x0,y0,...]` in metres, aligned 1:1 (per car)
    /// with `vehicleCars` — the alpha-interpolation companion.
    #[wasm_bindgen(js_name = vehicleCarsPrev)]
    pub fn vehicle_cars_prev(&self) -> Vec<f32> {
        sim::render_buf::vehicle_cars_prev_m(&self.world)
    }

    #[wasm_bindgen(js_name = vehicleAngles)]
    pub fn vehicle_angles(&self) -> Vec<f32> {
        sim::render_buf::vehicle_angles(&self.world)
    }

    #[wasm_bindgen(js_name = vehicleLineIds)]
    pub fn vehicle_line_ids(&self) -> Vec<u32> {
        sim::render_buf::vehicle_line_ids(&self.world)
    }

    /// Interleaved marching-legion positions `[x0,y0,...]` in metres (fantasy, S8). Empty for transit
    /// (no armies). Read each frame like vehicle positions; the count is tiny (legions, capped).
    #[wasm_bindgen(js_name = armyPositions)]
    pub fn army_positions(&self) -> Vec<f32> {
        sim::render_buf::army_positions_m(&self.world)
    }

    /// Interleaved legion TARGET positions `[x0,y0,...]` in metres (fantasy, S11 — the AI general's intent),
    /// aligned with `armyPositions`. A marching legion's entry is its target town; others collapse to their
    /// own spot (zero-length arc). Lets the UI draw legion→target intent arcs. Empty for transit.
    #[wasm_bindgen(js_name = armyTargets)]
    pub fn army_targets(&self) -> Vec<f32> {
        sim::render_buf::army_targets_m(&self.world)
    }

    /// Interleaved RAIDER positions `[x0,y0,...]` in metres (fantasy, S11 — the rival). Empty for transit /
    /// a realm the rival hasn't reached. Read each frame like army positions; bounded (capped raiders).
    #[wasm_bindgen(js_name = raiderPositions)]
    pub fn raider_positions(&self) -> Vec<f32> {
        sim::render_buf::raider_positions_m(&self.world)
    }

    /// Interleaved RAIDER TARGET positions `[tx0,ty0,...]` in metres (#war — the rival's intent), aligned
    /// with `raiderPositions`. Each raider's entry is where it's HEADING (capital / supply seam / captured
    /// town), so the UI can draw the rival's intent. Empty for transit / before the rival reaches the realm.
    #[wasm_bindgen(js_name = raiderTargets)]
    pub fn raider_targets(&self) -> Vec<f32> {
        sim::render_buf::raider_targets_m(&self.world)
    }

    /// RAIDER ROLE per raider `[role0, ...]` (#war), aligned with `raiderPositions`: 0 breacher / 1 saboteur
    /// / 2 reclaimer. Lets the UI badge the three rival roles apart. Empty for transit.
    #[wasm_bindgen(js_name = raiderRoles)]
    pub fn raider_roles(&self) -> Vec<f32> {
        sim::render_buf::raider_roles_m(&self.world)
    }

    /// LEGION STATE per legion `[state0, ...]` (#war), aligned with `armyPositions`: 0 marching / 1 besieging
    /// / 2 done. Lets the UI dim inert garrisons. Empty for transit.
    #[wasm_bindgen(js_name = armyStates)]
    pub fn army_states(&self) -> Vec<f32> {
        sim::render_buf::army_states_m(&self.world)
    }

    /// Interleaved spell flashes `[x,y,kind,alpha,...]` in metres (fantasy, S11 — the spell arm). Empty
    /// otherwise. Read each frame like positions; tiny (a handful of brief flashes).
    #[wasm_bindgen(js_name = spellFlashes)]
    pub fn spell_flashes(&self) -> Vec<f32> {
        sim::render_buf::spell_flashes_m(&self.world)
    }

    /// Interleaved TTD signal markers `[x0_m, y0_m, status0, ...]` — single-track block state for the
    /// render layer. status 0 = clear (green), 1 = occupied (red), 2 = waiting (amber). Fresh Float32Array.
    #[wasm_bindgen(js_name = signalMarkers)]
    pub fn signal_markers(&self) -> Vec<f32> {
        sim::render_buf::signal_markers_m(&self.world)
    }

    /// PLAYER-PLACED block signals (TTD L5c) — the authoritative `world.signals` store as a flat
    /// `Float64Array`, 6 per signal `[line, path, span, at_mm, x_m, y_m]`. Distinct from `signalMarkers`
    /// (the per-tick occupancy readout): these are the posts the player dropped, so the UI draws them +
    /// hit-tests a click to remove one. `at_mm`/ids ride as f64 (exact integers < 2^53 ⇒ lossless round-trip
    /// back into a `RemoveSignal`); positions are local metres. Read on the ~3 Hz / on-change cadence.
    #[wasm_bindgen(js_name = placedSignals)]
    pub fn placed_signals(&self) -> Vec<f64> {
        sim::render_buf::placed_signals_f64(&self.world)
    }

    /// Interleaved `[onboard, capacity]` per vehicle (Uint16Array) — the train inspector's load.
    #[wasm_bindgen(js_name = vehicleLoads)]
    pub fn vehicle_loads(&self) -> Vec<u16> {
        sim::render_buf::vehicle_loads(&self.world)
    }

    /// Interleaved decadence-tide cells `[x0_m,y0_m,v0,...]` (fantasy S10c) — corrupted CA cells in
    /// metres + 0..1 strength, for the cold-tide overlay. Empty for transit / before the tide starts.
    #[wasm_bindgen(js_name = decadenceTide)]
    pub fn decadence_tide(&self) -> Vec<f32> {
        sim::render_buf::decadence_tide_m(&self.world)
    }

    /// Derived TrackGraph segments (TTD L1) as flat polylines: per segment `[n_pts, shared, x0,y0, …]` in
    /// metres — the shared-INFRASTRUCTURE render layer (a co-located corridor draws as one rail). Empty for
    /// continuous / non-grid networks. Render-only (the graph is never hashed).
    #[wasm_bindgen(js_name = trackGraph)]
    pub fn track_graph(&self) -> Vec<f32> {
        sim::render_buf::track_graph_m(&self.world)
    }

    /// Render-only "peep" dots (individual riders). One sweep at interpolation `alpha` (0..1) with
    /// the render `tick_ms` (for smooth walk motion) returns interleaved `[x0,y0,...]` metres and
    /// CACHES the paired RGBA, fetched by `peepColors()`. Determinism-free (no hashed state read or
    /// written). Capped at `MAX_VISIBLE_PEEPS`, so cost is bounded regardless of network size.
    #[wasm_bindgen(js_name = peepPositions)]
    pub fn peep_positions(&mut self, alpha: f32, tick_ms: f32) -> Vec<f32> {
        let (xy, rgba, cit) = sim::render_buf::fill_peeps(&self.world, alpha, tick_ms);
        self.peep_rgba = rgba;
        self.peep_cit = cit;
        xy
    }

    /// RGBA bytes (Uint8Array, 4 per peep) for the peeps from the LAST `peepPositions` call. Must be
    /// read immediately after it (the two calls share one sweep) — the frontend always pairs them.
    #[wasm_bindgen(js_name = peepColors)]
    pub fn peep_colors(&self) -> Vec<u8> {
        self.peep_rgba.clone()
    }

    /// Citizen id (Uint32Array, 1 per peep, index-aligned with `peepPositions`) behind each peep from
    /// the LAST sweep, or `u32::MAX` for an anonymous gravity rider. Lets the frontend map a clicked
    /// peep back to a rider to inspect/follow. Determinism-free (render-derived, no hashed state).
    #[wasm_bindgen(js_name = peepCitizens)]
    pub fn peep_citizens(&self) -> Vec<u32> {
        self.peep_cit.clone()
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

    /// Walk shed for a selected station — the buildability cells it actually reaches on foot
    /// (water severs, crossed corridors pinch), each with a decay intensity, for the lopsided
    /// catchment overlay (read-only). Empty when the city has no raster. Returns `ShedCell[]`.
    #[wasm_bindgen(js_name = stationWalkshed)]
    pub fn station_walkshed(&self, origin: u32) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.world.station_walkshed(origin))
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
