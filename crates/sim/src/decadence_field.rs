//! The decadence area-control field's STATIC topology (fantasy S10a). The spatial corruption tide
//! creeps over the baked continent toward the capital; this module derives the substrate it spreads
//! over — the hex-cell domain, its adjacency, the cheap creep gradient (hop distance to the capital),
//! and the far-edge reservoir seed — ONCE at construction from `CityData` (the buildability raster +
//! the baked capital). It is a pure function of `CityData`, reconstructible on replay exactly like
//! `World::build_lookup`, so it is **NOT hashed** (never enters `Canonical`). The dynamic, hashed
//! per-cell decadence values (the creeping tide + PURGE/DIFFUSE CA) layer on top in S10b; this is only
//! the board they move on. Empty for transit and for demo-arcadia worlds with no baked terrain — so it
//! is golden-neutral (it adds no state to the hash; S10b's field is the re-pin).
//!
//! Determinism: cells are sorted by axial `(q, r)` (index-stable order), adjacency + BFS iterate in that
//! order, and every float is confined to `hexgrid` (immediately quantised to integer axials), so two
//! builds of the same `CityData` are bit-identical (`tests/decadence_field.rs` pins it).
use crate::city::CityData;
use crate::geo_local::PointMm;
use crate::hexgrid::{self, Axial};
use crate::world::World;
use std::collections::VecDeque;

/// The reservoir (tide source) = every cell within this many hops of the MAXIMUM creep distance — a
/// "far edge" BAND, not a fixed count. A band is symmetric by construction (a fixed top-N truncation
/// tie-breaks by axial order and would bias the tide off a symmetric map); it is bounded by the domain.
const RESERVOIR_BAND: u32 = 2;

/// Is a terrain class part of the CA domain — passable land the corruption spreads over? Class codes
/// (`build_world.py`): 4=WATER, 6=MOUNTAIN (impassable ridge), 7=HILL, 8=FOREST, 9=LEY, 10=PLAIN. The
/// tide creeps over PASSABLE land only (not water, not the impassable ridge), routing around mountains
/// through the carved passes — matching the bake's loseability guarantee (the reservoir reaches the
/// capital over passable land). 0 (the `_ => Open` default) is treated as non-domain.
fn is_passable(c: u8) -> bool {
    matches!(c, 7 | 8 | 9 | 10)
}

/// Static topology of the decadence CA: the cell domain (axial, index-ordered), CSR hex adjacency, the
/// creep distance-to-capital gradient, the capital cell, and the reservoir seed. NOT hashed.
#[derive(Clone, Default)]
pub struct DecadenceField {
    /// Domain cells as axial `(q, r)`, sorted ⇒ deterministic, index-stable order. Index = CellId.
    pub cells: Vec<Axial>,
    /// CSR hex adjacency: cell `i`'s present neighbours are `nbr_flat[nbr_start[i]..nbr_start[i + 1]]`.
    pub nbr_start: Vec<u32>,
    pub nbr_flat: Vec<u32>,
    /// Hop distance from the capital over the domain (BFS); `u32::MAX` = unreachable. The cheap creep
    /// gradient toward the capital — computed once, never per tick.
    pub dist_to_capital: Vec<u32>,
    /// The capital cell (the lose target), if the baked capital falls on / near a domain cell.
    pub capital: Option<u32>,
    /// The maximum creep distance (the reservoir's distance to the capital) — the tide's full advance
    /// span, used to scale the derived lose meter. 0 if no capital / disconnected.
    pub max_dist: u32,
    /// Tide-origin seed cells: the cells farthest from the capital (the "far edge"), reachable to it.
    pub reservoir: Vec<u32>,
    /// Axial → CellId lookup (the inverse of `cells`). Static topology, queried not iterated (like
    /// `World::build_lookup`), so NOT hashed — used to map a station's mm to its domain cell for PURGE.
    pub index: rustc_hash::FxHashMap<Axial, u32>,
}

impl DecadenceField {
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    /// Cell `i`'s in-domain hex neighbours (CSR slice).
    pub fn neighbors(&self, i: u32) -> &[u32] {
        let s = self.nbr_start[i as usize] as usize;
        let e = self.nbr_start[i as usize + 1] as usize;
        &self.nbr_flat[s..e]
    }

