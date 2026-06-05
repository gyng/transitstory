//! The wasm->ts state port: copy-out of the vehicle SoA into flat f32/u32 buffers in
//! local metres (render-only floats — never fed back into state-affecting math). The
//! frontend copies these into a reused typed array each frame (PLAN §0.5: copy, not a
//! long-lived zero-copy view). Empty until T14 dispatches vehicles.
use crate::world::World;

#[inline]
fn mm_to_m(v: i64) -> f32 {
    v as f32 / 1000.0
}

// --- "peeps": individual rider dots (Cities:Skylines-style), purely RENDER-DERIVED ---------------
// A single index-ordered sweep turns the (un-hashed) in-transit passenger set into a flat dot
// buffer: each onboard rider scatters around its train, each waiting rider fans out on its platform
// (or walks IN if freshly spawned), and each recently-arrived rider walks OUT of the station. NONE
// of this enters Canonical (vehicles/waiting/recent_alight are excluded from the hash), so the whole
// readout is determinism-free. Walks are SHORT bounded stubs (never long home->station lines that
// would cross water on the OSM basemap). Capped at MAX_VISIBLE_PEEPS so cost is O(cap), not O(pop).

/// Hard cap on simultaneously-rendered peeps (the in-transit subset is sampled to this in sweep
/// order). Keeps per-frame work bounded regardless of network size.
pub const MAX_VISIBLE_PEEPS: usize = 4096;
/// Walk-in / walk-out animation window (ms of sim time).
pub const PEEP_WALK_MS: i64 = 6_000;
/// Cap on the walk-out breadcrumb ring buffer (board_alight prunes to this).
pub const MAX_RECENT_ALIGHT: usize = 8_192;

const STUB_M: f32 = 80.0; // bounded walk-stub length (metres) — short, never an ocean-crossing line
const FAN_M: f32 = 26.0; // platform fan-out radius (metres)
const RIDE_M: f32 = 15.0; // scatter radius around a moving train (metres)

/// Cheap stable integer hash (no RNG state) — used for per-peep jitter/direction so a dot doesn't
/// flicker frame-to-frame. Cosmetic only; need not be replay-reproducible.
#[inline]
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
fn unit_from_seed(seed: u32) -> (f32, f32) {
    let a = (hash_u32(seed) as f32 / u32::MAX as f32) * std::f32::consts::TAU;
    (a.cos(), a.sin())
}

/// A stable jittered offset (metres) within a disc of `radius`, deterministic from `seed`.
#[inline]
fn jitter(seed: u32, radius: f32) -> (f32, f32) {
    let (ux, uy) = unit_from_seed(seed);
    let r = (hash_u32(seed ^ 0x9e37_79b9) as f32 / u32::MAX as f32).sqrt() * radius;
    (ux * r, uy * r)
}

#[inline]
fn rgb_of(color: u32) -> (u8, u8, u8) {
    (((color >> 16) & 0xff) as u8, ((color >> 8) & 0xff) as u8, (color & 0xff) as u8)
}

/// Cell centre (metres) for a demand-grid cell index, if it exists.
#[inline]
fn cell_centre_m(w: &World, cell: u32) -> Option<(f32, f32)> {
    w.city.demand.cells.get(cell as usize).map(|c| (mm_to_m(c.x_mm), mm_to_m(c.y_mm)))
}

/// Unit direction pointing from a citizen's HOME (or WORK) cell toward a station — so walk-in peeps
/// approach from the home side and walk-out peeps head toward their destination. Falls back to a
/// stable per-seed pseudo-random direction (gravity mode has anonymous pax with no home/work).
#[inline]
fn walk_dir(w: &World, citizen: u32, cell_of: impl Fn(&crate::agents::Citizen) -> u32, to: (f32, f32), seed: u32) -> (f32, f32) {
    if citizen != u32::MAX {
        if let Some(pop) = &w.population {
            if let Some(c) = pop.citizens.get(citizen as usize) {
                if let Some((cx, cy)) = cell_centre_m(w, cell_of(c)) {
                    let (dx, dy) = (to.0 - cx, to.1 - cy);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 1.0 {
                        return (dx / len, dy / len);
                    }
                }
            }
        }
    }
    unit_from_seed(seed ^ 0x5151_5151)
}

