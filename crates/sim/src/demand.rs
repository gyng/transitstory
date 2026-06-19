//! Demand: catchment capture (gravity, normalized so a cell's weight is SHARED across
//! in-range stations — no double-counting) + deterministic seeded passenger spawn with a
//! gravity destination pick. Routing here is the single-line direct-ride case; the data is
//! shaped so RAPTOR/transfers slot in later behind a Router trait (PLAN §6).
use crate::ids::StationId;
use crate::pax::Pax;
use crate::station::Station;
use crate::world::World;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;

/// Catchment radius (mm). ~500 m walk shed.
pub const CATCHMENT_MM: i64 = 500_000;
/// Expected passengers per second per unit of captured origin weight (× this × dt_ms).
pub const DEMAND_RATE_PER_MS: f32 = 1.0e-5;
/// Distance-decay scale for destination attractiveness (mm) — the geometric fallback used only
/// when the router exposes no accessibility data. (Pure distance: frame-free.)
const DEST_DECAY_MM: f64 = 3_000_000.0;
/// Accessibility-decay scale (sim-ms): a destination's pull halves at ~this transit travel time —
/// 15 CLOCK-minutes. Travel times shrank ×CLOCK_SCALE in the unification, so this decay anchor
/// shrank with them; the accessibility weighting over real geography is unchanged.
const ACCESS_DECAY_MS: f64 = 30_000.0;
/// Max walking-transfer distance (mm) between two stations — ~400 m, a generous interchange walk
/// shed. Stops closer than this form a footpath interchange even on unconnected lines.
pub(crate) const FOOTPATH_MM: i64 = 400_000;
/// Walking speed (mm per sim-second) for the footpath time estimate — ~1.4 m/s ON THE CLOCK
/// (×CLOCK_SCALE frame): a 400 m interchange walk costs ~5 clock-minutes, like before.
pub(crate) const WALK_SPEED_MM_S: i64 = 42_000;

/// Integer walk time (ms) for a footpath of `dist_mm`: dist / speed, with the mm/s → ms ×1000.
#[inline]
pub(crate) fn walk_ms(dist_mm: i64) -> i64 {
    dist_mm.max(0).saturating_mul(1000) / WALK_SPEED_MM_S
}

/// Transit-oriented demand growth — the one-more-day engine. Once per in-game day (clock-derived,
/// deterministic), every demand cell grows: cells within a catchment of a SERVED station grow at
/// the city's full `growth_bp_per_day` (the city densifies around the transit you run), the rest
/// at a third of it (ambient sprawl — a network you stop extending slowly falls behind). Both
/// origin and dest weights grow, capped at `growth_cap_w` (2× the strongest initial cell).
/// Crow-flies distance, not the walkshed: growth is "near transit", and the cheap test keeps the
/// once-a-day pass O(cells × served). Multiplicative, so empty cells stay empty (growth densifies,
/// it doesn't invent demand from nothing). Sets `demand_dirty` so capture recomputes.
pub(crate) fn grow(world: &mut World) {
    let bp = world.city.growth_bp_per_day;
    if bp <= 0 {
        return;
    }
    let day = world.clock_ms / (24 * crate::tod::HOUR_MS);
    if day <= world.last_growth_day {
        return;
    }
    world.last_growth_day = day;

    let served_pos: Vec<crate::geo_local::PointMm> = world
        .stations
        .iter()
        .enumerate()
        .filter(|(s, st)| !st.removed && world.serving.get(*s).map(|v| !v.is_empty()).unwrap_or(false))
        .map(|(_, st)| st.pos)
        .collect();

    let near = 1.0 + bp as f32 / 10_000.0;
    let ambient = 1.0 + bp as f32 / 30_000.0; // a third of the transit-adjacent rate
    let cap = world.growth_cap_w;
    for cell in world.city.demand.cells.iter_mut() {
        let cpos = crate::geo_local::PointMm::new(cell.x_mm, cell.y_mm);
        let factor = if served_pos.iter().any(|p| p.dist_mm(&cpos) <= CATCHMENT_MM) { near } else { ambient };
        cell.origin_w = (cell.origin_w * factor).min(cap);
        cell.dest_w = (cell.dest_w * factor).min(cap);
    }
    world.demand_dirty = true; // captured weights + coverage re-derive from the grown grid

    // Agent demand grows with the city: top the population up to the new (homes-derived) target,
    // drawn from the GROWN grid — new residents move in where the growth happened. Append-only +
    // seed-keyed, so replays redraw identical citizens (see Population::grow_to).
    if world.agent_demand {
        if let Some(mut pop) = world.population.take() {
            let target = world.agent_population_target();
            pop.grow_to(world, target, world.seed);
            world.population = Some(pop);
        }
    }
}

