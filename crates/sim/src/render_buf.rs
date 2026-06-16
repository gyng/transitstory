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
pub const PEEP_WALK_MS: i64 = 1_900; // 80 m stub at the unified 42_000 mm/s walk speed (clock-honest)
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

/// Per-vehicle dominant CARGO commodity (#in-world-cargo), aligned 1:1 with `vehicle_positions_m`: the
/// commodity of its onboard cargo (a cart carries one source's output), or 255 if empty / a transit rider.
/// Lets the 3D cargo block be coloured by the GOODS it hauls (ore / grain / aether / fuel / processed).
/// Render-only (the Pax queues are excluded from Canonical).
pub fn vehicle_cargo_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out = Vec::with_capacity(v.len());
    for i in 0..v.len() {
        let kind = v.onboard_pax.get(i).and_then(|q| q.first()).map(|p| p.commodity).unwrap_or(255);
        out.push(kind as f32);
    }
    out
}

/// Trailing CARGO CARS pulled by each rail train (#multi-car), as a flat list ACROSS all vehicles —
/// 6 f32 per car: `[x_m, y_m, angle_rad, commodity, load, line_id]`. A rail/heavy train is drawn as its
/// locomotive body (the existing [`vehicle_positions_m`] entry) PLUS this string of cargo cars trailing
/// behind it along the SAME polyline, so the consist curves with the track instead of being one block.
/// Each car sits `k · car_len` back from the loco's arc-length on the train's `(line, path)` (clamped to
/// the path), inheriting the loco's commodity + load factor; `line_id` lets the chassis take the line
/// colour while the load lump takes the commodity colour. Cars are derived from capacity (a fatter train
/// pulls more), so they're a pure copy-out (no hashed state) — bus/ferry/air emit none (single body).
pub fn vehicle_cars_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out: Vec<f32> = Vec::new();
    for i in 0..v.len() {
        let Some(line) = w.lines.get(v.line[i].index()) else { continue };
        let cars = car_count(line.mode, line.vehicle_spec().capacity);
        if cars == 0 {
            continue;
        }
        let Some(path) = line.paths.get(v.path[i] as usize) else { continue };
        let len = path.length_mm();
        let dir = v.dir[i] as i64;
        let commodity = v.onboard_pax.get(i).and_then(|q| q.first()).map(|p| p.commodity).unwrap_or(255);
        let cap = line.vehicle_spec().capacity.max(1);
        let load = (v.onboard[i] as f32 / cap as f32).clamp(0.0, 1.0);
        let line_id = v.line[i].0 as f32;
        for k in 1..=cars {
            let back = CAR_PITCH_MM * k as i64;
            let s = (v.s_mm[i] - dir * back).clamp(0, len);
            let (cx, cy) = path.point_at(s);
            let mut ang = path.heading_at(s);
            if dir < 0 {
                ang += std::f32::consts::PI;
            }
            out.push(mm_to_m(cx));
            out.push(mm_to_m(cy));
            out.push(ang);
            out.push(commodity as f32);
            out.push(load);
            out.push(line_id);
        }
    }
    out
}

/// Previous-tick positions of the trailing cargo cars `[x0,y0, ...]` in metres, aligned 1:1 (per car)
/// with [`vehicle_cars_m`] — the alpha-interpolation companion (same `k · car_len` standoff applied to
/// the loco's PREVIOUS arc-length). Order/length match exactly so the frontend lerps each car cur↔prev.
pub fn vehicle_cars_prev_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out: Vec<f32> = Vec::new();
    for i in 0..v.len() {
        let Some(line) = w.lines.get(v.line[i].index()) else { continue };
        let cars = car_count(line.mode, line.vehicle_spec().capacity);
        if cars == 0 {
            continue;
        }
        let Some(path) = line.paths.get(v.path[i] as usize) else { continue };
        let len = path.length_mm();
        let dir = v.dir[i] as i64;
        for k in 1..=cars {
            let back = CAR_PITCH_MM * k as i64;
            let s = (v.prev_s_mm[i] - dir * back).clamp(0, len);
            let (px, py) = path.point_at(s);
            out.push(mm_to_m(px));
            out.push(mm_to_m(py));
        }
    }
    out
}

/// Centre-to-centre spacing (mm) between consecutive units of a consist (loco→car, car→car). This is a
/// RENDER pitch, deliberately the VISUAL car length — the frontend draws each unit at `VEHICLE_SCALE`
/// (≈150 m on the map, a "diorama" scale far larger than the real ~20 m wagon), so the cars must be
/// spaced by that visual length to sit nose-to-tail; spacing by the real spec consist length packs
/// 150 m meshes ~47 m apart and they overlap into one stubby blob. Kept just under the loco's visual
/// length so couplers read as a thin gap. Tied to render.ts VEHICLE_SCALE (keep them in step).
const CAR_PITCH_MM: i64 = 150_000;

