//! Agent-based demand — PROTOTYPE / benchmark scaffold (opt-in; gravity stays the default).
//!
//! A dormant, seed-derived population of citizens, each with a HOME cell + WORK cell + a name
//! index, who make scheduled AM (home→work) / PM (work→home) trips. The design invariants that
//! keep it cheap (see the perf analysis):
//!   • the population is a pure function of `(seed, grid)` → regenerable, so it is NOT hashed;
//!     only the trips it spawns (Pax → ridership/telemetry counters) affect sim state;
//!   • a bucketed departure schedule makes per-tick spawning **O(departures)**, never O(N);
//!   • names are **indices** into a name table, never `String`s in the hot data.
//!
//! `spawn_trips` is the agent counterpart to `demand::spawn`: it pushes routed `Pax` into the
//! same `waiting` queues, so the whole downstream (route cache, board/alight, motion) is shared.
use crate::pax::Pax;
use crate::ids::StationId;
use crate::world::World;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// One in-game day in sim-ms (24 in-game hours).
const DAY_MS: i64 = 24 * crate::tod::HOUR_MS;
/// Departure-time buckets across the day — the schedule index (≈1 s sim-time each).
const BUCKETS: usize = 1440;
const BUCKET_MS: i64 = DAY_MS / BUCKETS as i64;
/// Max walk (mm) from a home/work cell to its boarding station; beyond this the citizen can't ride.
const ACCESS_MM: i64 = 1_200_000; // ~1.2 km
/// Name table size (first × last); a citizen stores only a u32 index into it.
pub const NAME_SPACE: u32 = 64 * 64;

/// One citizen: where they live + work + a name index. 12 bytes, `Copy`. Dormant unless on a trip.
#[derive(Clone, Copy)]
pub struct Citizen {
    pub name_idx: u32,
    pub home_cell: u32,
    pub work_cell: u32,
}

pub struct Population {
    pub citizens: Vec<Citizen>,
    /// Citizen indices departing HOME→work / WORK→home in each ~1 s departure bucket.
    am: Vec<Vec<u32>>,
    pm: Vec<Vec<u32>>,
    /// Nearest served station per demand cell (-1 = none within ACCESS_MM). Rebuilt on net change.
    cell_station: Vec<i32>,
}