/// Recompute per-station captured origin/destination weight from the demand grid. Each
/// cell's weight is distributed across in-range stations by normalized decay, so the total
/// captured per cell never exceeds the cell weight. Cheap; runs only when `demand_dirty`.
pub(crate) fn prepare(world: &mut World) {
    if !world.demand_dirty {
        return;
    }
    world.demand_dirty = false;
    let n = world.stations.len();

    let stations = &world.stations;
    let cells = &world.city.demand.cells;
    let build_lookup = &world.build_lookup;
    let build_cell_mm = world.build_cell_mm;
    let r = CATCHMENT_MM as f64;
    let mut origin = vec![0f32; n];
    let mut dest = vec![0f32; n];
    // Fantasy S7e: per-station captured origin weight broken down BY commodity, so a station's output
    // commodity = its argmax. Allocated regardless (cheap: n × N_COMMODITIES f32) but only meaningful for
    // arcadia (transit cells are all commodity 0, so the argmax is trivially ORE and `produce` never runs).
    let mut origin_by_comm = vec![[0f32; crate::forge::N_COMMODITIES]; n];
    // ...and captured DEST weight by commodity, so a SINK's recipe = the commodities it requires (S7e-2).
    let mut dest_by_comm = vec![[0f32; crate::forge::N_COMMODITIES]; n];
    let mut in_range: Vec<(usize, f64)> = Vec::new();

    for cell in cells {
        in_range.clear();
        let mut sum_w = 0.0;
        let cell_pos = crate::geo_local::PointMm::new(cell.x_mm, cell.y_mm);
        for (si, st) in stations.iter().enumerate() {
            if st.removed {
                continue; // a bulldozed station captures nothing; its share frees up for neighbours
            }
            // Walk-shed distance, not crow-flies: water severs the share entirely and a crossed
            // road/rail corridor inflates it (a None drops the cell from this station's shed).
            let eff = match crate::walkshed::effective_walk_dist(build_lookup, build_cell_mm, st.pos, cell_pos, r) {
                Some(e) => e,
                None => continue,
            };
            let t = eff / r;
            let w = (-(t * t)).exp(); // gaussian-ish decay, > 0 within range
            in_range.push((si, w));
            sum_w += w;
        }
        if sum_w > 0.0 {
            let comm = (cell.commodity as usize).min(crate::forge::N_COMMODITIES - 1);
            for &(si, w) in &in_range {
                let frac = (w / sum_w) as f32;
                origin[si] += cell.origin_w * frac;
                dest[si] += cell.dest_w * frac;
                origin_by_comm[si][comm] += cell.origin_w * frac;
                dest_by_comm[si][comm] += cell.dest_w * frac;
            }
        }
    }
    // Each station's output commodity = the commodity it captures the most ORIGIN weight of (argmax;
    // ties → lowest index, deterministic). ORE (0) for any station with no captured origin (e.g. a sink).
    let station_commodity: Vec<u8> = origin_by_comm
        .iter()
        .map(|by| {
            let mut best = 0usize;
            for c in 1..crate::forge::N_COMMODITIES {
                if by[c] > by[best] {
                    best = c;
                }
            }
            best as u8
        })
        .collect();
    // Each sink's RECIPE = the distinct commodities it captures DEST weight of (ascending). A
    // commodity-0 world yields [0] everywhere ⇒ `consume` takes the consume-all path ⇒ byte-identical.
    let station_recipe: Vec<Vec<u8>> = dest_by_comm
        .iter()
        .map(|by| (0..crate::forge::N_COMMODITIES).filter(|&c| by[c] > 0.0).map(|c| c as u8).collect())
        .collect();

    // Footpath edges: nearby station pairs walkable on foot (an interchange by foot), with their
    // integer walk time. O(n²) but only on a station change. Index-ordered → deterministic.
    let mut footpaths: Vec<Vec<(u32, i64)>> = vec![Vec::new(); n];
    for i in 0..n {
        if stations[i].removed {
            continue;
        }
        for j in 0..n {
            if i == j || stations[j].removed {
                continue;
            }
            let d = stations[i].pos.dist_mm(&stations[j].pos);
            if d <= FOOTPATH_MM {
                footpaths[i].push((j as u32, walk_ms(d)));
            }
        }
    }
    world.footpaths = footpaths;

    // S7e multi-stage: flatten the per-commodity DEST weights for commodity-aware routing, and flag
    // whether this world uses any PROCESSED good (a processor output, or a sink that wants a mid/final).
    // A raw-only world (transit, the current baked world) ⇒ `has_multistage` false ⇒ routing unchanged.
    let mut flat_dest = vec![0f32; n * crate::forge::N_COMMODITIES];
    let mut has_multistage = false;
    for (s, by) in dest_by_comm.iter().enumerate() {
        for (c, &w) in by.iter().enumerate() {
            flat_dest[s * crate::forge::N_COMMODITIES + c] = w;
            if c >= crate::forge::FIRST_MID && w > 0.0 {
                has_multistage = true;
            }
        }
    }
    if station_commodity.iter().any(|&c| (c as usize) >= crate::forge::FIRST_MID) {
        has_multistage = true;
    }

    world.captured_origin = origin;
    world.captured_dest = dest;
    // #5 the conquest-gauge denominator: the BAKED town-sink count. Track its MAX during BUILD mode — the network
    // grows as the baked supply graph + the player's initial build are placed (prepare re-runs on each change, and
    // the full-network prepare often lands in build mode before the first Run) — then FREEZE it permanently once
    // the player Runs (the one-way `baked_locked` latch, NOT the toggling `running` flag — else a Run→Build→place-
    // unserved-sink→Run loop would re-arm the .max() and dip the gauge on a strict-superset network). ≈ the live
    // count (the baked towns never leave), so no gauge re-tune. A pure-build session keeps tracking until first Run.
    if !world.running && !world.baked_locked {
        let count = world
            .captured_dest
            .iter()
            .zip(world.captured_origin.iter())
            .filter(|(&cd, &co)| cd > co && cd > 0.0)
            .count() as i64;
        world.baked_town_sinks = world.baked_town_sinks.max(count);
    }
    world.dest_by_comm = flat_dest;
    world.has_multistage = has_multistage;
    world.station_commodity = station_commodity;
    world.station_recipe = station_recipe;
    world.spawn_accum.resize(n, 0.0);
    world.waiting.resize_with(n, Default::default);
    world.boardings.resize(n, 0);
    world.alightings.resize(n, 0);
    world.denied_at.resize(n, 0);
    world.abandoned_at.resize(n, 0);
}