    /// Build the static CA topology from the baked terrain. Empty (no CA) when there is no terrain
    /// (transit / demo arcadia) or no `grid_cell_mm`. Deterministic.
    pub fn build(city: &CityData) -> Self {
        let size = city.grid_cell_mm;
        if size <= 0 || city.buildability.cells.is_empty() {
            return Self::default();
        }
        // 1. Domain = passable land cells, reinterpreted as axial via the SAME hexgrid transform the
        //    bake quantised them with (so a baked hex centre maps back to its own cell). Sort + dedup
        //    for a deterministic, index-stable cell order.
        let mut cells: Vec<Axial> = city
            .buildability
            .cells
            .iter()
            .filter(|c| is_passable(c.c))
            .map(|c| hexgrid::axial_of(PointMm::new(c.x_mm, c.y_mm), size))
            .collect();
        cells.sort_unstable();
        cells.dedup();
        if cells.is_empty() {
            return Self::default();
        }
        // Construction-only lookup (queried, never iterated — order-independent, like `build_lookup`).
        let index: rustc_hash::FxHashMap<Axial, u32> =
            cells.iter().enumerate().map(|(i, &a)| (a, i as u32)).collect();

        // 2. CSR adjacency over the 6 pointy-top hex neighbours present in the domain.
        const DIRS: [Axial; 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];
        let mut nbr_start: Vec<u32> = Vec::with_capacity(cells.len() + 1);
        let mut nbr_flat: Vec<u32> = Vec::new();
        nbr_start.push(0);
        for &(q, r) in &cells {
            for &(dq, dr) in &DIRS {
                if let Some(&j) = index.get(&(q + dq, r + dr)) {
                    nbr_flat.push(j);
                }
            }
            nbr_start.push(nbr_flat.len() as u32);
        }

        // 3. Capital cell: a baked world (a non-empty domain ⇒ we are here) always has one — the seat
        //    the tide races toward. The baked capital mm → axial; if that exact cell isn't in the domain
        //    (a coarse bin landed it just off a passable cell), snap to the nearest domain cell (index-
        //    ordered argmin over hex distance ⇒ deterministic). `(0, 0)` is a legitimate origin cell, NOT
        //    an "unset" sentinel — non-baked worlds already returned empty above.
        let ca = hexgrid::axial_of(PointMm::new(city.capital_x_mm, city.capital_y_mm), size);
        let capital = index.get(&ca).copied().or_else(|| {
            let mut best: Option<(i64, u32)> = None;
            for (i, &cell) in cells.iter().enumerate() {
                let d = hexgrid::distance(cell, ca);
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, i as u32));
                }
            }
            best.map(|(_, i)| i)
        });

        // 4. Creep distance: BFS from the capital over the domain (unweighted ⇒ unique shortest-path
        //    distances regardless of queue order). `u32::MAX` = unreachable (walled off by water/ridge).
        let mut dist_to_capital = vec![u32::MAX; cells.len()];
        if let Some(cap) = capital {
            dist_to_capital[cap as usize] = 0;
            let mut queue: VecDeque<u32> = VecDeque::new();
            queue.push_back(cap);
            while let Some(c) = queue.pop_front() {
                let nd = dist_to_capital[c as usize] + 1;
                let s = nbr_start[c as usize] as usize;
                let e = nbr_start[c as usize + 1] as usize;
                for &n in &nbr_flat[s..e] {
                    if dist_to_capital[n as usize] == u32::MAX {
                        dist_to_capital[n as usize] = nd;
                        queue.push_back(n);
                    }
                }
            }
        }

        // 5. Reservoir = the far-edge BAND: every reachable cell within `RESERVOIR_BAND` hops of the
        //    maximum creep distance (the far edge opposite the capital — the tide origin). A band (not a
        //    top-N truncation) so the seed is mirror-symmetric on a symmetric map; sorted (dist desc,
        //    axial) for a deterministic order. Empty if nothing is reachable (no capital / disconnected).
        let maxd = dist_to_capital.iter().copied().filter(|&d| d != u32::MAX).max().unwrap_or(0);
        let mut reservoir: Vec<u32> = if maxd > 0 {
            (0..cells.len() as u32)
                .filter(|&i| {
                    let d = dist_to_capital[i as usize];
                    d != u32::MAX && d > 0 && d + RESERVOIR_BAND >= maxd
                })
                .collect()
        } else {
            Vec::new()
        };
        reservoir.sort_by(|&a, &b| {
            dist_to_capital[b as usize]
                .cmp(&dist_to_capital[a as usize])
                .then(cells[a as usize].cmp(&cells[b as usize]))
        });

        DecadenceField { cells, nbr_start, nbr_flat, dist_to_capital, capital, reservoir, index, max_dist: maxd }
    }
}

