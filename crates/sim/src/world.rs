//! The deterministic world state and its two pure entry points: `apply(Command)` for
//! mutations and `tick(dt_ms)` for advancement. Holds the seeded RNG, the command log,
//! and `state_hash()` for the determinism test. No clock/thread/HashMap-iteration/float
//! in state-affecting paths.
use crate::city::CityData;
use crate::command::{Command, Event};
use crate::geo_local::PointMm;
use crate::hash::fnv1a;
use crate::ids::{LineId, StationId};
use crate::line::Line;
use crate::station::Station;
use crate::stats::{AccessLink, LineStat, LineView, OdLink, StationStat, StatsSnapshot, StationView};
use crate::tick;
use crate::trainset::TrainsetAssignment;
use crate::vehicle::VehicleSoA;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Frequency/capacity guardrails. `count` is clamped so the (future) pre-sized SoA
/// vehicle buffers can never be exceeded; headway has a sane floor.
pub const MAX_TRAINS_PER_LINE: u16 = 24;
pub const MIN_HEADWAY_MS: i64 = 30_000; // 30 s
pub const MAX_HEADWAY_MS: i64 = 1_800_000; // 30 min
pub const DEFAULT_HEADWAY_MS: i64 = 300_000; // 5 min

// Economy (optional, NIMBY-style). Dollars. Construction is a one-time capital cost; fares
// accrue per boarding. The disruption metric feeds the surface land-taking premium.
pub const START_BUDGET: i64 = 2_000_000_000;
pub const FARE: i64 = 2; // $ per boarding
const PER_KM_SURFACE: i64 = 8_000_000;
const PER_KM_ELEVATED: i64 = 30_000_000;
const PER_KM_TUNNEL: i64 = 90_000_000;
// Heavy / high-speed rail needs dedicated, grade-separated right-of-way: far pricier per km.
const PER_KM_HSR_SURFACE: i64 = 24_000_000;
const PER_KM_HSR_ELEVATED: i64 = 60_000_000;
const PER_KM_HSR_TUNNEL: i64 = 180_000_000;
const TAKING_PER_KM_BUILT: i64 = 6_000_000;
const TRAIN_COST: i64 = 15_000_000;
// Recurring maintenance (opex), accrued only while the economy is ON and running. A slow drain
// that fares must outrun — the second pressure axis alongside waiting. Tunable game balance.
const DAY_MS: i64 = 86_400_000;
const OPEX_PER_TRAIN_DAY: i64 = 200_000;
const OPEX_PER_KM_DAY: i64 = 50_000;

pub struct World {
    pub seed: u64,
    pub clock_ms: i64,
    pub running: bool,
    pub rng: ChaCha8Rng,
    pub stations: Vec<Station>,
    pub lines: Vec<Line>,
    pub vehicles: VehicleSoA,
    pub city: CityData,
    pub cmd_log: Vec<Command>,
    /// Set when a line/trainset/headway/running change requires the dispatcher to rebuild
    /// vehicles; cleared after a rebuild so steady running does no work.
    pub dispatch_dirty: bool,