/// Spawn passengers this tick at served stations (deterministic accumulator for count;
/// seeded RNG only for the gravity destination pick).
pub(crate) fn spawn(world: &mut World, dt_ms: i64) {
    // Transit gravity: time-of-day modulates overall volume + AM(home→work)/PM(work→home) direction.
    let hour = crate::tod::hour_of_day(world.clock_ms);
    let mult = crate::tod::demand_multiplier(hour);
    let bias = crate::tod::work_bias(hour);
    spawn_modulated(world, dt_ms, mult, bias, None);
}

/// The shared spawn+route body, parameterized by demand `mult` (overall volume) and `bias`
/// (origin↔dest directionality: `bias=1.0` spawns at origins/sources and routes to dests/sinks).
/// Gravity passes time-of-day values; the fantasy `SupplyChainDemand` passes steady `(1.0, 1.0)` for a
/// constant source→sink commodity flow with no commuter rush. The `world.rng` draw order (per served
/// station, then per spawned token via `pick_dest`) is IDENTICAL for both callers ⇒ each mode's golden
/// pin is independently stable; the parameters only scale/steer, never reorder the draws.
pub(crate) fn spawn_modulated(world: &mut World, dt_ms: i64, mult: f32, bias: f32, mut gate: Option<&mut [i64]>) {
    let n = world.stations.len();
    if n == 0 {
        return;
    }
    let now = world.clock_ms;
    let max_legs = world.max_legs;

    let World {
        ref stations,
        ref lines,
        ref serving,
        ref footpaths,
        ref captured_origin,
        ref captured_dest,
        ref dest_by_comm,
        has_multistage,
        ref station_commodity,
        ref mut spawn_accum,
        ref mut waiting,
        ref mut rng,
        ref mut route_cache,
        ref mut access_cache,
        ref router,
        ..
    } = *world;

    // Reused destination-candidate scratch (`pick_dest` clears + refills it each call) — the same
    // contents and the same weighted-draw order as a fresh `Vec`, just allocated once per spawn pass
    // instead of once per token. Behaviour byte-identical.
    let mut cands: Vec<(StationId, u64)> = Vec::new();

    for s in 0..n {
        // Only stations on an operational line originate trips.
        if serving.get(s).map(|v| v.is_empty()).unwrap_or(true) {
            continue;
        }
        // Trip origins: AM weights residential (captured_origin), PM weights jobs (captured_dest).
        let co = captured_origin.get(s).copied().unwrap_or(0.0);
        let cd = captured_dest.get(s).copied().unwrap_or(0.0);
        let origin_strength = bias * co + (1.0 - bias) * cd;
        if origin_strength <= 0.0 {
            continue;
        }

        spawn_accum[s] += origin_strength * DEMAND_RATE_PER_MS * mult * dt_ms as f32;
        if spawn_accum[s] < 1.0 {
            continue; // nobody spawns this tick → skip the (lazy, cached) accessibility solve
        }
        // One-to-all transit travel time from this origin, cached until the network changes.
        let access = access_cache
            .entry(s as u32)
            .or_insert_with(|| router.reachable(lines, serving, footpaths, StationId(s as u32), max_legs));
        while spawn_accum[s] >= 1.0 {
            // Buffer gate (fantasy, S7b): a node ships only what it has produced. When its per-station
            // ship budget is exhausted, stop and DROP the whole-unit backlog (keep only the sub-unit
            // remainder) — a steady flow, not an order queue that would burst on refill. `None`
            // (transit gravity) ⇒ unbounded, so the branch is skipped and behaviour is byte-identical.
            if let Some(g) = gate.as_deref() {
                if g.get(s).copied().unwrap_or(0) <= 0 {
                    spawn_accum[s] = spawn_accum[s].fract();
                    break;
                }
            }
            spawn_accum[s] -= 1.0;
            // S7e: a cart carries its origin's output commodity; commodity-aware routing (multi-stage
            // worlds only) sends it to a node that WANTS that commodity, not the highest-total-dest node.
            let trip_commodity = station_commodity.get(s).copied().unwrap_or(0) as usize;
            if let Some(dest) = pick_dest(
                stations, serving, captured_origin, captured_dest, dest_by_comm, has_multistage,
                trip_commodity, access, bias, s, rng, &mut cands,
            ) {
                // Route across the network (transfers at interchanges), cached per O/D pair.
                let entry = route_cache
                    .entry((s as u32, dest.0))
                    .or_insert_with(|| router.plan(lines, serving, footpaths, StationId(s as u32), dest, max_legs));
                if let Some(legs) = entry {
                    if !legs.is_empty() {
                        // A commodity was actually shipped ⇒ consume one unit from the node's buffer.
                        if let Some(g) = gate.as_deref_mut() {
                            if let Some(v) = g.get_mut(s) {
                                *v -= 1;
                            }
                        }
                        waiting[s].push_back(Pax {
                            legs: legs.clone(),
                            leg: 0,
                            t_spawn_ms: now,
                            t_wait_ms: now,
                            citizen_id: u32::MAX, // anonymous gravity trip
                            // S7e: the cart carries its source's output commodity → delivered to the sink's
                            // matching buffer slot. 0 (ORE) for transit (station_commodity all 0 there).
                            commodity: station_commodity.get(s).copied().unwrap_or(0),
                        });
                    }
                }
            }
        }
    }
}

