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
use crate::stats::{LineStat, LineView, StatsSnapshot, StationView};
use crate::tick;
use crate::trainset::TrainsetAssignment;
use crate::vehicle::VehicleSoA;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

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
}

/// Borrowed canonical view hashed for determinism. Field order = hash order (stable).
#[derive(Serialize)]
struct Canonical<'a> {
    clock_ms: i64,
    running: bool,
    stations: &'a [Station],
    lines: &'a [Line],
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
        }
    }

    /// Low-frequency structured readout for the UI (the wasm->ts query port). Ridership /
    /// waiting / coverage are filled in T16b; counts + per-line metadata are live now.
    pub fn stats_snapshot(&self) -> StatsSnapshot {
        let per_line = self
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| LineStat {
                line_id: i as u32,
                color: l.color,
                ridership: 0.0,
                stops: l.stops.len() as u32,
                trains: l.trainset.map(|t| t.count as u32).unwrap_or(0),
                headway_ms: l.headway_ms as f64,
            })
            .collect();
        StatsSnapshot {
            sim_clock_ms: self.clock_ms as f64,
            running: self.running,
            station_count: self.stations.len() as u32,
            line_count: self.lines.len() as u32,
            vehicle_count: self.vehicles.len() as u32,
            ridership_total: 0.0,
            waiting_total: 0.0,
            left_behind: 0.0,
            avg_load_factor: 0.0,
            coverage_score: 0,
            per_station: Vec::new(),
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
        // Snapshot stop positions first to avoid borrowing self mutably + immutably.
        if let Some(l) = self.lines.get(line.index()) {
            let pts: Vec<PointMm> = l.stops.iter().map(|&s| self.station_pos(s)).collect();
            if let Some(l) = self.lines.get_mut(line.index()) {
                l.polyline = pts;
                l.arclen_mm.clear();
                let mut acc = 0i64;
                for i in 0..l.polyline.len() {
                    if i == 0 {
                        l.arclen_mm.push(0);
                    } else {
                        acc += l.polyline[i - 1].dist_mm(&l.polyline[i]);
                        l.arclen_mm.push(acc);
                    }
                }
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