    // --- demand / ridership (T16) ---
    /// Per-station captured origin (resident) and destination (job) weight from the grid.
    pub captured_origin: Vec<f32>,
    pub captured_dest: Vec<f32>,
    /// Fractional passenger-spawn accumulator per station (deterministic count).
    pub spawn_accum: Vec<f32>,
    /// Per-station FIFO queue of waiting passengers (each carrying a multi-leg route).
    pub waiting: Vec<VecDeque<crate::pax::Pax>>,
    /// Per-station lines serving it (operational only); rebuilt by the dispatcher for routing.
    pub serving: Vec<Vec<LineId>>,
    /// Inter-station footpaths: per station, the nearby stations reachable on foot within
    /// `FOOTPATH_MM`, each with its integer walk time (ms). Derived from positions, rebuilt with
    /// the catchment when stations change. Lets RAPTOR transfer between unconnected lines whose
    /// stops are close (an interchange by foot); board_alight delays the rider by the walk time.
    pub footpaths: Vec<Vec<(u32, i64)>>,
    /// Route cache (origin,dest)->legs, so BFS isn't rerun per spawn on large networks.
    /// A derived cache (not hashed); cleared when the network changes. Lookup-only, so no
    /// HashMap-iteration determinism hazard.
    pub route_cache: rustc_hash::FxHashMap<(u32, u32), Option<Vec<crate::routing::Leg>>>,
    /// Accessibility cache: origin station → one-to-all transit travel time (ms) to every station
    /// (`i64::MAX` = unreachable), from `Router::reachable`. Lets the demand model weight a trip's
    /// destination by how fast the network reaches it. Derived (not hashed); cleared on network
    /// change alongside `route_cache`; lookup-only, so no HashMap-iteration determinism hazard.
    pub access_cache: rustc_hash::FxHashMap<u32, Vec<i64>>,
    /// Buildability lookup: (cell_x, cell_y) -> class code. Built once from CityData; lookup-only.
    pub build_lookup: rustc_hash::FxHashMap<(i32, i32), u8>,
    pub build_cell_mm: i64,
    /// Cumulative boardings (the headline ridership counter).
    pub ridership_total: u64,
    pub boardings: Vec<u64>,
    pub alightings: Vec<u64>,
    // --- passenger lifecycle telemetry (service-quality legibility) ---
    /// Σ end-to-end trip time (ms) over completed trips, and the completed-trip count.
    pub total_journey_ms: u64,
    pub journey_samples: u64,
    /// Σ platform wait (ms) over boardings, and the boarding count (one sample per board).
    pub total_wait_ms: u64,
    pub wait_samples: u64,
    /// Cumulative times a rider wanting a line was passed by a full vehicle (the real
    /// "left behind" pressure — distinct from the live waiting-queue depth).
    pub denied_boardings: u64,
    /// Cumulative riders who gave up waiting (renege) because service was too infrequent.
    pub abandoned: u64,
    /// `denied_boardings`/`abandoned` bucketed PER STATION (where the loss happened). Index-stable
    /// with `stations`; sums equal the global totals. Surfaced as the per-platform starvation
    /// signal. Folded into state_hash (deterministic, derived from the same command/tick sequence).
    pub denied_at: Vec<u64>,
    pub abandoned_at: Vec<u64>,
    /// Set when stations change (catchment capture needs recompute).
    pub demand_dirty: bool,
    /// Optional economy (NIMBY-style): when OFF (the default), money is informational only —
    /// when ON, construction you can't afford is rejected and opex drains the balance.
    pub economy_enabled: bool,
    /// Cumulative maintenance (opex) charged so far, and the sub-day remainder (exact integer
    /// accrual). Affects `balance` → the afford-gate, so both are folded into state_hash.
    pub opex_accrued: i64,
    pub opex_rem: i64,
    /// The trip-planning strategy (the routing seam). `BfsRouter` ships; RAPTOR swaps in here.
    pub router: Box<dyn crate::routing::Router>,
    /// Max legs (transfers + 1) a routed trip may use (from CityData, or the routing default).
    pub max_legs: usize,
}

/// Borrowed canonical view hashed for determinism. Field order = hash order (stable).
/// Vehicle integer state (line/arc-position/dir/dwell/onboard) is included so the
/// determinism test covers movement; render-only floats (x/y/angle) are excluded.
#[derive(Serialize)]
struct Canonical<'a> {
    clock_ms: i64,
    running: bool,
    stations: &'a [Station],
    lines: &'a [Line],
    veh_line: &'a [LineId],
    veh_s_mm: &'a [i64],
    veh_dir: &'a [i8],
    veh_dwell_ms: &'a [i64],
    veh_onboard: &'a [u16],
    ridership_total: u64,
    total_journey_ms: u64,
    journey_samples: u64,
    total_wait_ms: u64,
    wait_samples: u64,
    denied_boardings: u64,
    abandoned: u64,
    denied_at: &'a [u64],
    abandoned_at: &'a [u64],
    opex_accrued: i64,
    opex_rem: i64,
}

/// Save artifact: a seed plus the ordered command log. Replaying it reconstructs state
/// exactly (the determinism guarantee) and is the future lockstep-multiplayer transport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveGame {
    pub seed: u64,
    pub commands: Vec<Command>,
}

impl World {
    pub fn new(seed: u64, city: CityData) -> Self {
        // Build the buildability lookup from the committed grid (div_euclid so negative mm,
        // e.g. west of a Calgary origin, index consistently with the cells' centres).
        let build_cell_mm = if city.buildability.cell_m > 0.0 {
            (city.buildability.cell_m * 1000.0) as i64
        } else {
            120_000
        };
        let mut build_lookup = rustc_hash::FxHashMap::default();
        for cell in &city.buildability.cells {
            build_lookup.insert(
                (
                    cell.x_mm.div_euclid(build_cell_mm) as i32,
                    cell.y_mm.div_euclid(build_cell_mm) as i32,
                ),
                cell.c,
            );
        }
        let max_legs = if city.max_legs == 0 {
            crate::routing::DEFAULT_MAX_LEGS
        } else {
            city.max_legs
        };
        World {
            seed,
            clock_ms: 0,
            running: false,
            rng: ChaCha8Rng::seed_from_u64(seed),
            stations: Vec::new(),
            lines: Vec::new(),
            vehicles: VehicleSoA::default(),
            city,
            cmd_log: Vec::new(),
            dispatch_dirty: false,
            captured_origin: Vec::new(),
            captured_dest: Vec::new(),
            spawn_accum: Vec::new(),
            waiting: Vec::new(),
            ridership_total: 0,
            boardings: Vec::new(),
            alightings: Vec::new(),
            total_journey_ms: 0,
            journey_samples: 0,
            total_wait_ms: 0,
            wait_samples: 0,
            denied_boardings: 0,
            abandoned: 0,
            denied_at: Vec::new(),
            abandoned_at: Vec::new(),
            serving: Vec::new(),
            footpaths: Vec::new(),
            route_cache: rustc_hash::FxHashMap::default(),
            access_cache: rustc_hash::FxHashMap::default(),
            build_lookup,
            build_cell_mm,
            demand_dirty: false,
            economy_enabled: false,
            opex_accrued: 0,
            opex_rem: 0,
            router: Box::new(crate::routing::RaptorRouter),
            max_legs,
        }
    }