impl Population {
    /// Generate a population from the city's demand grid — homes weighted by `origin_w`, jobs by
    /// `dest_w`. Deterministic from `seed` (its own RNG stream, so it never perturbs the sim's).
    pub fn generate(world: &World, n: usize, seed: u64) -> Population {
        let cells = &world.city.demand.cells;
        let home_cum = cumulative(cells.iter().map(|c| c.origin_w as f64));
        let work_cum = cumulative(cells.iter().map(|c| c.dest_w as f64));
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xC1_71_2E_5);
        let mut citizens = Vec::with_capacity(n);
        let mut am = vec![Vec::new(); BUCKETS];
        let mut pm = vec![Vec::new(); BUCKETS];
        for i in 0..n {
            let home = weighted_pick(&home_cum, &mut rng) as u32;
            let work = weighted_pick(&work_cum, &mut rng) as u32;
            let name_idx = rng.random_range(0..NAME_SPACE as u64) as u32;
            // Departures concentrate (triangular) around the AM(08:00)/PM(18:00) peaks.
            let am_b = (peak_ms(8, &mut rng) / BUCKET_MS) as usize % BUCKETS;
            let pm_b = (peak_ms(18, &mut rng) / BUCKET_MS) as usize % BUCKETS;
            am[am_b].push(i as u32);
            pm[pm_b].push(i as u32);
            citizens.push(Citizen { name_idx, home_cell: home, work_cell: work });
        }
        Population { citizens, am, pm, cell_station: Vec::new() }
    }

    /// Append citizens up to `n` (no-op at or above it), drawn from the CURRENT demand grid —
    /// the transit-oriented-growth hook: as the city grows, new residents move in following the
    /// GROWN distribution, i.e. they cluster around the network that caused the growth, and agent
    /// trips rise with the city like gravity trips do. Append-only (existing citizens, their
    /// schedules, and any in-flight trips are untouched) and deterministic: the RNG stream is
    /// keyed by (seed, start index), so a replayed sequence of day-boundary top-ups redraws the
    /// exact same citizens regardless of when each top-up ran.
    pub fn grow_to(&mut self, world: &World, n: usize, seed: u64) {
        let start = self.citizens.len();
        if n <= start {
            return;
        }
        let cells = &world.city.demand.cells;
        let home_cum = cumulative(cells.iter().map(|c| c.origin_w as f64));
        let work_cum = cumulative(cells.iter().map(|c| c.dest_w as f64));
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x6E77_C17A ^ ((start as u64) << 1));
        self.citizens.reserve(n - start);
        for i in start..n {
            let home = weighted_pick(&home_cum, &mut rng) as u32;
            let work = weighted_pick(&work_cum, &mut rng) as u32;
            let name_idx = rng.random_range(0..NAME_SPACE as u64) as u32;
            let am_b = (peak_ms(8, &mut rng) / BUCKET_MS) as usize % BUCKETS;
            let pm_b = (peak_ms(18, &mut rng) / BUCKET_MS) as usize % BUCKETS;
            self.am[am_b].push(i as u32);
            self.pm[pm_b].push(i as u32);
            self.citizens.push(Citizen { name_idx, home_cell: home, work_cell: work });
        }
    }

    /// Spawn the trips whose departure bucket the clock crossed this tick — O(departures), not O(N).
    /// Mirrors `demand::spawn`'s route-and-queue, so the downstream load is identical to gravity's.
    pub fn spawn_trips(&mut self, world: &mut World, dt_ms: i64) {
        // Refresh the nearest-served-station map when the grid size changes OR the network changed
        // (the served set moves under us as the player builds/bulldozes — dispatch flags it). Without
        // the dirty check this was built once and went stale (agents routed to dead/old stations).
        if self.cell_station.len() != world.city.demand.cells.len() || world.cell_station_dirty {
            self.rebuild_cell_station(world);
            world.cell_station_dirty = false;
        }
        let now = world.clock_ms;
        let max_legs = world.max_legs;
        // Absolute bucket counter → fires each bucket once per day, no day-wrap special-casing.
        let cur_abs = now.div_euclid(BUCKET_MS);
        let last_abs = (now - dt_ms).div_euclid(BUCKET_MS);
        if cur_abs <= last_abs {
            return; // no departure bucket boundary crossed this tick
        }
        let World { ref lines, ref serving, ref footpaths, ref mut waiting, ref mut route_cache, ref router, .. } = *world;
        // Bounds-safe cell→station lookup: a cell with no served station in range (or a map not yet
        // built — e.g. an empty demand grid) reads as -1, which push_trip treats as "no trip".
        let station_of = |cell: u32| self.cell_station.get(cell as usize).copied().unwrap_or(-1);
        for ab in (last_abs + 1)..=cur_abs {
            let b = ab.rem_euclid(BUCKETS as i64) as usize;
            for &ci in &self.am[b] {
                if let Some(c) = self.citizens.get(ci as usize) {
                    push_trip(station_of(c.home_cell), station_of(c.work_cell), ci, lines, serving, footpaths, waiting, route_cache, &**router, max_legs, now);
                }
            }
            for &ci in &self.pm[b] {
                if let Some(c) = self.citizens.get(ci as usize) {
                    push_trip(station_of(c.work_cell), station_of(c.home_cell), ci, lines, serving, footpaths, waiting, route_cache, &**router, max_legs, now);
                }
            }
        }
    }

    /// Nearest served station to each demand cell (within ACCESS_MM); rebuilt on a network change.
    fn rebuild_cell_station(&mut self, world: &World) {
        let cells = &world.city.demand.cells;
        let mut out = vec![-1i32; cells.len()];
        for (ci, cell) in cells.iter().enumerate() {
            let mut best = -1i32;
            let mut bestd = ACCESS_MM as f64;
            for (si, st) in world.stations.iter().enumerate() {
                if st.removed || world.serving.get(si).map(|v| v.is_empty()).unwrap_or(true) {
                    continue;
                }
                let dx = (st.pos.x_mm - cell.x_mm) as f64;
                let dy = (st.pos.y_mm - cell.y_mm) as f64;
                let d = (dx * dx + dy * dy).sqrt();
                if d <= bestd {
                    bestd = d;
                    best = si as i32;
                }
            }
            out[ci] = best;
        }
        self.cell_station = out;
    }

    /// Heap footprint estimate (bytes) — population table + schedule buckets + cell map.
    pub fn mem_bytes(&self) -> usize {
        use std::mem::size_of;
        self.citizens.len() * size_of::<Citizen>()
            + (self.am.iter().map(Vec::len).sum::<usize>() + self.pm.iter().map(Vec::len).sum::<usize>()) * size_of::<u32>()
            + (self.am.len() + self.pm.len()) * size_of::<Vec<u32>>()
            + self.cell_station.len() * size_of::<i32>()
    }
}