/// Cargo-car COUNT a train pulls, derived from its capacity (#multi-car): only RAIL + HEAVY-rail trains
/// pull cars (bus/ferry/air are a single body → 0). A fatter train pulls more cars, clamped 2..=6 so
/// every train reads as a STRING (never a lone wagon) and a giant HSR consist never explodes the instance
/// count. Pure derivation (no hashed state): Standard(7)→3, Heavy(15)→5, Express(4)→2, HSR(18)→5.
#[inline]
fn car_count(mode: u8, capacity: u16) -> usize {
    use crate::trainset::tmode;
    match mode {
        tmode::RAIL | tmode::HEAVY => (((capacity as usize) + 5) / 4).clamp(2, 6),
        _ => 0,
    }
}

/// Interleaved marching-legion positions `[x0,y0, ...]` in metres (fantasy, S8 render). Each army owns
/// an arc-length `s_mm` on its route; we interpolate the route polyline (`Path::point_at`) to cartesian
/// here in the copy-out (float allowed). Besieging/done legions sit at the target. Empty for transit.
pub fn army_positions_m(w: &World) -> Vec<f32> {
    let a = &w.armies;
    let mut out = Vec::with_capacity(a.len() * 2);
    for i in 0..a.len() {
        let (x, y) = w
            .lines
            .get(a.line[i].index())
            .and_then(|l| l.paths.get(a.path[i] as usize))
            .map(|p| p.point_at(a.s_mm[i]))
            .unwrap_or((0, 0));
        out.push(mm_to_m(x));
        out.push(mm_to_m(y));
    }
    out
}

/// Interleaved legion TARGET positions `[x0,y0, ...]` in metres (fantasy, S11 render — the AI general's
/// intent). Same length/order as [`army_positions_m`]: a MARCHING legion emits its target town's centre
/// (so the UI can draw an intent arc from the legion to where it's headed); a besieging/idle legion emits
/// its OWN position (a zero-length arc → invisible). Render-only copy-out (float allowed). Empty for transit.
pub fn army_targets_m(w: &World) -> Vec<f32> {
    let a = &w.armies;
    let mut out = Vec::with_capacity(a.len() * 2);
    for i in 0..a.len() {
        let own = || {
            w.lines
                .get(a.line[i].index())
                .and_then(|l| l.paths.get(a.path[i] as usize))
                .map(|p| p.point_at(a.s_mm[i]))
                .unwrap_or((0, 0))
        };
        // Only a MARCHING legion has a forward intent to draw; otherwise collapse the arc to its own spot.
        let (x, y) = if a.state[i] == crate::army::MARCHING {
            w.stations
                .get(a.target[i] as usize)
                .map(|s| (s.pos.x_mm, s.pos.y_mm))
                .unwrap_or_else(own)
        } else {
            own()
        };
        out.push(mm_to_m(x));
        out.push(mm_to_m(y));
    }
    out
}

/// Interleaved RAIDER positions `[x0_m, y0_m, ...]` in metres (fantasy S11 — the rival). Raiders march
/// free 2-D (their position IS the authoritative hashed state), so this is a direct copy-out of the
/// MARCHING raiders. Empty for transit / a realm the rival hasn't reached.
pub fn raider_positions_m(w: &World) -> Vec<f32> {
    let r = &w.raiders;
    let mut out = Vec::with_capacity(r.live() * 2);
    for i in 0..r.len() {
        if r.state[i] != crate::raider::MARCHING {
            continue;
        }
        out.push(mm_to_m(r.x_mm[i]));
        out.push(mm_to_m(r.y_mm[i]));
    }
    out
}

/// Interleaved RAIDER TARGET positions `[tx0,ty0,...]` in metres (#war — the rival's intent), aligned 1:1
/// with `raider_positions_m` (same MARCHING filter + order). Each entry is where that raider is HEADING:
/// the capital (a breacher), a supply-line seam (a saboteur), or a captured town (a reclaimer) — so the UI
/// can draw the rival's intent and the player can see the smart enemy coming. Empty for transit.
pub fn raider_targets_m(w: &World) -> Vec<f32> {
    let r = &w.raiders;
    let mut out = Vec::with_capacity(r.live() * 2);
    for i in 0..r.len() {
        if r.state[i] != crate::raider::MARCHING {
            continue;
        }
        out.push(mm_to_m(r.tx_mm[i]));
        out.push(mm_to_m(r.ty_mm[i]));
    }
    out
}