    /// Buildability class at a local mm point (Open if outside the grid).
    pub fn classify(&self, x_mm: i64, y_mm: i64) -> u8 {
        let key = (
            x_mm.div_euclid(self.build_cell_mm) as i32,
            y_mm.div_euclid(self.build_cell_mm) as i32,
        );
        self.build_lookup.get(&key).copied().unwrap_or(crate::city::class::OPEN)
    }

    /// Per-segment (disruption, surface-water flag, TRACK capital) for a line's current
    /// geometry + span modes — no trainset cost. A pure read of `self` (the buildability grid)
    /// and the line, shared by `recompute_line_buildability` (the committed line) and
    /// `preview_line_cost` (a hypothetical one) so the cost formula is never duplicated.
    fn line_cost_metrics(&self, l: &Line) -> (i64, bool, i64) {
        use crate::city::class;
        use crate::line::mode;
        use crate::trainset::tmode;
        let tm = l.mode;
        let mut disr = 0i64;
        let mut water = false;
        let mut capital = 0i64;
        for vi in 1..l.polyline.len() {
            let seg_m = (l.arclen_mm[vi] - l.arclen_mm[vi - 1]) / 1000; // mm -> metres
            if seg_m <= 0 {
                continue;
            }
            let span = l.span_of(l.arclen_mm[vi]);
            let m = l.span_mode.get(span).copied().unwrap_or(mode::SURFACE);
            let c = self.classify(l.polyline[vi].x_mm, l.polyline[vi].y_mm);
            // Per-mode placement: rail/bus blocked by water + penalised through built land;
            // ferry wants water (penalised over land, water is free); air is exempt.
            let (w, blocks_on_water): (i64, bool) = match tm {
                tmode::BUS => (
                    match c {
                        class::BUILT => 2, // buses run on city streets fine
                        class::WATER => 20,
                        class::PARK => 2,
                        _ => 0,
                    },
                    true,
                ),
                tmode::FERRY => (if c == class::WATER { 0 } else { 14 }, false), // must stay on water
                tmode::AIR => (0, false), // flies over anything
                _ => (
                    match c {
                        class::BUILT => 10,
                        class::WATER => 20,
                        class::PARK => 3,
                        _ => 0,
                    },
                    true,
                ),
            };
            let factor: i64 = match m {
                mode::ELEVATED => 25,
                mode::TUNNEL => 8,
                _ => 100, // Surface pays full
            };
            disr += w * seg_m * factor / 100;
            if blocks_on_water && c == class::WATER && m == mode::SURFACE {
                water = true;
            }
            // Capital per metre by transport mode (rail/heavy also by build-mode).
            let per_km = match tm {
                // Buses ride the existing ROAD network for free; off-road they build a busway.
                tmode::BUS => if c == class::ROAD { 0 } else { 3_000_000 },
                // Ferries cross open WATER for free (just terminals); forced over land they'd dig.
                tmode::FERRY => if c == class::WATER { 0 } else { 5_000_000 },
                tmode::AIR => 1_000_000,
                tmode::HEAVY => match m {
                    mode::ELEVATED => PER_KM_HSR_ELEVATED,
                    mode::TUNNEL => PER_KM_HSR_TUNNEL,
                    _ => PER_KM_HSR_SURFACE,
                },
                _ => match m {
                    mode::ELEVATED => PER_KM_ELEVATED,
                    mode::TUNNEL => PER_KM_TUNNEL,
                    _ => PER_KM_SURFACE,
                },
            };
            capital += per_km * seg_m / 1000;
            // Surface track through built-up land takes land (rail + heavy rail).
            if (tm == tmode::RAIL || tm == tmode::HEAVY) && c == class::BUILT && m == mode::SURFACE {
                capital += TAKING_PER_KM_BUILT * seg_m / 1000;
            }
        }
        (disr, water, capital)
    }

    /// Recompute a line's disruption + water flag + capital from the buildability grid and its
    /// per-span build modes. Cheap (one pass over the polyline vertices); called on a geometry
    /// or mode change.
    fn recompute_line_buildability(&mut self, line: LineId) {
        use crate::line::mode;
        let idx = line.index();
        if idx >= self.lines.len() {
            return;
        }
        let nspans = self.lines[idx].stops.len().saturating_sub(1);
        if self.lines[idx].span_mode.len() != nspans {
            self.lines[idx].span_mode.resize(nspans, mode::SURFACE);
        }
        let (disr, water, mut capital) = self.line_cost_metrics(&self.lines[idx]);
        capital += self.lines[idx].trainset.map(|t| t.count as i64).unwrap_or(0) * TRAIN_COST;
        self.lines[idx].disruption_units = disr;
        self.lines[idx].crosses_water_surface = water;
        self.lines[idx].capital_cost = capital;
    }