/// Build the peep dot buffers at interpolation `alpha` (0=prev tick .. 1=cur), `tick_ms` = the
/// render tick length (so walk progress advances smoothly between ticks). Returns
/// `(positions_metres_interleaved, rgba_u8, citizen_ids)` — `citizen_ids[k]` is the citizen behind
/// peep `k` (or `u32::MAX` for an anonymous gravity trip), so the frontend can map a clicked peep
/// back to a rider and inspect/follow them. Pure read — touches no sim state, no hash.
pub fn fill_peeps(w: &World, alpha: f32, tick_ms: f32) -> (Vec<f32>, Vec<u8>, Vec<u32>) {
    let cap = MAX_VISIBLE_PEEPS;
    let mut xy: Vec<f32> = Vec::with_capacity(cap * 2);
    let mut col: Vec<u8> = Vec::with_capacity(cap * 4);
    let mut cit: Vec<u32> = Vec::with_capacity(cap);
    let now = w.clock_ms as f32 + alpha * tick_ms;

    // Inline push via a macro (not a closure — a closure would hold a mutable borrow of xy/col and
    // block the `n >= cap` capacity checks). `n` is the running peep count. The 7th arg is the
    // citizen id behind this peep (kept index-aligned with positions/colours for click-to-inspect).
    let mut n = 0usize;
    macro_rules! push {
        ($x:expr, $y:expr, $r:expr, $g:expr, $b:expr, $a:expr, $cit:expr) => {{
            xy.push($x);
            xy.push($y);
            col.push($r);
            col.push($g);
            col.push($b);
            col.push($a);
            cit.push($cit);
            n += 1;
        }};
    }

    // 1) RIDING — scatter onboard riders around their train's interpolated position (line colour).
    'riding: for i in 0..w.vehicles.len() {
        let px = w.vehicles.prev_x_mm[i] as f32;
        let cx = w.vehicles.x_mm[i] as f32;
        let py = w.vehicles.prev_y_mm[i] as f32;
        let cy = w.vehicles.y_mm[i] as f32;
        let vx = (px + (cx - px) * alpha) / 1000.0;
        let vy = (py + (cy - py) * alpha) / 1000.0;
        let (lr, lg, lb) = w.lines.get(w.vehicles.line[i].index()).map(|l| rgb_of(l.color)).unwrap_or((90, 90, 90));
        for pax in &w.vehicles.onboard_pax[i] {
            if n >= cap {
                break 'riding;
            }
            let seed = peep_seed(pax);
            let (jx, jy) = jitter(seed, RIDE_M);
            push!(vx + jx, vy + jy, lr, lg, lb, 235, pax.citizen_id);
        }
    }

    // 2) WAITING — fan riders out on the platform; freshly-spawned ones WALK IN along a short stub.
    'waiting: for s in 0..w.waiting.len() {
        if n >= cap {
            break;
        }
        let Some(st) = w.stations.get(s) else { continue };
        if st.removed {
            continue;
        }
        let sx = mm_to_m(st.pos.x_mm);
        let sy = mm_to_m(st.pos.y_mm);
        for pax in &w.waiting[s] {
            if n >= cap {
                break 'waiting;
            }
            let seed = peep_seed(pax);
            let prog = ((now - pax.t_spawn_ms as f32) / PEEP_WALK_MS as f32).clamp(0.0, 1.0);
            if prog < 1.0 {
                // Walking in: start STUB_M back on the home side, arrive at the platform.
                let (dx, dy) = walk_dir(w, pax.citizen_id, |c| c.home_cell, (sx, sy), seed);
                let back = STUB_M * (1.0 - prog);
                let (jx, jy) = jitter(seed, FAN_M * 0.5);
                push!(sx - dx * back + jx, sy - dy * back + jy, 92, 102, 116, (140.0 + 90.0 * prog) as u8, pax.citizen_id);
            } else {
                let (jx, jy) = jitter(seed, FAN_M);
                push!(sx + jx, sy + jy, 74, 84, 100, 220, pax.citizen_id);
            }
        }
    }

    // 3) WALK-OUT — recently-arrived riders amble out of the station toward their destination, fading.
    for r in &w.recent_alight {
        if n >= cap {
            break;
        }
        let prog = ((now - r.t_ms as f32) / PEEP_WALK_MS as f32).clamp(0.0, 1.0);
        if prog >= 1.0 {
            continue;
        }
        let Some(st) = w.stations.get(r.station as usize) else { continue };
        let sx = mm_to_m(st.pos.x_mm);
        let sy = mm_to_m(st.pos.y_mm);
        let (dx, dy) = walk_dir(w, r.citizen, |c| c.work_cell, (sx, sy), r.citizen ^ r.station);
        let fwd = STUB_M * prog;
        let (jx, jy) = jitter(r.citizen ^ (r.station << 8), FAN_M * 0.5);
        push!(sx + dx * fwd + jx, sy + dy * fwd + jy, 120, 130, 144, ((1.0 - prog) * 200.0) as u8, r.citizen);
    }

    (xy, col, cit)
}

/// Stable cosmetic seed per passenger: the citizen id when known, else a hash of the spawn time
/// (gravity-mode pax are all the same anonymous id, so the timestamp gives each a distinct seed).
#[inline]
fn peep_seed(pax: &crate::pax::Pax) -> u32 {
    if pax.citizen_id != u32::MAX {
        pax.citizen_id
    } else {
        (pax.t_spawn_ms as u64 as u32).wrapping_mul(2_654_435_761)
    }
}

/// Interleaved current positions `[x0,y0, x1,y1, ...]` in metres.
pub fn vehicle_positions_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out = Vec::with_capacity(v.len() * 2);
    for i in 0..v.len() {
        out.push(mm_to_m(v.x_mm[i]));
        out.push(mm_to_m(v.y_mm[i]));
    }
    out
}

/// Interleaved previous-tick positions `[x0,y0, ...]` in metres (for alpha interpolation).
pub fn vehicle_prev_positions_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out = Vec::with_capacity(v.len() * 2);
    for i in 0..v.len() {
        out.push(mm_to_m(v.prev_x_mm[i]));
        out.push(mm_to_m(v.prev_y_mm[i]));
    }
    out
}

pub fn vehicle_angles(w: &World) -> Vec<f32> {
    w.vehicles.angle.clone()
}

pub fn vehicle_line_ids(w: &World) -> Vec<u32> {
    w.vehicles.line.iter().map(|l| l.0).collect()
}

/// Interleaved `[onboard, capacity]` per vehicle (Uint16Array) — drives the train inspector's
/// load-factor readout. Capacity is the line's per-mode vehicle spec (single source: trainset.rs),
/// so the UI never re-derives it and can't drift.
pub fn vehicle_loads(w: &World) -> Vec<u16> {
    let v = &w.vehicles;
    let mut out = Vec::with_capacity(v.len() * 2);
    for i in 0..v.len() {
        let cap = w
            .lines
            .get(v.line[i].index())
            .map(|l| l.vehicle_spec().capacity)
            .unwrap_or(0);
        out.push(v.onboard[i]);
        out.push(cap);
    }
    out
}
