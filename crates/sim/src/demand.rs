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
/// Distance-decay scale for destination attractiveness (mm).
const DEST_DECAY_MM: f64 = 3_000_000.0;

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

    world.captured_origin = origin;
    world.captured_dest = dest;
    world.spawn_accum.resize(n, 0.0);
    world.waiting.resize_with(n, Default::default);
    world.boardings.resize(n, 0);
    world.alightings.resize(n, 0);
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
        ref captured_origin,
        ref captured_dest,
        ref mut spawn_accum,
        ref mut waiting,
        ref mut rng,
        ref mut route_cache,
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
        while spawn_accum[s] >= 1.0 {
            spawn_accum[s] -= 1.0;
            if let Some(dest) =
                pick_dest(stations, serving, captured_origin, captured_dest, bias, s, rng)
            {
                // Route across the network (transfers at interchanges), cached per O/D pair.
                let entry = route_cache
                    .entry((s as u32, dest.0))
                    .or_insert_with(|| router.plan(lines, serving, StationId(s as u32), dest, max_legs));
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

/// Pick a network-wide destination among ALL served stations, weighted by attractiveness
/// (AM→jobs, PM→homes) × distance-decay. Integer weighted draw from a seeded RNG.
#[allow(clippy::too_many_arguments)]
fn pick_dest(
    stations: &[Station],
    serving: &[Vec<crate::ids::LineId>],
    captured_origin: &[f32],
    captured_dest: &[f32],
    bias: f32,
    origin: usize,
    rng: &mut ChaCha8Rng,
) -> Option<StationId> {
    let opos = stations[origin].pos;
    let mut cands: Vec<(StationId, u64)> = Vec::new();
    let mut total: u64 = 0;
    for d_idx in 0..stations.len() {
        if d_idx == origin || serving.get(d_idx).map(|v| v.is_empty()).unwrap_or(true) {
            continue;
        }
        let dist = opos.dist_mm(&stations[d_idx].pos).max(1) as f64;
        // AM: pulled toward jobs (captured_dest); PM: toward homes (captured_origin).
        let cd = captured_dest.get(d_idx).copied().unwrap_or(0.0);
        let cor = captured_origin.get(d_idx).copied().unwrap_or(0.0);
        let attract = (bias * cd + (1.0 - bias) * cor) as f64;
        let decay = 1.0 / (1.0 + dist / DEST_DECAY_MM);
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