    /// Authoritative construction cost (track only, no trains) for a hypothetical line through
    /// the given station ids in `mode` — the cost-preview query for the build HUD, using the
    /// SAME formula as a committed line (no UI-side duplication; AGENTS "logic lives in core").
    /// Spans default to Surface (the draft is surface until grade-separated post-commit).
    pub fn preview_line_cost(&self, station_ids: &[u32], mode: u8, loop_line: bool) -> i64 {
        let pts: Vec<PointMm> = station_ids
            .iter()
            .filter_map(|&id| self.stations.get(id as usize))
            .filter(|s| !s.removed)
            .map(|s| s.pos)
            .collect();
        if pts.len() < 2 {
            return 0;
        }
        let mut l = Line::new(0, DEFAULT_HEADWAY_MS);
        l.mode = mode;
        l.loop_line = loop_line;
        l.rebuild_from_points(&pts); // empty span_mode ⇒ every span defaults to Surface
        let (_disr, _water, capital) = self.line_cost_metrics(&l);
        capital
    }

    /// Best (shortest) headway among operational lines serving station `s`, if any.
    /// "Operational" = has a trainset and ≥2 stops.
    fn best_headway_at(&self, s: usize) -> Option<i64> {
        let mut best: Option<i64> = None;
        for l in &self.lines {
            if !l.removed && l.trainset.is_some() && l.stops.len() >= 2 && l.stops.iter().any(|st| st.index() == s) {
                best = Some(best.map_or(l.headway_ms, |b| b.min(l.headway_ms)));
            }
        }
        best
    }

    /// 0–100 coverage score: the blend AGENTS mandates — (% of captured origin demand served)
    /// × a bounded wait-vs-headway quality factor. A served station contributes its demand
    /// weight scaled by the quality of its BEST (shortest-headway) line, where quality runs
    /// from 1.0 at the min headway down to a floor of 0.5 at the max headway. The floor is what
    /// keeps it MONOTONIC: extending coverage adds a non-negative term, and shortening a
    /// headway only raises a station's quality — neither can ever lower the score (PLAN §7).
    fn coverage_score(&self) -> u8 {
        let total: f32 = self.captured_origin.iter().sum();
        if total <= 0.0 {
            return 0;
        }
        let span = (MAX_HEADWAY_MS - MIN_HEADWAY_MS).max(1) as f32;
        let mut served = 0.0f32;
        for (s, &w) in self.captured_origin.iter().enumerate() {
            if w <= 0.0 {
                continue;
            }
            if let Some(h) = self.best_headway_at(s) {
                let frac_h = ((h - MIN_HEADWAY_MS) as f32 / span).clamp(0.0, 1.0);
                let quality = 1.0 - 0.5 * frac_h; // [0.5, 1.0]
                served += w * quality;
            }
        }
        ((served / total * 100.0).round() as i64).clamp(0, 100) as u8
    }

    /// Total one-time construction capital across all lines.
    fn capital_total(&self) -> i64 {
        self.lines.iter().filter(|l| !l.removed).map(|l| l.capital_cost).sum()
    }

    /// Current money: start budget + fares − capital − opex. Negative = over budget.
    fn balance(&self) -> i64 {
        START_BUDGET + self.ridership_total as i64 * FARE - self.capital_total() - self.opex_accrued
    }

    /// After a capital-changing mutation + recompute: true iff the economy is on AND the change
    /// raised capital AND drove the balance negative — i.e. the player can't afford it. The
    /// caller must then restore the pre-command state (the afford-gate; clamps live in the core).
    fn overspent(&self, old_capital: i64) -> bool {
        self.economy_enabled && self.capital_total() > old_capital && self.balance() < 0
    }