#[allow(clippy::too_many_arguments)]
fn push_trip(
    o: i32,
    d: i32,
    cid: u32,
    lines: &[crate::line::Line],
    serving: &[Vec<crate::ids::LineId>],
    footpaths: &[Vec<(u32, i64)>],
    waiting: &mut [std::collections::VecDeque<Pax>],
    route_cache: &mut rustc_hash::FxHashMap<(u32, u32), Option<Vec<crate::routing::Leg>>>,
    router: &dyn crate::routing::Router,
    max_legs: usize,
    now: i64,
) {
    if o < 0 || d < 0 || o == d {
        return; // home or work isn't near a served station, or they coincide
    }
    let (oi, di) = (o as u32, d as u32);
    let entry = route_cache
        .entry((oi, di))
        .or_insert_with(|| router.plan(lines, serving, footpaths, StationId(oi), StationId(di), max_legs));
    if let Some(legs) = entry {
        if !legs.is_empty() {
            waiting[oi as usize].push_back(Pax { legs: legs.clone(), leg: 0, t_spawn_ms: now, t_wait_ms: now, citizen_id: cid });
        }
    }
}

/// Prefix sums of (clamped non-negative) weights — the cumulative table for a weighted draw.
fn cumulative(weights: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut acc = 0.0;
    weights
        .map(|w| {
            acc += w.max(0.0);
            acc
        })
        .collect()
}

/// First index whose cumulative weight exceeds a seeded draw in [0, total). Deterministic.
fn weighted_pick(cum: &[f64], rng: &mut ChaCha8Rng) -> usize {
    let total = cum.last().copied().unwrap_or(0.0);
    if total <= 0.0 || cum.is_empty() {
        return 0;
    }
    let r = rng.random_range(0..1_000_000u64) as f64 / 1_000_000.0 * total;
    cum.partition_point(|&c| c <= r).min(cum.len() - 1)
}

/// A departure ms-of-day concentrated (triangular: sum of two uniforms) around an in-game `hour`.
fn peak_ms(hour: i64, rng: &mut ChaCha8Rng) -> i64 {
    let base = (hour - 6).rem_euclid(24) * crate::tod::HOUR_MS; // ms-of-day of this in-game hour
    let half = crate::tod::HOUR_MS * 3 / 2; // ±1.5 in-game hours of spread
    let j = rng.random_range(0..half as u64) as i64 + rng.random_range(0..half as u64) as i64 - half;
    (base + j).rem_euclid(DAY_MS)
}

// --- citizen identity resolution (for the journey inspector) ---

impl Population {
    /// The citizen's display name (resolved from their name index).
    pub fn name(&self, id: u32) -> String {
        self.citizens.get(id as usize).map(|c| citizen_name(c.name_idx)).unwrap_or_default()
    }
    /// Nearest served station to the citizen's HOME cell (None if unserved / not yet mapped).
    pub fn home_station(&self, id: u32) -> Option<u32> {
        let c = self.citizens.get(id as usize)?;
        match *self.cell_station.get(c.home_cell as usize)? {
            s if s >= 0 => Some(s as u32),
            _ => None,
        }
    }
    /// Nearest served station to the citizen's WORK cell.
    pub fn work_station(&self, id: u32) -> Option<u32> {
        let c = self.citizens.get(id as usize)?;
        match *self.cell_station.get(c.work_cell as usize)? {
            s if s >= 0 => Some(s as u32),
            _ => None,
        }
    }
}

/// Resolve a name index to "First Last" from the (Singapore-flavoured, multicultural) name tables.
/// Pure + deterministic; the modulo decouples it from the exact table lengths.
pub fn citizen_name(name_idx: u32) -> String {
    let first = FIRST_NAMES[(name_idx as usize / LAST_NAMES.len()) % FIRST_NAMES.len()];
    let last = LAST_NAMES[name_idx as usize % LAST_NAMES.len()];
    format!("{first} {last}")
}

const FIRST_NAMES: &[&str] = &[
    "Wei", "Mei", "Jun", "Hui", "Xin", "Li", "Ying", "Ming", "Hao", "Ling",
    "Siti", "Nur", "Aisyah", "Faiz", "Rizwan", "Hafiz", "Aniq", "Farah", "Iskandar", "Zara",
    "Arjun", "Priya", "Ravi", "Deepa", "Karthik", "Anita", "Vijay", "Lakshmi", "Suresh", "Divya",
    "Daniel", "Grace", "Marcus", "Chloe", "Ryan", "Emma", "Aaron", "Sophia", "Nathan", "Olivia",
    "Wen", "Jia", "Yusof", "Imran", "Meera", "Tara", "Ethan", "Hannah", "Bryan", "Sarah",
];

const LAST_NAMES: &[&str] = &[
    "Tan", "Lim", "Lee", "Ng", "Wong", "Goh", "Chua", "Koh", "Teo", "Ong",
    "Bin Rahman", "Binte Yusof", "Ismail", "Rahim", "Hassan", "Abdullah",
    "Kumar", "Raj", "Nair", "Pillai", "Menon", "Reddy",
    "Sim", "Chan", "Low", "Yeo", "Toh", "Chong", "Ho", "Fernandez", "D'Cruz", "Singh",
];
