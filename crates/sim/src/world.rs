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
    /// Per-station FIFO queue of waiting passengers (their destination station).
    pub waiting: Vec<VecDeque<StationId>>,
    /// Cumulative boardings (the headline ridership counter).
    pub ridership_total: u64,
    pub boardings: Vec<u64>,
    pub alightings: Vec<u64>,
    /// Set when stations change (catchment capture needs recompute).
    pub demand_dirty: bool,
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
            demand_dirty: false,
        }
    }

    /// True if station `s` is on a line with a trainset and at least 2 stops.
    fn station_served(&self, s: usize) -> bool {
        self.lines
            .iter()
            .any(|l| l.trainset.is_some() && l.stops.len() >= 2 && l.stops.iter().any(|st| st.index() == s))
    }

    /// 0–100 coverage score: fraction of total origin demand that is served by some line.
    /// Monotonic — extending a line to cover more demand can only raise it (PLAN §7).
    fn coverage_score(&self) -> u8 {
        let total: f32 = self.captured_origin.iter().sum();
        if total <= 0.0 {
            return 0;
        }
        let served: f32 = self
            .captured_origin
            .iter()
            .enumerate()
            .filter(|(s, _)| self.station_served(*s))
            .map(|(_, &w)| w)
            .sum();
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
                if let Some(t) = l.trainset {
                    let cap = crate::trainset::spec(t.spec).capacity.max(1) as f32;
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
                    color: l.color,
                    ridership: ridership as f64,
                    stops: l.stops.len() as u32,
                    trains: l.trainset.map(|t| t.count as u32).unwrap_or(0),
                    headway_ms: l.headway_ms as f64,
                }
            })
            .collect();

        StatsSnapshot {
            sim_clock_ms: self.clock_ms as f64,
            running: self.running,
            station_count: self.stations.len() as u32,
            line_count: self.lines.len() as u32,
            vehicle_count: self.vehicles.len() as u32,
            ridership_total: self.ridership_total as f64,
            waiting_total: waiting_total as f64,
            left_behind: waiting_total as f64,
            avg_load_factor,
            coverage_score: self.coverage_score(),
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
            Command::CreateLine { color } => {
                let id = LineId(self.lines.len() as u32);
                self.lines.push(Line::new(*color, DEFAULT_HEADWAY_MS));
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
            Command::SetRunning { running } => {
                self.running = *running;
                vec![Event::RunningSet { running: *running }]
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
                color: l.color,
                stops: l.stops.iter().map(|s| s.0).collect(),
                polyline_mm: l
                    .polyline
                    .iter()
                    .map(|p| [p.x_mm as f64, p.y_mm as f64])
                    .collect(),
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