    /// Accrue recurring maintenance (opex) for one running step. Exact integer accrual via a
    /// sub-day remainder; only charged while the economy is enabled. Deterministic.
    fn accrue_opex(&mut self, dt_ms: i64) {
        if !self.economy_enabled || dt_ms <= 0 {
            return;
        }
        let trains: i64 = self.lines.iter().filter(|l| !l.removed).filter_map(|l| l.trainset).map(|t| t.count as i64).sum();
        let km: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.length_mm() / 1_000_000).sum();
        let rate_per_day = trains * OPEX_PER_TRAIN_DAY + km * OPEX_PER_KM_DAY;
        self.opex_rem += rate_per_day * dt_ms;
        self.opex_accrued += self.opex_rem / DAY_MS;
        self.opex_rem %= DAY_MS;
    }

    /// Charge opex for one running tick (called from the tick phase loop).
    pub(crate) fn tick_economy(&mut self, dt_ms: i64) {
        self.accrue_opex(dt_ms);
    }

    /// Low-frequency structured readout for the UI (the wasm->ts query port).
    pub fn stats_snapshot(&self) -> StatsSnapshot {
        let waiting_total: u64 = self.waiting.iter().map(|q| q.len() as u64).sum();

        // Average load factor across vehicles (onboard / capacity), plus a per-line mean for the
        // line-inspect strain readout — same single pass, re-binned by line index (no new state).
        let mut load_sum = 0.0f32;
        let mut load_n = 0u32;
        let mut line_load_sum = vec![0.0f32; self.lines.len()];
        let mut line_load_n = vec![0u32; self.lines.len()];
        for i in 0..self.vehicles.len() {
            let li = self.vehicles.line[i].index();
            if let Some(l) = self.lines.get(li) {
                if l.trainset.is_some() {
                    let cap = crate::trainset::spec_for_mode(l.mode).capacity.max(1) as f32;
                    let lf = self.vehicles.onboard[i] as f32 / cap;
                    load_sum += lf;
                    load_n += 1;
                    line_load_sum[li] += lf;
                    line_load_n[li] += 1;
                }
            }
        }
        let avg_load_factor = if load_n > 0 { load_sum / load_n as f32 } else { 0.0 };

        let per_station = (0..self.stations.len())
            .filter(|&s| !self.stations[s].removed)
            .map(|s| StationStat {
                station_id: s as u32,
                boardings: *self.boardings.get(s).unwrap_or(&0) as f64,
                alightings: *self.alightings.get(s).unwrap_or(&0) as f64,
                waiting: self.waiting.get(s).map(|q| q.len()).unwrap_or(0) as f64,
                demand_origin: *self.captured_origin.get(s).unwrap_or(&0.0) as f64,
                demand_dest: *self.captured_dest.get(s).unwrap_or(&0.0) as f64,
                serving: self.serving.get(s).map(|v| v.len()).unwrap_or(0) as u32,
                denied: *self.denied_at.get(s).unwrap_or(&0) as f64,
                abandoned: *self.abandoned_at.get(s).unwrap_or(&0) as f64,
            })
            .collect();

        let per_line = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.removed)
            .map(|(i, l)| {
                let ridership: u64 = l
                    .stops
                    .iter()
                    .map(|st| *self.boardings.get(st.index()).unwrap_or(&0))
                    .sum();
                LineStat {
                    line_id: i as u32,
                    name: l.name.clone(),
                    mode: l.mode,
                    color: l.color,
                    ridership: ridership as f64,
                    stops: l.stops.len() as u32,
                    trains: l.trainset.map(|t| t.count as u32).unwrap_or(0),
                    headway_ms: l.headway_ms as f64,
                    disruption: l.disruption_units as f64,
                    crosses_water: l.crosses_water_surface,
                    capital_cost: l.capital_cost as f64,
                    load_factor: if line_load_n[i] > 0 { line_load_sum[i] / line_load_n[i] as f32 } else { 0.0 },
                }
            })
            .collect();

        // Economy: balance = start budget + fares − capital − opex (informational when off).
        let capital_spent = self.capital_total();
        let fare_revenue: i64 = self.ridership_total as i64 * FARE;
        let balance = self.balance();

        // Build impact: total disruption per km of track, mapped to 0..100 (lower is better).
        let total_disr: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.disruption_units).sum();
        let total_track_m: i64 = self.lines.iter().filter(|l| !l.removed).map(|l| l.length_mm() / 1000).sum();
        let build_difficulty =
            ((total_disr * 5 / total_track_m.max(1)).clamp(0, 100)) as u8;

        StatsSnapshot {
            sim_clock_ms: self.clock_ms as f64,
            running: self.running,
            station_count: self.stations.iter().filter(|s| !s.removed).count() as u32,
            line_count: self.lines.iter().filter(|l| !l.removed).count() as u32,
            vehicle_count: self.vehicles.len() as u32,
            ridership_total: self.ridership_total as f64,
            waiting_total: waiting_total as f64,
            left_behind: self.denied_boardings as f64,
            denied_boardings: self.denied_boardings as f64,
            abandoned: self.abandoned as f64,
            avg_journey_ms: if self.journey_samples > 0 {
                self.total_journey_ms as f64 / self.journey_samples as f64
            } else {
                0.0
            },
            avg_wait_ms: if self.wait_samples > 0 {
                self.total_wait_ms as f64 / self.wait_samples as f64
            } else {
                0.0
            },
            avg_load_factor,
            coverage_score: self.coverage_score(),
            sim_hour: crate::tod::hour_of_day(self.clock_ms),
            period: crate::tod::period_label(crate::tod::hour_of_day(self.clock_ms)).to_string(),
            demand_multiplier: crate::tod::demand_multiplier(crate::tod::hour_of_day(self.clock_ms)) as f64,
            build_difficulty,
            economy_enabled: self.economy_enabled,
            balance: balance as f64,
            capital_spent: capital_spent as f64,
            fare_revenue: fare_revenue as f64,
            opex_spent: self.opex_accrued as f64,
            per_station,
            per_line,
        }
    }

    fn station_pos(&self, id: StationId) -> PointMm {
        self.stations
            .get(id.index())
            .map(|s| s.pos)
            .unwrap_or(PointMm::new(0, 0))
    }

    fn rebuild_line_geometry(&mut self, line: LineId) {
        // Snapshot stop positions first to avoid borrowing self mutably + immutably, then
        // rebuild the smoothed (curved) polyline + arc-length tables.
        if let Some(l) = self.lines.get(line.index()) {
            let pts: Vec<PointMm> = l.stops.iter().map(|&s| self.station_pos(s)).collect();
            // Buses follow the ROAD raster between stops and ferries follow WATER (auto-routed, A*
            // over the grid); other modes use the player's hand-placed waypoints. Both are
            // pass-through shaping points fed to the same smoother.
            use crate::trainset::tmode;
            let corridor = match l.mode {
                tmode::BUS => Some(crate::city::class::ROAD),
                tmode::FERRY => Some(crate::city::class::WATER),
                _ => None,
            };
            let span_points: Vec<Vec<PointMm>> = if let Some(prefer) = corridor {
                (0..pts.len().saturating_sub(1))
                    .map(|i| crate::roadnav::class_route(&self.build_lookup, self.build_cell_mm, prefer, pts[i], pts[i + 1]))
                    .collect()
            } else {
                l.waypoints.clone()
            };
            if let Some(l) = self.lines.get_mut(line.index()) {
                l.rebuild_with_span_points(&pts, &span_points);
            }
        }
    }

    /// Apply one command. Total + infallible: invalid commands return a `Rejected` event
    /// rather than panicking. Always records the command in the log.
    pub fn apply(&mut self, cmd: &Command) -> Vec<Event> {
        let events = match cmd {
            Command::PlaceStation { x_mm, y_mm, name } => {
                let id = StationId(self.stations.len() as u32);
                let name = name
                    .clone()
                    .unwrap_or_else(|| format!("Station {}", id.0 + 1));
                self.stations
                    .push(Station::new(PointMm::new(*x_mm, *y_mm), name.clone()));
                self.demand_dirty = true; // catchment capture must recompute
                vec![Event::StationPlaced { id, name }]
            }
            Command::CreateLine { color, name, loop_line, mode } => {
                let id = LineId(self.lines.len() as u32);
                let mut l = Line::new(*color, DEFAULT_HEADWAY_MS);
                l.name = name.clone().unwrap_or_else(|| format!("Line {}", id.0 + 1));
                l.loop_line = *loop_line;
                l.mode = *mode;
                self.lines.push(l);
                vec![Event::LineCreated { id }]
            }
            Command::AddStop {
                line,
                station,
                after,
            } => {
                let valid_line = line.index() < self.lines.len();
                let valid_station = station.index() < self.stations.len();
                if valid_line && valid_station {
                    let old_capital = self.capital_total();
                    let saved_stops = self.lines[line.index()].stops.clone();
                    {
                        let l = &mut self.lines[line.index()];
                        match after {
                            Some(i) if *i <= l.stops.len() => l.stops.insert(*i, *station),
                            _ => l.stops.push(*station),
                        }
                    }
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    if self.overspent(old_capital) {
                        // Can't afford this extension — restore the line exactly (afford-gate).
                        self.lines[line.index()].stops = saved_stops;
                        self.rebuild_line_geometry(*line);
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough money for this extension".into(),
                        }]
                    } else {
                        vec![Event::StopAdded {
                            line: *line,
                            station: *station,
                        }]
                    }
                } else {
                    vec![Event::Rejected {
                        reason: "AddStop: unknown line or station".into(),
                    }]
                }
            }
            Command::AssignTrainset { line, spec, count } => {
                if line.index() < self.lines.len() {
                    let count = (*count).clamp(1, MAX_TRAINS_PER_LINE);
                    let old_capital = self.capital_total();
                    let saved = self.lines[line.index()].trainset;
                    self.lines[line.index()].trainset = Some(TrainsetAssignment { spec: *spec, count });
                    self.recompute_line_buildability(*line); // train count affects capital cost
                    if self.overspent(old_capital) {
                        self.lines[line.index()].trainset = saved;
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough money for these trains".into(),
                        }]
                    } else {
                        vec![Event::TrainsetAssigned { line: *line, count }]
                    }
                } else {
                    vec![Event::Rejected {
                        reason: "AssignTrainset: unknown line".into(),
                    }]
                }
            }
            Command::SetHeadway { line, headway_ms } => {
                if let Some(l) = self.lines.get_mut(line.index()) {
                    let h = (*headway_ms).clamp(MIN_HEADWAY_MS, MAX_HEADWAY_MS);
                    l.headway_ms = h;
                    vec![Event::HeadwaySet {
                        line: *line,
                        headway_ms: h,
                    }]
                } else {
                    vec![Event::Rejected {
                        reason: "SetHeadway: unknown line".into(),
                    }]
                }
            }
            Command::SetSegmentMode { line, span, mode } => {
                if line.index() < self.lines.len() {
                    let old_capital = self.capital_total();
                    let saved_modes = self.lines[line.index()].span_mode.clone();
                    {
                        let l = &mut self.lines[line.index()];
                        let nspans = l.stops.len().saturating_sub(1);
                        if l.span_mode.len() != nspans {
                            l.span_mode.resize(nspans, crate::line::mode::SURFACE);
                        }
                        let m = (*mode).min(crate::line::mode::TUNNEL);
                        if *span == u32::MAX {
                            for s in l.span_mode.iter_mut() {
                                *s = m;
                            }
                        } else if (*span as usize) < l.span_mode.len() {
                            l.span_mode[*span as usize] = m;
                        }
                    }
                    self.recompute_line_buildability(*line);
                    if self.overspent(old_capital) {
                        self.lines[line.index()].span_mode = saved_modes;
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected {
                            reason: "Not enough money to grade-separate this line".into(),
                        }]
                    } else {
                        let m = (*mode).min(crate::line::mode::TUNNEL);
                        vec![Event::SegmentModeSet { line: *line, span: *span, mode: m }]
                    }
                } else {
                    vec![Event::Rejected { reason: "SetSegmentMode: unknown line".into() }]
                }
            }
            Command::SetRunning { running } => {
                self.running = *running;
                vec![Event::RunningSet { running: *running }]
            }
            Command::SetEconomy { enabled } => {
                self.economy_enabled = *enabled;
                vec![Event::EconomySet { enabled: *enabled }]
            }
            Command::RemoveStation { station } => {
                let idx = station.index();
                if idx < self.stations.len() && !self.stations[idx].removed {
                    self.stations[idx].removed = true;
                    // Drop the station from every line that stops there, then rebuild those
                    // lines' geometry + cost (the line simply skips the bulldozed stop).
                    let affected: Vec<usize> = self
                        .lines
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| !l.removed && l.stops.iter().any(|s| s.index() == idx))
                        .map(|(li, _)| li)
                        .collect();
                    for li in affected {
                        self.lines[li].stops.retain(|s| s.index() != idx);
                        self.rebuild_line_geometry(LineId(li as u32));
                        self.recompute_line_buildability(LineId(li as u32));
                    }
                    if let Some(q) = self.waiting.get_mut(idx) {
                        q.clear(); // riders waiting at a bulldozed station are gone
                    }
                    self.demand_dirty = true; // its catchment frees up for neighbours
                    vec![Event::StationRemoved { station: *station }]
                } else {
                    vec![Event::Rejected {
                        reason: "RemoveStation: unknown or already removed".into(),
                    }]
                }
            }
            Command::RemoveLine { line } => {
                let idx = line.index();
                if idx < self.lines.len() && !self.lines[idx].removed {
                    self.lines[idx].removed = true; // vehicles despawn on the next dispatch rebuild
                    vec![Event::LineRemoved { line: *line }]
                } else {
                    vec![Event::Rejected {
                        reason: "RemoveLine: unknown or already removed".into(),
                    }]
                }
            }
            Command::SetLineWaypoints { line, waypoints } => {
                if line.index() < self.lines.len() && !self.lines[line.index()].removed {
                    let old_capital = self.capital_total();
                    let saved = self.lines[line.index()].waypoints.clone();
                    self.lines[line.index()].waypoints = waypoints
                        .iter()
                        .map(|span| span.iter().map(|&[x, y]| PointMm::new(x, y)).collect())
                        .collect();
                    // Bending the track changes its length → geometry, buildability and cost.
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    if self.overspent(old_capital) {
                        self.lines[line.index()].waypoints = saved;
                        self.rebuild_line_geometry(*line);
                        self.recompute_line_buildability(*line);
                        vec![Event::Rejected { reason: "Not enough money to reroute this line".into() }]
                    } else {
                        vec![Event::WaypointsSet { line: *line }]
                    }
                } else {
                    vec![Event::Rejected { reason: "SetLineWaypoints: unknown line".into() }]
                }
            }
        };
        // Any change to lines / trainsets / headway / running invalidates dispatch.
        if !matches!(cmd, Command::PlaceStation { .. }) {
            self.dispatch_dirty = true;
        }
        self.cmd_log.push(cmd.clone());
        events
    }

    /// Advance the simulation by one fixed step.
    pub fn tick(&mut self, dt_ms: i64) {
        tick::step(self, dt_ms);
    }

    /// FNV-1a over a canonical, ordered serialization of state. The determinism oracle.
    pub fn state_hash(&self) -> u64 {
        let canon = Canonical {
            clock_ms: self.clock_ms,
            running: self.running,
            stations: &self.stations,
            lines: &self.lines,
            veh_line: &self.vehicles.line,
            veh_s_mm: &self.vehicles.s_mm,
            veh_dir: &self.vehicles.dir,
            veh_dwell_ms: &self.vehicles.dwell_until_ms,
            veh_onboard: &self.vehicles.onboard,
            ridership_total: self.ridership_total,
            total_journey_ms: self.total_journey_ms,
            journey_samples: self.journey_samples,
            total_wait_ms: self.total_wait_ms,
            wait_samples: self.wait_samples,
            denied_boardings: self.denied_boardings,
            abandoned: self.abandoned,
            denied_at: &self.denied_at,
            abandoned_at: &self.abandoned_at,
            opex_accrued: self.opex_accrued,
            opex_rem: self.opex_rem,
        };
        let bytes = postcard::to_allocvec(&canon).expect("canonical state serializes");
        fnv1a(&bytes)
    }

    /// Authoritative station geometry for rendering (mm as f64; no BigInt at the boundary).
    pub fn stations_view(&self) -> Vec<StationView> {
        self.stations
            .iter()
            .enumerate()
            .map(|(i, s)| StationView {
                id: i as u32,
                x_mm: s.pos.x_mm as f64,
                y_mm: s.pos.y_mm as f64,
                name: s.name.clone(),
                removed: s.removed,
            })
            .collect()
    }

    /// OD "desire lines" from a selected origin station: the top `top_k` destinations its riders
    /// are drawn toward (gravity attractiveness × accessibility), for the on-selection flow overlay.
    /// Read-only — solves accessibility fresh and mutates nothing. `weight` is normalized 0..1 vs
    /// the strongest link; empty if the origin isn't an operational, served station.
    pub fn station_od(&self, origin: u32, top_k: usize) -> Vec<OdLink> {
        let mut w = crate::demand::od_weights(self, origin as usize);
        if w.is_empty() {
            return Vec::new();
        }
        // Descending by pull; partial_cmp fallback keeps it total-ordered (weights are finite).
        w.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let max = w.first().map(|(_, x)| *x).unwrap_or(1.0).max(1e-9);
        w.into_iter()
            .take(top_k)
            .map(|(d, wt)| {
                let p = self.stations[d as usize].pos;
                OdLink { dest: d, x_mm: p.x_mm as f64, y_mm: p.y_mm as f64, weight: (wt / max) as f32 }
            })
            .collect()
    }

    /// Accessibility isochrone from a selected origin station: every OTHER served station it can
    /// reach by transit, with the travel time (wait + ride + transfers) via `Router::reachable`.
    /// For the opt-in "Reach" overlay. Read-only — solves fresh, mutates nothing. Empty if the
    /// origin isn't an operational, served station; unreachable stations are simply omitted.
    pub fn station_access(&self, origin: u32) -> Vec<AccessLink> {
        let o = origin as usize;
        if o >= self.stations.len() || self.serving.get(o).map(|v| v.is_empty()).unwrap_or(true) {
            return Vec::new();
        }
        let access = self
            .router
            .reachable(&self.lines, &self.serving, &self.footpaths, StationId(origin), self.max_legs);
        if access.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<AccessLink> = Vec::new();
        for d in 0..self.stations.len() {
            if d == o
                || self.stations[d].removed
                || self.serving.get(d).map(|v| v.is_empty()).unwrap_or(true)
            {
                continue;
            }
            match access.get(d).copied() {
                Some(t) if t < i64::MAX => {
                    let p = self.stations[d].pos;
                    out.push(AccessLink { station: d as u32, x_mm: p.x_mm as f64, y_mm: p.y_mm as f64, ms: t as f64 });
                }
                _ => {}
            }
        }
        out
    }

    /// Authoritative line geometry (ordered stops + polyline) for rendering.
    pub fn lines_view(&self) -> Vec<LineView> {
        self.lines
            .iter()
            .enumerate()
            .map(|(i, l)| LineView {
                id: i as u32,
                name: l.name.clone(),
                mode: l.mode,
                loop_line: l.loop_line,
                color: l.color,
                stops: l.stops.iter().map(|s| s.0).collect(),
                polyline_mm: l
                    .polyline
                    .iter()
                    .map(|p| [p.x_mm as f64, p.y_mm as f64])
                    .collect(),
                min_radius_mm: l.min_radius_mm as f64,
                span_modes: l.span_mode.clone(),
                crosses_water_surface: l.crosses_water_surface,
                removed: l.removed,
            })
            .collect()
    }

    pub fn save(&self) -> SaveGame {
        SaveGame {
            seed: self.seed,
            commands: self.cmd_log.clone(),
        }
    }
}

/// Reconstruct a world by replaying a save (seed + command log) onto a fresh `CityData`.
/// `tick_to` advances the clock by replaying ticks; pass the original tick schedule.
pub fn replay(save: &SaveGame, city: CityData) -> World {
    let mut w = World::new(save.seed, city);
    for cmd in &save.commands {
        w.apply(cmd);
    }
    w
}