// ── S10b: the dynamic decadence CA (the creeping tide) ────────────────────────────────────────────
// The hashed per-cell field (`World::decadence_cells`, dense over `DecadenceField::cells`) evolves by a
// DOUBLE-BUFFERED integer diffusion that creeps from the reservoir toward the capital, with PURGE (the
// player's rail network holds the line) STRICTLY DOMINATING DIFFUSE. Determinism: integer, index-ordered;
// the only map read is `index.get` (queried, never iterated). Bounded per tick by the domain size (the
// hard cap `MAX_CA_CELLS`, binding condition #3) — a bench-gate test pins the per-tick work.
//
// S10b-1 scope: the engine runs PARALLEL to the scalar `decadence` lose meter (unchanged). Rewiring the
// lose condition to the field reaching the capital + re-tuning the creep against the baked world is S10b-2.

/// Fully-corrupted cell value — the saturation ceiling + the reservoir seed level.
pub const DECAD_MAX: i32 = 1000;
/// A cell advances (the tide flows in) only from a FARTHER-from-capital neighbour at least this corrupt,
/// so the front is a gradient creeping capital-ward, not an instant flood. A ring reaches this in
/// `ADVANCE_THRESHOLD / gain` ticks, so the front advances one ring at that cadence.
const ADVANCE_THRESHOLD: i32 = 100;
/// Default diffuse gain per sim-second (→ +10 per 50 ms tick) when a city sets no `decadence_creep_per_s`
/// — the FAST rate the S10b field tests rely on. A baked world overrides it with a slow rate (a
/// multi-minute creep to the capital): at gain 1/tick the front advances one ring per ~100 ticks.
pub const DEFAULT_CREEP_PER_S: i64 = 200;
/// The tide has "reached the capital" (the realm falls) once its front is within this many hexes of the
/// capital — LARGER than the capital barracks's `PURGE_RADIUS`, so a lone capital can't make the realm
/// unloseable: the player must extend the network's purge ring outward to actually hold the heartland.
pub(crate) const LOSE_DIST: u32 = 3;
/// PURGE per sim-second for a network-covered cell (→ −100 per tick): 10× the diffuse gain, so PURGE
/// STRICTLY DOMINATES DIFFUSE — held ground trends to 0 (the build-plan invariant).
const PURGE_PER_S: i64 = 2000;
/// Hex radius the player's network purges around each station (the rail presence holds the frontier).
const PURGE_RADIUS: u32 = 2;
/// Hard cap on CA domain cells (binding condition #3 — the perf-cliff guard). A baked world is sized
/// conservatively below this (the map-gen "Open decisions"); a larger domain DISABLES the CA rather than
/// risk a per-tick bloom. The step is O(domain) ≤ O(MAX_CA_CELLS) — trivially within the 20–30 Hz budget.
pub const MAX_CA_CELLS: usize = 30_000;

