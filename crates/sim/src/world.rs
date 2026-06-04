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
use crate::stats::{LineStat, LineView, StationStat, StatsSnapshot, StationView};
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
const TAKING_PER_KM_BUILT: i64 = 6_000_000;
const TRAIN_COST: i64 = 15_000_000;

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
    /// Route cache (origin,dest)->legs, so BFS isn't rerun per spawn on large networks.
    /// A derived cache (not hashed); cleared when the network changes. Lookup-only, so no
    /// HashMap-iteration determinism hazard.
    pub route_cache: rustc_hash::FxHashMap<(u32, u32), Option<Vec<crate::routing::Leg>>>,
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
    /// Set when stations change (catchment capture needs recompute).
    pub demand_dirty: bool,
    /// Optional economy (NIMBY-style): when off, money is informational only.
    pub economy_enabled: bool,
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
            serving: Vec::new(),
            route_cache: rustc_hash::FxHashMap::default(),
            build_lookup,
            build_cell_mm,
            demand_dirty: false,
            economy_enabled: true,
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

    /// Recompute a line's surface-rail disruption + water flag from the buildability grid and
    /// its per-span build modes. Cheap (one pass over the polyline vertices); called on a
    /// geometry or mode change. Disruption = Σ weight(class) × segment-metres × mode-factor.
    fn recompute_line_buildability(&mut self, line: LineId) {
        use crate::city::class;
        use crate::line::mode;
        let idx = line.index();
        if idx >= self.lines.len() {
            return;
        }
        let nspans = self.lines[idx].stops.len().saturating_sub(1);
        if self.lines[idx].span_mode.len() != nspans {
            self.lines[idx].span_mode.resize(nspans, mode::SURFACE);
        }

        use crate::trainset::tmode;
        let tm = self.lines[idx].mode;
        let mut disr = 0i64;
        let mut water = false;
        let mut capital = 0i64;
        {
            let l = &self.lines[idx];
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
                // Capital per metre by transport mode (rail also by build-mode).
                let per_km = match tm {
                    tmode::BUS => 3_000_000,
                    tmode::FERRY => 5_000_000,
                    tmode::AIR => 1_000_000,
                    _ => match m {
                        mode::ELEVATED => PER_KM_ELEVATED,
                        mode::TUNNEL => PER_KM_TUNNEL,
                        _ => PER_KM_SURFACE,
                    },
                };
                capital += per_km * seg_m / 1000;
                if tm == tmode::RAIL && c == class::BUILT && m == mode::SURFACE {
                    capital += TAKING_PER_KM_BUILT * seg_m / 1000;
                }
            }
        }
        capital += self.lines[idx].trainset.map(|t| t.count as i64).unwrap_or(0) * TRAIN_COST;
        self.lines[idx].disruption_units = disr;
        self.lines[idx].crosses_water_surface = water;
        self.lines[idx].capital_cost = capital;
    }

    /// Best (shortest) headway among operational lines serving station `s`, if any.
    /// "Operational" = has a trainset and ≥2 stops.
    fn best_headway_at(&self, s: usize) -> Option<i64> {
        let mut best: Option<i64> = None;
        for l in &self.lines {
            if l.trainset.is_some() && l.stops.len() >= 2 && l.stops.iter().any(|st| st.index() == s) {
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

    /// Low-frequency structured readout for the UI (the wasm->ts query port).
    pub fn stats_snapshot(&self) -> StatsSnapshot {
        let waiting_total: u64 = self.waiting.iter().map(|q| q.len() as u64).sum();

        // Average load factor across vehicles (onboard / capacity).
        let mut load_sum = 0.0f32;
        let mut load_n = 0u32;
        for i in 0..self.vehicles.len() {
            if let Some(l) = self.lines.get(self.vehicles.line[i].index()) {
                if l.trainset.is_some() {
                    let cap = crate::trainset::spec_for_mode(l.mode).capacity.max(1) as f32;
                    load_sum += self.vehicles.onboard[i] as f32 / cap;
                    load_n += 1;
                }
            }
        }
        let avg_load_factor = if load_n > 0 { load_sum / load_n as f32 } else { 0.0 };

        let per_station = (0..self.stations.len())
            .map(|s| StationStat {
                station_id: s as u32,
                boardings: *self.boardings.get(s).unwrap_or(&0) as f64,
                alightings: *self.alightings.get(s).unwrap_or(&0) as f64,
                waiting: self.waiting.get(s).map(|q| q.len()).unwrap_or(0) as f64,
            })
            .collect();

        let per_line = self
            .lines
            .iter()
            .enumerate()
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
                }
            })
            .collect();

        // Economy: balance = start budget + fares − capital (informational when off).
        let capital_spent: i64 = self.lines.iter().map(|l| l.capital_cost).sum();
        let fare_revenue: i64 = self.ridership_total as i64 * FARE;
        let balance = START_BUDGET + fare_revenue - capital_spent;

        // Build impact: total disruption per km of track, mapped to 0..100 (lower is better).
        let total_disr: i64 = self.lines.iter().map(|l| l.disruption_units).sum();
        let total_track_m: i64 = self.lines.iter().map(|l| l.length_mm() / 1000).sum();
        let build_difficulty =
            ((total_disr * 5 / total_track_m.max(1)).clamp(0, 100)) as u8;

        StatsSnapshot {
            sim_clock_ms: self.clock_ms as f64,
            running: self.running,
            station_count: self.stations.len() as u32,
            line_count: self.lines.len() as u32,
            vehicle_count: self.vehicles.len() as u32,
            ridership_total: self.ridership_total as f64,
            waiting_total: waiting_total as f64,
            left_behind: self.denied_boardings as f64,
            denied_boardings: self.denied_boardings as f64,
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
            if let Some(l) = self.lines.get_mut(line.index()) {
                l.rebuild_from_points(&pts);
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
                    {
                        let l = &mut self.lines[line.index()];
                        match after {
                            Some(i) if *i <= l.stops.len() => l.stops.insert(*i, *station),
                            _ => l.stops.push(*station),
                        }
                    }
                    self.rebuild_line_geometry(*line);
                    self.recompute_line_buildability(*line);
                    vec![Event::StopAdded {
                        line: *line,
                        station: *station,
                    }]
                } else {
                    vec![Event::Rejected {
                        reason: "AddStop: unknown line or station".into(),
                    }]
                }
            }
            Command::AssignTrainset { line, spec, count } => {
                if let Some(l) = self.lines.get_mut(line.index()) {
                    let count = (*count).clamp(1, MAX_TRAINS_PER_LINE);
                    l.trainset = Some(TrainsetAssignment { spec: *spec, count });
                    self.recompute_line_buildability(*line); // train count affects capital cost
                    vec![Event::TrainsetAssigned { line: *line, count }]
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
                if let Some(l) = self.lines.get_mut(line.index()) {
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
                    self.recompute_line_buildability(*line);
                    vec![Event::SegmentModeSet { line: *line, span: *span, mode: m }]
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
            })
            .collect()
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
