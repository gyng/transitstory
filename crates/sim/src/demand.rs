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
/// when the router exposes no accessibility data.
const DEST_DECAY_MM: f64 = 3_000_000.0;
/// Accessibility-decay scale (ms): a destination's pull halves at ~this transit travel time, so
/// well-connected (fast-to-reach) places draw proportionally more trips. ~15 min.
const ACCESS_DECAY_MS: f64 = 900_000.0;
/// Max walking-transfer distance (mm) between two stations — ~400 m, a generous interchange walk
/// shed. Stops closer than this form a footpath interchange even on unconnected lines.
pub(crate) const FOOTPATH_MM: i64 = 400_000;
/// Walking speed (mm/s) for the footpath time estimate (~1.4 m/s). Integer ms throughout.
pub(crate) const WALK_SPEED_MM_S: i64 = 1_400;

/// Integer walk time (ms) for a footpath of `dist_mm`: dist / speed, with the mm/s → ms ×1000.
#[inline]
pub(crate) fn walk_ms(dist_mm: i64) -> i64 {
    dist_mm.max(0).saturating_mul(1000) / WALK_SPEED_MM_S
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
    let r = CATCHMENT_MM as f64;
    let mut origin = vec![0f32; n];
    let mut dest = vec![0f32; n];
    let mut in_range: Vec<(usize, f64)> = Vec::new();

    for cell in cells {
        in_range.clear();
        let mut sum_w = 0.0;
        for (si, st) in stations.iter().enumerate() {
            if st.removed {
                continue; // a bulldozed station captures nothing; its share frees up for neighbours
            }
            let dx = (st.pos.x_mm - cell.x_mm) as f64;
            let dy = (st.pos.y_mm - cell.y_mm) as f64;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= r {
                let t = d / r;
                let w = (-(t * t)).exp(); // gaussian-ish decay, > 0 within range
                in_range.push((si, w));
                sum_w += w;
            }
        }
        if sum_w > 0.0 {
            for &(si, w) in &in_range {
                let frac = (w / sum_w) as f32;
                origin[si] += cell.origin_w * frac;
                dest[si] += cell.dest_w * frac;
            }
        }
    }

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

    world.captured_origin = origin;
    world.captured_dest = dest;
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
    let n = world.stations.len();
    if n == 0 {
        return;
    }
    // Time-of-day modulation: overall volume + AM(home→work)/PM(work→home) directionality.
    let hour = crate::tod::hour_of_day(world.clock_ms);
    let mult = crate::tod::demand_multiplier(hour);
    let bias = crate::tod::work_bias(hour);
    let now = world.clock_ms;
    let max_legs = world.max_legs;

    let World {
        ref stations,
        ref lines,
        ref serving,
        ref footpaths,
        ref captured_origin,
        ref captured_dest,
        ref mut spawn_accum,
        ref mut waiting,
        ref mut rng,
        ref mut route_cache,
        ref mut access_cache,
        ref router,
        ..
    } = *world;

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
            spawn_accum[s] -= 1.0;
            if let Some(dest) =
                pick_dest(stations, serving, captured_origin, captured_dest, access, bias, s, rng)
            {
                // Route across the network (transfers at interchanges), cached per O/D pair.
                let entry = route_cache
                    .entry((s as u32, dest.0))
                    .or_insert_with(|| router.plan(lines, serving, footpaths, StationId(s as u32), dest, max_legs));
                if let Some(legs) = entry {
                    if !legs.is_empty() {
                        waiting[s].push_back(Pax {
                            legs: legs.clone(),
                            leg: 0,
                            t_spawn_ms: now,
                            t_wait_ms: now,
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
    access: &[i64],
    bias: f32,
    origin: usize,
    rng: &mut ChaCha8Rng,
) -> Option<StationId> {
    let use_access = !access.is_empty();
    let opos = stations[origin].pos;
    let mut cands: Vec<(StationId, u64)> = Vec::new();
    let mut total: u64 = 0;
    for d_idx in 0..stations.len() {
        if d_idx == origin || serving.get(d_idx).map(|v| v.is_empty()).unwrap_or(true) {
            continue;
        }
        // AM: pulled toward jobs (captured_dest); PM: toward homes (captured_origin).
        let cd = captured_dest.get(d_idx).copied().unwrap_or(0.0);
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
    for (st, w) in cands {
        acc += w;
        if r < acc {
            return Some(st);
        }
    }
    None
}
