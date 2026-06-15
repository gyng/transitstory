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
    // --- fantasy (arcadia) read-out; all 0/false for transit, so the field is mode-agnostic ---
    /// The canonicalised ruleset tag ("transit" | "arcadia") — lets the HUD pick the mode-appropriate
    /// readout (tribute/decadence vs riders/coverage) from the snapshot alone.
    pub ruleset: String,
    /// GOLD — the universal war-chest (every delivery mints it; funds legions + general tech). Named
    /// `tribute` for back-compat (the channel split kept gold's volume identical). The S11 split adds:
    pub tribute: f64,
    /// MANA — minted by AETHER chains; funds arcane tech (e.g. Sappers). 0 until aether is delivered.
    pub mana: f64,
    /// MANPOWER — minted by INGOT/ARMS chains; funds military tech (e.g. Conscription). 0 until arms flow.
    pub manpower: f64,
    /// Spreading-corruption pressure (the lose meter); `realm_lost` once it reaches the capital.
    pub decadence: f64,
    /// Decadence as a 0–100 fraction of the capital threshold — the lose-meter gauge fill.
    pub decadence_pct: f64,
    /// Towns conquered this game (the conquest score).
    pub towns_captured: f64,
    /// Legions currently fielded (the war machine's mobile force).
    pub army_count: u32,
    /// Decadence RAIDERS marching (the rival, S11) — the mobile enemy your network must cut down. 0 for
    /// transit + a realm with no decadence field.
    pub raider_count: u32,
    /// True once decadence has overrun the capital — the realm has fallen.
    pub realm_lost: bool,
    /// Unlocked-tech bitset (S11) — bit `TECHS[id].bit` set ⇒ that upgrade is active. The HUD reads it
    /// to render each tech as locked / affordable / unlocked (cost vs `mana`).
    pub tech_unlocked: u32,
    /// Cumulative spells cast (S11 spell arm) — the HUD shows the realm's magic is active once SPELLCRAFT
    /// is unlocked. 0 for transit / a realm without the spell arm.
    pub spells_cast: u32,
    /// AUTOCAST toggle state (S11) — the HUD's spell-bar checkbox reflects it. False (manual cast) by
    /// default; always false for transit.
    pub autocast: bool,
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
    /// Remaining siege resistance — a town's FRONTIER garrison (S11), grinding down under siege; 0 once
    /// captured (or for a non-town / before the war ticks). The HUD shows it for sink (town) stations.
    pub town_resistance: f64,
    /// Forge-Line BUFFER fill 0..1 (fantasy #8): the fullest of this node's commodity buffers / BUFFER_CAP.
    /// A backed-up SOURCE reads ~1 (ship it!), a starved SINK ~0. 0 for transit / non-forge. Derived from
    /// the hashed `forge_stock` for the HUD's node buffer pips — NOT hashed itself (it's a snapshot readout).
    pub buffer_fill: f64,
    /// Fantasy (arcadia) #9: this station is a CAPTURED HOLDING — a town conquest flipped (its garrison
    /// ground to 0). The EXACT mirror of `World::buildable_at`'s per-town test (`town_value == Some(0)`),
    /// so the realm-border overlay matches the gate: **false** before the war ticks (resistance not yet
    /// initialised) and for still-neutral towns / player stations; true only once a town actually falls.
    /// A snapshot readout (NOT hashed); false for transit. The capital is a holding via the baked seat, not
    /// this flag (its garrison never reaches 0), so the overlay adds it separately — no double-count.
    pub captured: bool,
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
    /// Posted bounty on this town (fantasy, S8 steering) — >0 draws a marker so the player SEES where
    /// they've baited the legions. 0 for transit + un-bountied towns. Render read-out, not hashed.
    pub bounty: f64,
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
    /// Trunk polyline vertices in mm `[[x,y], ...]` in stop order.
    pub polyline_mm: Vec<[f64; 2]>,
    /// One polyline per BRANCH path (paths[1..]); each is the full trunk-prefix→branch track in mm.
    /// Drawn in the line's colour alongside the trunk so a Y-shaped line shows its spur (P3).
    #[serde(default)]
    pub branch_polylines_mm: Vec<Vec<[f64; 2]>>,
    /// Per branch: the uniform build mode of its OWN spans (0=Surface,1=Elevated,2=Tunnel), or -1 if
    /// mixed. For the Editor's per-branch Track control.
    #[serde(default)]
    pub branch_modes: Vec<i32>,
    /// Per branch: its terminus station id (the last stop), for the "→ <name>" label.
    #[serde(default)]
    pub branch_termini: Vec<u32>,
    /// Tightest curve radius (mm) on the line; large value == effectively straight.
    pub min_radius_mm: f64,
    /// Build mode per inter-stop span (0=Surface,1=Elevated,2=Tunnel).
    pub span_modes: Vec<u8>,
    pub crosses_water_surface: bool,
    /// Track type per inter-stop span (0=Double,1=Single; P2) — the trunk's, for the Editor toggle.
    #[serde(default)]
    pub track_types: Vec<u8>,
    /// Tombstoned (bulldozed): kept for index-stable ids, but the frontend skips rendering it.
    pub removed: bool,
}