/// Expected destination weights from `origin` over all served stations — the SAME gravity
/// (attractiveness × accessibility-decay) weights `pick_dest` samples from, minus the `+1` RNG
/// baseline, so the OD "desire line" overlay shows where riders from here are actually drawn.
/// Pure read: solves accessibility fresh (no cache mutation). Returns `(dest_index, weight)` for
/// served, non-removed destinations with positive pull; unsorted. Empty if `origin` isn't served.
pub fn od_weights(world: &World, origin: usize) -> Vec<(u32, f64)> {
    let n = world.stations.len();
    if origin >= n || world.serving.get(origin).map(|v| v.is_empty()).unwrap_or(true) {
        return Vec::new();
    }
    let bias = crate::tod::work_bias(crate::tod::hour_of_day(world.clock_ms));
    let access = world
        .router
        .reachable(&world.lines, &world.serving, &world.footpaths, StationId(origin as u32), world.max_legs);
    let use_access = !access.is_empty();
    let opos = world.stations[origin].pos;
    let mut out: Vec<(u32, f64)> = Vec::new();
    for d in 0..n {
        if d == origin
            || world.stations[d].removed
            || world.serving.get(d).map(|v| v.is_empty()).unwrap_or(true)
        {
            continue;
        }
        let cd = world.captured_dest.get(d).copied().unwrap_or(0.0);
        let cor = world.captured_origin.get(d).copied().unwrap_or(0.0);
        let attract = (bias * cd + (1.0 - bias) * cor) as f64;
        let decay = if use_access {
            match access.get(d).copied() {
                Some(t) if t < i64::MAX => ACCESS_DECAY_MS / (ACCESS_DECAY_MS + t as f64),
                _ => 0.0,
            }
        } else {
            let dist = opos.dist_mm(&world.stations[d].pos).max(1) as f64;
            1.0 / (1.0 + dist / DEST_DECAY_MM)
        };
        let w = attract * decay;
        if w > 0.0 {
            out.push((d as u32, w));
        }
    }
    out
}