/// One CA tick on the baked decadence field. Double-buffered (read `cur`, write a scratch `next`) so the
/// neighbour-reading diffusion never reads half-updated cells. No-op when there is no field (transit /
/// demo) or the domain exceeds the hard cap. Integer + index-ordered ⇒ deterministic.
pub(crate) fn step(world: &mut World, dt_ms: i64) {
    let n = world.decadence_field.len();
    if n == 0 || n > MAX_CA_CELLS {
        return;
    }
    if world.decadence_cells.len() != n {
        world.decadence_cells.resize(n, 0);
    }
    let dt = dt_ms.max(0);
    let mut creep = if world.city.decadence_creep_per_s > 0 {
        world.city.decadence_creep_per_s
    } else {
        DEFAULT_CREEP_PER_S
    };
    // S11 SAPPERS: the tide creeps at HALF rate when the tech is unlocked (buys defensive runway). 0 ⇒
    // full rate, byte-identical (transit has no field; pre-tech arcadia keeps `tech_unlocked` 0).
    if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::SAPPERS) {
        creep = (creep / 2).max(1);
    }
    // CONTINUOUS gain via a sub-unit accumulator (replaces the old `.max(1)` floor): `creep·dt/1000`
    // truncates a slow rate (< 20/s at dt=50) to 0, which froze the tide; flooring at 1 fixed the freeze
    // but made every rate < 20/s identical (a coarse knob, min ~17-min runway). Now the milli-gain
    // accrues across ticks (`decadence_gain_accum`) and whole units are extracted — so a slow creep
    // advances at its true average rate (the front steps forward on rollover ticks), enabling a tunable
    // multi-game-minute → multi-day runway. A rate ≥ 20/s rolls over every tick (exact integer gain, no
    // remainder) ⇒ byte-identical to the floor for the default/baked rates.
    world.decadence_gain_accum = world.decadence_gain_accum.saturating_add(creep.saturating_mul(dt));
    let gain = (world.decadence_gain_accum / 1000) as i32;
    world.decadence_gain_accum -= gain as i64 * 1000;
    let purge = (PURGE_PER_S.saturating_mul(dt) / 1000) as i32;
    let field = &world.decadence_field;

    // PURGE mask: cells within PURGE_RADIUS of a station ON A BUILT LINE — the player's RAIL NETWORK
    // holds the line, not isolated unconnected nodes (the baked world seeds ~40 resource/town stations
    // with no track; those must NOT suppress the tide, or the map would start immune to it). So a station
    // purges only once the player rails it into the network. Bounded BFS per stop over the field
    // adjacency; index-ordered ⇒ deterministic; `index.get` is a query, never an iteration.
    let mut purged = vec![false; n];
    let size = world.city.grid_cell_mm.max(1);
    for line in &world.lines {
        if line.removed {
            continue;
        }
        for stop in &line.stops {
            let Some(s) = world.stations.get(stop.index()) else { continue };
            if s.removed {
                continue;
            }
            let Some(&start) = field.index.get(&hexgrid::axial_of(s.pos, size)) else { continue };
            if purged[start as usize] {
                continue; // already covered by another stop's disk
            }
            let mut frontier = vec![start];
            purged[start as usize] = true;
            for _ in 0..PURGE_RADIUS {
                let mut nextf = Vec::new();
                for &c in &frontier {
                    for &nb in field.neighbors(c) {
                        if !purged[nb as usize] {
                            purged[nb as usize] = true;
                            nextf.push(nb);
                        }
                    }
                }
                frontier = nextf;
            }
        }
    }

    // Double-buffered creep: read `cur`, write `next`.
    let cur = &world.decadence_cells;
    let mut next = cur.clone();
    // 1. SEED the reservoir (the inexhaustible far-edge source).
    for &res in &field.reservoir {
        next[res as usize] = DECAD_MAX;
    }
    // 2. DIFFUSE toward the capital: a cell gains iff a FARTHER-from-capital neighbour is corrupt enough.
    for c in 0..n {
        let dc = field.dist_to_capital[c];
        let s = field.nbr_start[c] as usize;
        let e = field.nbr_start[c + 1] as usize;
        let advancing = field.nbr_flat[s..e]
            .iter()
            .any(|&nb| field.dist_to_capital[nb as usize] > dc && cur[nb as usize] >= ADVANCE_THRESHOLD);
        if advancing {
            next[c] = (next[c] + gain).min(DECAD_MAX);
        }
    }
    // 3. PURGE: the network holds the line — strictly dominates the diffuse gain ⇒ held ground → 0.
    for c in 0..n {
        if purged[c] {
            next[c] = (next[c] - purge).max(0);
        }
    }
    world.decadence_cells = next;

    // 4. S10b-2 — derive the global lose meter from the tide's FRONT (the nearest-to-capital corrupted
    // cell), scaled so it hits `CAPITAL_THRESHOLD` exactly when the front reaches `LOSE_DIST`. The
    // network's PURGE pushes the front back ⇒ lowers the meter (build/hold to survive); the tide reaching
    // the capital ⇒ the realm falls. Overwrites the scalar `decadence` for baked worlds (the `war_step`
    // branch runs THIS instead of `decadence::step` when a field exists ⇒ no double-count).
    let max_dist = world.decadence_field.max_dist;
    let mut front = max_dist;
    for c in 0..n {
        if world.decadence_cells[c] >= FRONT_THRESHOLD {
            let d = world.decadence_field.dist_to_capital[c];
            if d < front {
                front = d;
            }
        }
    }
    let span = max_dist.saturating_sub(LOSE_DIST).max(1) as i64;
    let advanced = max_dist.saturating_sub(front) as i64;
    let front_meter = crate::decadence::CAPITAL_THRESHOLD.saturating_mul(advanced).saturating_div(span);
    // S11 RIVAL: add the permanent raider-breach floor on top of the tide front (both bounded by the
    // capital threshold). 0 without raiders ⇒ byte-identical to the pre-rival derivation.
    world.decadence = front_meter
        .saturating_add(world.raider_breach)
        .clamp(0, crate::decadence::CAPITAL_THRESHOLD);
}

/// A cell counts as part of the tide FRONT (for the derived lose meter) once it carries any corruption;
/// a network-PURGEd cell drops back to 0 and stops counting, so holding the line retreats the front.
pub(crate) const FRONT_THRESHOLD: i32 = 1;