/// RAIDER ROLE per marching raider (#war), aligned 1:1 with `raider_positions_m` — 0 BREACHER (capital),
/// 1 SABOTEUR (rail seam), 2 RECLAIMER (captured town). DERIVED from the raider's exact target position (a
/// render-only classification, so it needs no hashed role byte / re-pin): target == the capital ⇒ breacher
/// (incl. a re-aimed fallback, correctly now a breacher); target == a captured town's exact position ⇒
/// reclaimer; else ⇒ saboteur. Lets the UI badge the three roles apart (they're otherwise identical dots).
pub fn raider_roles_m(w: &World) -> Vec<f32> {
    let (cx, cy) = (w.city.capital_x_mm, w.city.capital_y_mm);
    let r = &w.raiders;
    let mut out = Vec::with_capacity(r.live());
    for i in 0..r.len() {
        if r.state[i] != crate::raider::MARCHING {
            continue;
        }
        let (tx, ty) = (r.tx_mm[i], r.ty_mm[i]);
        let role: u8 = if tx == cx && ty == cy {
            0 // breacher — aimed at the seat
        } else if (0..w.stations.len()).any(|s| {
            w.town_value.get(s).copied() == Some(0)
                && !w.stations[s].removed
                && w.stations[s].pos.x_mm == tx
                && w.stations[s].pos.y_mm == ty
        }) {
            2 // reclaimer — aimed at a captured town's exact position
        } else {
            1 // saboteur — aimed at a rail seam
        };
        out.push(role as f32);
    }
    out
}

/// LEGION STATE per legion (#war), aligned 1:1 with `army_positions_m` — 0 MARCHING, 1 BESIEGING, 2 DONE
/// (captured/garrisoned/inert). Lets the UI dim the permanent DONE garrisons (which otherwise render as
/// full-strength live dots that litter the map + inflate the apparent army strength). Render-only copy-out.
pub fn army_states_m(w: &World) -> Vec<f32> {
    w.armies.state.iter().map(|&s| s as f32).collect()
}

/// Interleaved SPELL FLASHES `[x0_m, y0_m, kind0, alpha0, ...]` (fantasy S11 — the spell arm). A brief
/// burst at each auto-cast site; `kind` (0 purge / 1 smite / 2 warpath) picks the colour, `alpha` (1→0
/// over the flash's life) the fade. Render-only (the flashes are not hashed). Empty otherwise.
pub fn spell_flashes_m(w: &World) -> Vec<f32> {
    let mut out = Vec::with_capacity(w.spell_flashes.len() * 4);
    for f in &w.spell_flashes {
        out.push(mm_to_m(f.x_mm));
        out.push(mm_to_m(f.y_mm));
        out.push(f.kind as f32);
        out.push((1.0 - f.age_ms as f32 / 1500.0).clamp(0.0, 1.0)); // 1500 = spell::FLASH_MS
    }
    out
}

/// Interleaved TTD signal markers `[x0_m, y0_m, status0, ...]` (3 f32 each): each single-track block's
/// render state — `status` 1 = OCCUPIED (a cart is in the block → red), 2 = WAITING (a cart is held at
/// this gate for the block ahead → amber). Self-positioned via `point_at(mid_mm)` on its `(line, path)`
/// (float allowed in this copy-out). Render-only scratch (NOT hashed); empty for transit / double-track
/// networks (no single-span meets) and when nothing is running.
pub fn signal_markers_m(w: &World) -> Vec<f32> {
    let mut out = Vec::with_capacity(w.signal_occupancy.len() * 3);
    for s in &w.signal_occupancy {
        let (x, y) = w
            .lines
            .get(s.line as usize)
            .and_then(|l| l.paths.get(s.path as usize))
            .map(|p| p.point_at(s.mid_mm))
            .unwrap_or((0, 0));
        out.push(mm_to_m(x));
        out.push(mm_to_m(y));
        out.push(s.status as f32);
    }
    out
}

/// Interleaved decadence-tide cells `[x0_m, y0_m, v0, ...]` (fantasy S10c): each CORRUPTED CA cell
/// (decadence > 0) as local metres + a 0..1 strength (`decadence / DECAD_MAX`) for the cold-tide overlay.
/// Empty for transit / before the tide starts. Render-only — the field is hashed; this is a copy-out
/// (the strength feeds alpha, never state). Bounded by the (capped) CA domain.
pub fn decadence_tide_m(w: &World) -> Vec<f32> {
    let f = &w.decadence_field;
    let size = w.city.grid_cell_mm;
    if size <= 0 || f.is_empty() {
        return Vec::new();
    }
    let n = w.decadence_cells.len().min(f.cells.len());
    let mut out = Vec::new();
    for c in 0..n {
        let v = w.decadence_cells[c];
        if v <= 0 {
            continue;
        }
        let p = crate::hexgrid::center_of(f.cells[c], size);
        out.push(mm_to_m(p.x_mm));
        out.push(mm_to_m(p.y_mm));
        out.push(v as f32 / crate::decadence_field::DECAD_MAX as f32);
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