/// Pick a network-wide destination among ALL served stations, weighted by attractiveness
/// (AM→jobs, PM→homes) × **accessibility-decay** — how fast transit reaches it (wait + ride),
/// not crow-flies metres — so a good network induces demand toward the places it connects well.
/// Falls back to geometric distance-decay when no accessibility data is available (e.g. the
/// router is `BfsRouter`, or geometry isn't solved yet). Integer weighted draw from a seeded RNG.
#[allow(clippy::too_many_arguments)]
fn pick_dest(
    stations: &[Station],
    serving: &[Vec<crate::ids::LineId>],
    captured_origin: &[f32],
    captured_dest: &[f32],
    dest_by_comm: &[f32],
    has_multistage: bool,
    commodity: usize,
    access: &[i64],
    bias: f32,
    origin: usize,
    rng: &mut ChaCha8Rng,
    cands: &mut Vec<(StationId, u64)>,
) -> Option<StationId> {
    let use_access = !access.is_empty();
    let opos = stations[origin].pos;
    cands.clear();
    let mut total: u64 = 0;
    for d_idx in 0..stations.len() {
        if d_idx == origin || serving.get(d_idx).map(|v| v.is_empty()).unwrap_or(true) {
            continue;
        }
        // AM: pulled toward jobs (captured_dest); PM: toward homes (captured_origin). S7e multi-stage:
        // when this world uses processed goods, weight by the destination's dest OF THE CART'S COMMODITY
        // (so a raw goes to its processor, a mid to its final sink) — not the total dest that would lure
        // it to the highest-demand node regardless of what it wants. Raw-only worlds keep total dest
        // (byte-identical: transit + the current baked world never set `has_multistage`).
        let cd = if has_multistage {
            dest_by_comm.get(d_idx * crate::forge::N_COMMODITIES + commodity).copied().unwrap_or(0.0)
        } else {
            captured_dest.get(d_idx).copied().unwrap_or(0.0)
        };
        let cor = captured_origin.get(d_idx).copied().unwrap_or(0.0);
        let attract = (bias * cd + (1.0 - bias) * cor) as f64;
        let decay = if use_access {
            match access.get(d_idx).copied() {
                Some(t) if t < i64::MAX => ACCESS_DECAY_MS / (ACCESS_DECAY_MS + t as f64),
                _ => 0.0, // unreachable within max_legs → only the +1 baseline keeps it possible
            }
        } else {
            let dist = opos.dist_mm(&stations[d_idx].pos).max(1) as f64;
            1.0 / (1.0 + dist / DEST_DECAY_MM)
        };
        let w = (attract * decay * 1000.0) as u64 + 1; // +1 baseline so any station can be chosen
        total += w;
        cands.push((StationId(d_idx as u32), w));
    }
    if cands.is_empty() || total == 0 {
        return None;
    }
    let r = rng.random_range(0..total);
    let mut acc = 0u64;
    for &(st, w) in cands.iter() {
        acc += w;
        if r < acc {
            return Some(st);
        }
    }
    None
}
