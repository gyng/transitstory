//! S11 — the RIVAL: decadence RAIDERS (the design's gate-safe Tier-1-LITE enemy faction). The decadence
//! isn't just a passive tide: it FIELDS marauders. Raiders spawn from the far-edge RESERVOIR (the tide
//! origin), march straight at the CAPITAL, and — if they reach it — DEEPEN the rot (a direct shove on the
//! lose meter). The player's RAIL NETWORK is the defence: a raider that passes within range of a station
//! on a built line is cut down (coverage = defence, reinforcing the core loop — no RTS micro, Majesty-style).
//!
//! **Gate-safe by construction** (the design flags the rival as the biggest gate-blind risk — a livelocking
//! rival passes `run==run`):
//! - **No livelock:** a raider marches STRAIGHT at the fixed capital, so its distance-to-capital is
//!   monotone non-increasing every tick until it resolves. It can't oscillate or stall.
//! - **No sawtooth / bounded system-wide:** a hard `MAX_RAIDERS` slot cap with DONE-slot RECYCLING — the
//!   SoA never grows past the cap, and spawning continues at steady state (no unbounded accumulation).
//! - **Deterministic:** integer-exact (`i64::isqrt` for the 2-D step), index-ordered, NO rng — the spawn
//!   cadence is a fixed accumulator and the reservoir is cycled by a counter. Replays bit-for-bit.
//! - **Golden-neutral:** no reservoir (transit / demo arcadia have no decadence field) ⇒ no spawns ⇒ the
//!   `RaiderSoA` stays empty ⇒ the appended hashed slices re-pin once then byte-identical.
use crate::world::World;

/// Raider state. DONE slots are RECYCLED by the next spawn (bounding the SoA system-wide).
pub const MARCHING: u8 = 0;
pub const DONE: u8 = 1; // intercepted or arrived — inert, its slot reusable

/// Hard cap on raider SLOTS (the SoA never exceeds this; DONE slots recycle ⇒ steady-state, no sawtooth).
const MAX_RAIDERS: usize = 64;
/// Raider march pace (mm/s) — slower than a legion (the baked 200 km/s‑scale army) so a built network has
/// time to cut them down. A creeping marauder, not a blitz. Tunable.
const RAIDER_SPEED_MM_S: i64 = 90_000;
/// Decadence the realm suffers when a raider reaches the capital — added to `raider_breach`, which the
/// field step folds into the lose meter. The threat is the SWARM once decadence is high (the cadence
/// shortens), not one raider. Tunable.
const RAIDER_DAMAGE: i64 = 300;
/// Breach HEAL rate (per sim-second): `raider_breach` decays toward 0 between raids — the realm rebuilds.
/// This is the RECOVERY LEVER (adversarial-review fix): a network that cuts the raiders down (no fresh
/// breaches) heals back to 0, so the rot is the CURRENT raid pressure vs the heal, never an irreversible
/// point-of-no-return. Under sustained UNDEFENDED assault arrivals outpace the heal ⇒ breach still mounts
/// ⇒ the realm falls (the threat keeps its teeth). Tunable; calibrated between the defended/undefended
/// arrival rates. (A sub-unit accumulator carries the remainder so a slow heal isn't truncated to 0.)
const BREACH_HEAL_PER_S: i64 = 6;
/// A raider within this range (mm) of a station ON A BUILT LINE is cut down (coverage = defence).
pub(crate) const DEFENSE_RANGE_MM: i64 = 4_000_000;
/// Rail-attack (#war): a raider that slips the station cordon and reaches an OPERATIONAL line's TRACK
/// within this range (mm of the stop-to-stop polyline) CUTS it — freezing its trains for `RAIL_DISABLE_MS`
/// — and spends itself (despawn). Strictly SMALLER than `DEFENSE_RANGE_MM`, so a defended station always
/// cuts the raider down before it can reach the adjacent track; only LONG sparse spans (mid-span > def
/// range from either endpoint) expose a seam a raider can sever. Denser networks defend; this is the front.
const RAIL_ATTACK_RANGE_MM: i64 = 2_000_000;
/// How long a cut line stays RAIDED (sim-ms) — its trains freeze in place, no delivery, until it lapses.
/// Longer than the base spawn period so a raid is a real disruption you must out-build, not a blink; the
/// line auto-recovers (no permanent loss — the log is append-only, the timer just gates dispatch/advance).
const RAIL_DISABLE_MS: i64 = 120_000;
/// Within this range (mm) of the capital counts as ARRIVED (deepen the rot + despawn).
const ARRIVE_MM: i64 = 2_000_000;
/// Base spawn period (ms). The cadence SHORTENS as decadence rises (decadence-fed spawning — "their
/// economy IS the decadence"): `period = BASE / (1 + decadence/SCALE)`, floored. So raiders are rare early
/// (the conquest window is unthreatened) and swarm late (the pressure the player must out-run). Tunable.
const SPAWN_BASE_MS: i64 = 90_000;
const SPAWN_DECADENCE_SCALE: i64 = 4_000;
const SPAWN_MIN_MS: i64 = 12_000;
/// Front targeting (#war): the rival cycles THREE roles by spawn-cursor parity (deterministic, no rng):
///   0 → BREACHER  — march the capital, deepen the rot (the lose pressure; winnability anchor).
///   1 → SABOTEUR  — march a supply line's SEAM and cut it (the felt rail-attack + a supply front).
///   2 → RECLAIMER — march a CAPTURED town and RE-GARRISON it (you must HOLD conquered ground — the
///                   territory front oscillates: you take, the rival re-contests, you re-take).
/// A role with no objective (no rail / no captured town) falls back to the capital, so no raider stalls.
const ROLE_CYCLE: u32 = 3;
/// Reclaim re-garrison (#war): a reclaimer flips a captured town back to this much siege resistance — a
/// LIGHT re-contest (the base garrison, no depth bonus), so re-taking is a quick re-siege, not a fresh
/// conquest. `towns_captured` is NOT touched (cumulative ⇒ the monotonic Standing gauge is untouched).
const RECLAIM_GARRISON: i64 = 500;

/// Separate Structure-of-Arrays for raiders. Authoritative (hashed) FREE 2-D position (they march off-rail,
/// so the position IS the state — unlike legions, whose `s_mm` is the authority and x/y are render-only).
#[derive(Clone, Default)]
pub struct RaiderSoA {
    pub x_mm: Vec<i64>,
    pub y_mm: Vec<i64>,
    pub state: Vec<u8>,
    /// March TARGET (mm) — most raiders aim at the capital (the breach threat); SABOTEURS (#war) aim at a
    /// player supply line's midpoint to CUT it (rail-attack, made FELT). Authoritative (hashed) — the march
    /// pace + the no-livelock guarantee are measured against it. Default = capital, set at spawn.
    pub tx_mm: Vec<i64>,
    pub ty_mm: Vec<i64>,
}

impl RaiderSoA {
    pub fn len(&self) -> usize {
        self.x_mm.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x_mm.is_empty()
    }
    /// Live (MARCHING) raiders — the bounded-population figure (DONE slots don't count).
    pub fn live(&self) -> usize {
        self.state.iter().filter(|&&s| s == MARCHING).count()
    }
    /// Spawn a raider at `(x, y)` marching at TARGET `(tx, ty)`, RECYCLING the lowest-index DONE slot if one
    /// exists, else pushing a new slot (only up to `MAX_RAIDERS`). Recycling is what bounds the SoA
    /// system-wide. Returns false if the cap is hit (no DONE slot + already full ⇒ the spawn is skipped).
    fn spawn_at(&mut self, x: i64, y: i64, tx: i64, ty: i64) -> bool {
        if let Some(i) = self.state.iter().position(|&s| s == DONE) {
            self.x_mm[i] = x;
            self.y_mm[i] = y;
            self.tx_mm[i] = tx;
            self.ty_mm[i] = ty;
            self.state[i] = MARCHING;
            true
        } else if self.len() < MAX_RAIDERS {
            self.x_mm.push(x);
            self.y_mm.push(y);
            self.tx_mm.push(tx);
            self.ty_mm.push(ty);
            self.state.push(MARCHING);
            true
        } else {
            false
        }
    }
}

/// The rival's tick (run AFTER the player's army phases in `war_step`): spawn → march → resolve. Inert
/// (no reservoir) for transit + demo arcadia, so those stay byte-identical. Integer + index-ordered ⇒
/// deterministic.
pub(crate) fn step(world: &mut World, dt_ms: i64) {
    spawn(world, dt_ms);
    march(world, dt_ms);
    resolve(world);
    heal_breach(world, dt_ms);
}

/// HEAL: `raider_breach` decays toward 0 (the realm rebuilds between raids) — the recovery lever, so a
/// network that holds the approach is never permanently doomed by past raids. A sub-unit accumulator
/// carries the remainder (a slow per-tick rate isn't truncated to 0). Integer + deterministic.
fn heal_breach(world: &mut World, dt_ms: i64) {
    if world.raider_breach <= 0 {
        world.raider_breach_heal_accum = 0; // nothing to heal ⇒ keep the accumulator clean/deterministic
        return;
    }
    world.raider_breach_heal_accum = world.raider_breach_heal_accum.saturating_add(BREACH_HEAL_PER_S.saturating_mul(dt_ms.max(0)));
    let heal = world.raider_breach_heal_accum / 1000;
    if heal > 0 {
        world.raider_breach_heal_accum -= heal * 1000;
        world.raider_breach = (world.raider_breach - heal).max(0);
    }
}

/// SPAWN: accrue the (decadence-fed) cadence; when due, field one raider at the next reservoir cell. No
/// reservoir ⇒ no-op (golden-neutral). Deterministic: a fixed accumulator + a cursor cycling the reservoir.
fn spawn(world: &mut World, dt_ms: i64) {
    let reservoir_len = world.decadence_field.reservoir.len();
    if reservoir_len == 0 {
        return; // no decadence field/reservoir ⇒ the rival never fields (transit/demo) ⇒ golden-neutral
    }
    world.raider_spawn_accum_ms = world.raider_spawn_accum_ms.saturating_add(dt_ms.max(0));
    // Decadence-fed cadence: the rot fuels the raids — more decadence ⇒ shorter period ⇒ a rising swarm.
    let period = (SPAWN_BASE_MS / (1 + world.decadence.max(0) / SPAWN_DECADENCE_SCALE)).max(SPAWN_MIN_MS);
    if world.raider_spawn_accum_ms < period {
        return;
    }
    world.raider_spawn_accum_ms = 0;
    let size = world.city.grid_cell_mm.max(1);
    let cell = world.decadence_field.reservoir[(world.raider_cursor as usize) % reservoir_len];
    let cursor = world.raider_cursor;
    world.raider_cursor = world.raider_cursor.wrapping_add(1);
    let axial = world.decadence_field.cells[cell as usize];
    let p = crate::hexgrid::center_of(axial, size);
    // Targeting (#war): cycle the three roles by cursor parity (deterministic). Saboteurs aim at the nearest
    // supply-line SEAM; reclaimers at the nearest CAPTURED town; breachers (and any role with no objective)
    // at the capital — so a raider always has a fixed target and never stalls.
    let (cx, cy) = (world.city.capital_x_mm, world.city.capital_y_mm);
    let target = match cursor % ROLE_CYCLE {
        1 => nearest_seam(world, p.x_mm, p.y_mm).unwrap_or((cx, cy)),
        2 => nearest_captured_town(world, p.x_mm, p.y_mm).map(|(_, tx, ty)| (tx, ty)).unwrap_or((cx, cy)),
        _ => (cx, cy),
    };
    world.raiders.spawn_at(p.x_mm, p.y_mm, target.0, target.1); // skipped silently if the cap is hit (bounded)
}

/// Rail-attack targeting (#war): the most vulnerable SEAM nearest `(x, y)` — the midpoint of the longest
/// span (consecutive stop pair) of the nearest OPERATIONAL line. A saboteur aims here; a span long enough
/// puts the seam beyond the station cordon (cuttable), a short one keeps it defended (the saboteur dies at
/// the line, like a breacher at the capital). Index-ordered, lowest-index tiebreak ⇒ deterministic. `None`
/// if there is no operational line to seek.
fn nearest_seam(world: &World, x: i64, y: i64) -> Option<(i64, i64)> {
    let mut best: Option<(i128, i64, i64)> = None; // (dist² to seam, seam x, seam y)
    for line in world.lines.iter() {
        if line.removed || line.stops.len() < 2 {
            continue;
        }
        // The line's LONGEST span = its most exposed seam.
        let mut seam: Option<(i128, i64, i64)> = None; // (span len², midx, midy)
        for w in line.stops.windows(2) {
            let (Some(a), Some(b)) = (world.stations.get(w[0].index()), world.stations.get(w[1].index()))
            else {
                continue;
            };
            if a.removed || b.removed {
                continue;
            }
            let (dx, dy) = ((b.pos.x_mm - a.pos.x_mm) as i128, (b.pos.y_mm - a.pos.y_mm) as i128);
            let len2 = dx * dx + dy * dy;
            if seam.map_or(true, |(l, _, _)| len2 > l) {
                seam = Some((len2, (a.pos.x_mm + b.pos.x_mm) / 2, (a.pos.y_mm + b.pos.y_mm) / 2));
            }
        }
        if let Some((_, sx, sy)) = seam {
            let (ddx, ddy) = ((sx - x) as i128, (sy - y) as i128);
            let d2 = ddx * ddx + ddy * ddy;
            if best.map_or(true, |(bd, _, _)| d2 < bd) {
                best = Some((d2, sx, sy));
            }
        }
    }
    best.map(|(_, sx, sy)| (sx, sy))
}

/// Front targeting (#war): the nearest CAPTURED town (`town_value == 0`, the conquest-flip signal) to
/// `(x, y)`, as `(station index, x, y)`. A reclaimer aims here to re-garrison it. Index-ordered, lowest-
/// index tiebreak ⇒ deterministic. `None` if the realm holds no captured ground yet (early game).
pub(crate) fn nearest_captured_town(world: &World, x: i64, y: i64) -> Option<(usize, i64, i64)> {
    let mut best: Option<(i128, usize, i64, i64)> = None;
    for s in 0..world.stations.len() {
        if world.town_value.get(s).copied() != Some(0) || world.stations[s].removed {
            continue;
        }
        let (px, py) = (world.stations[s].pos.x_mm, world.stations[s].pos.y_mm);
        let (ddx, ddy) = ((px - x) as i128, (py - y) as i128);
        let d2 = ddx * ddx + ddy * ddy;
        if best.map_or(true, |(bd, _, _, _)| d2 < bd) {
            best = Some((d2, s, px, py));
        }
    }
    best.map(|(_, s, px, py)| (s, px, py))
}

/// MARCH: advance each MARCHING raider STRAIGHT at its TARGET (#war — the capital for a breacher, a supply
/// line's seam for a saboteur) by `step` mm (integer-exact via `isqrt`). Distance-to-TARGET is monotone
/// non-increasing within an episode — the no-livelock guarantee (a saboteur re-aims at the capital at most
/// once in `resolve`, so it converges).
fn march(world: &mut World, dt_ms: i64) {
    let step = RAIDER_SPEED_MM_S.saturating_mul(dt_ms.max(0)) / 1000;
    let gcm = world.city.grid_cell_mm;
    for i in 0..world.raiders.len() {
        if world.raiders.state[i] != MARCHING {
            continue;
        }
        let (x, y) = (world.raiders.x_mm[i], world.raiders.y_mm[i]);
        let (tx, ty) = (world.raiders.tx_mm[i], world.raiders.ty_mm[i]);
        // #cost-routing: head for the NEXT cell on the terrain-cost hex path to the target — `line_costed`
        // swings AROUND water/mountains (the SAME `World::terrain_cost` router the rails use) instead of
        // cutting straight through everything (and off the continent). Re-routed each cell, so it tracks a
        // moving seam target too. Off-grid (no lattice) ⇒ straight at the target (no raiders there anyway).
        let (wx, wy) = if gcm > 0 {
            let from = crate::hexgrid::axial_of(crate::geo_local::PointMm::new(x, y), gcm);
            let to = crate::hexgrid::axial_of(crate::geo_local::PointMm::new(tx, ty), gcm);
            if from == to {
                (tx, ty) // already in the target's cell ⇒ home straight in
            } else {
                let cost = |c: crate::hexgrid::Axial| world.terrain_cost(c);
                let path = crate::hexgrid::line_costed(from, to, &cost);
                let next = path.get(1).copied().unwrap_or(to);
                let c = crate::hexgrid::center_of(next, gcm);
                (c.x_mm, c.y_mm)
            }
        } else {
            (tx, ty)
        };
        // Snap to the FINAL target when within a step of it (resolve handles arrival), else step toward the
        // routed waypoint; reaching the waypoint cell re-routes from there next tick.
        let (dxt, dyt) = (tx - x, ty - y);
        let dist_t = (dxt.saturating_mul(dxt).saturating_add(dyt.saturating_mul(dyt))).isqrt();
        if dist_t <= step || dist_t == 0 {
            world.raiders.x_mm[i] = tx;
            world.raiders.y_mm[i] = ty;
            continue;
        }
        let (dx, dy) = (wx - x, wy - y);
        let dist = (dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))).isqrt();
        if dist <= step || dist == 0 {
            world.raiders.x_mm[i] = wx;
            world.raiders.y_mm[i] = wy;
        } else {
            world.raiders.x_mm[i] = x + dx.saturating_mul(step) / dist;
            world.raiders.y_mm[i] = y + dy.saturating_mul(step) / dist;
        }
    }
}

/// RESOLVE: a MARCHING raider is CUT DOWN if it's within `DEFENSE_RANGE_MM` of a station on a built line
/// (the network defends); else if it has reached the capital, it deepens the rot then despawns. Index-
/// ordered ⇒ deterministic. The DONE slot is recyclable by the next spawn (bounded SoA).
fn resolve(world: &mut World) {
    let (cx, cy) = (world.city.capital_x_mm, world.city.capital_y_mm);
    // S11 WARD_LINES: arcane wards extend the rail cordon's reach +50% (range ×3/2). 0 ⇒ ×1, byte-identical.
    let def_range = if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::WARD_LINES) {
        DEFENSE_RANGE_MM * 3 / 2
    } else {
        DEFENSE_RANGE_MM
    };
    let def2 = def_range.saturating_mul(def_range);
    let arr2 = ARRIVE_MM.saturating_mul(ARRIVE_MM);
    let atk2 = (RAIL_ATTACK_RANGE_MM as i128) * (RAIL_ATTACK_RANGE_MM as i128); // #war: rail-attack reach²
    for i in 0..world.raiders.len() {
        if world.raiders.state[i] != MARCHING {
            continue;
        }
        let (x, y) = (world.raiders.x_mm[i], world.raiders.y_mm[i]);
        if intercepted(world, x, y, def2) {
            world.raiders.state[i] = DONE; // the rail network cuts it down
            crate::spell::fx_burst(world, crate::spell::FX_KILL, x, y); // #war: echo the cordon's kill (was silent)
            continue;
        }
        // Rail-attack (#war): the cordon missed it, but it has reached an OPERATIONAL line's TRACK — CUT
        // that line (its trains freeze for RAIL_DISABLE_MS) and spend the raider in the raid. The vulnerable
        // seam is a long span far from any defending station; a dense network leaves no gap to sever.
        if let Some(li) = nearest_rail(world, x, y, atk2) {
            world.disable_line(li, world.clock_ms.saturating_add(RAIL_DISABLE_MS));
            world.raiders.state[i] = DONE;
            continue;
        }
        let (dx, dy) = (cx - x, cy - y);
        if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) <= arr2 {
            // The rot deepens. `world.decadence` is RE-DERIVED from the tide front each tick
            // (decadence_field::step OVERWRITES it), so a raider can't shove that scalar directly; it adds
            // to `raider_breach`, which the field step ADDS on top of the front-derived meter. Capped at the
            // capital threshold (so a long assault can't blow it up + then take forever to heal); decays via
            // `heal_breach` (the realm recovers when the network holds — no point-of-no-return).
            world.raider_breach =
                world.raider_breach.saturating_add(RAIDER_DAMAGE).min(crate::decadence::CAPITAL_THRESHOLD);
            world.raiders.state[i] = DONE;
            crate::spell::fx_burst(world, crate::spell::FX_BREACH, cx, cy); // #war: echo the capital strike (was silent)
            continue;
        }
        // A raider reached its NON-capital target (a saboteur's seam or a reclaimer's town):
        let (tx, ty) = (world.raiders.tx_mm[i], world.raiders.ty_mm[i]);
        if (tx, ty) != (cx, cy) {
            let (sdx, sdy) = (tx - x, ty - y);
            if sdx.saturating_mul(sdx).saturating_add(sdy.saturating_mul(sdy)) <= arr2 {
                // RECLAIMER (#war): an UNDEFENDED captured town here gets RE-GARRISONED — the territory front
                // oscillates (you must hold conquered ground). It slipped the cordon (`intercepted` above), so
                // only OFF-network / un-warded holdings are re-contestable; railed ones stay safe. `town_value`
                // back up, `towns_captured` UNTOUCHED (cumulative ⇒ the monotonic Standing gauge is safe).
                if let Some((t, _, _)) = nearest_captured_town(world, x, y) {
                    let (cdx, cdy) = (world.stations[t].pos.x_mm - x, world.stations[t].pos.y_mm - y);
                    if cdx.saturating_mul(cdx).saturating_add(cdy.saturating_mul(cdy)) <= arr2 {
                        if let Some(v) = world.town_value.get_mut(t) {
                            *v = RECLAIM_GARRISON;
                        }
                        world.raiders.state[i] = DONE;
                        continue;
                    }
                }
                // Nothing to reclaim/cut here (seam already cut, town defended/gone) — RE-AIM at the capital
                // so it converges (the no-livelock fallback; a one-way, at-most-once switch).
                world.raiders.tx_mm[i] = cx;
                world.raiders.ty_mm[i] = cy;
            }
        }
    }
}

/// True iff a station ON A BUILT LINE sits within `def2` (squared mm) of `(x, y)` — the rail network's
/// defensive reach. Mirrors the PURGE rule (only railed stations count, not unconnected baked nodes).
pub(crate) fn intercepted(world: &World, x: i64, y: i64, def2: i64) -> bool {
    for line in &world.lines {
        if line.removed {
            continue;
        }
        for stop in &line.stops {
            let Some(s) = world.stations.get(stop.index()) else { continue };
            if s.removed {
                continue;
            }
            let (dx, dy) = (s.pos.x_mm - x, s.pos.y_mm - y);
            if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) <= def2 {
                return true;
            }
        }
    }
    // S11 STANDING_GARRISON: CAPTURED towns (town_value ground to 0) also cut raiders down — conquest
    // extends the cordon to the frontier you've taken, even off-rail. Gated on the tech; 0 ⇒ skipped.
    if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::STANDING_GARRISON) {
        for s in 0..world.stations.len() {
            if world.town_value.get(s).copied() == Some(0) && !world.stations[s].removed {
                let (dx, dy) = (world.stations[s].pos.x_mm - x, world.stations[s].pos.y_mm - y);
                if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) <= def2 {
                    return true;
                }
            }
        }
    }
    false
}

/// Rail-attack (#war): the nearest OPERATIONAL line whose TRACK (its stop-to-stop trunk polyline) passes
/// within `range2` (mm²) of `(x, y)`, by line index. Index-ordered, lowest-index tiebreak ⇒ deterministic.
/// Skips removed / sub-2-stop / already-RAIDED lines (a raider can't re-cut a frozen line). The trunk
/// polyline uses stop positions (waypoint bends are ignored — a forgiving straight-segment approximation).
fn nearest_rail(world: &World, x: i64, y: i64, range2: i128) -> Option<usize> {
    let mut best: Option<(i128, usize)> = None;
    for (li, line) in world.lines.iter().enumerate() {
        if line.removed || line.stops.len() < 2 || world.line_disabled(li) {
            continue;
        }
        for w in line.stops.windows(2) {
            let (Some(a), Some(b)) = (world.stations.get(w[0].index()), world.stations.get(w[1].index()))
            else {
                continue;
            };
            if a.removed || b.removed {
                continue;
            }
            let d2 = seg_dist2(x, y, a.pos.x_mm, a.pos.y_mm, b.pos.x_mm, b.pos.y_mm);
            if d2 <= range2 && best.map_or(true, |(bd, _)| d2 < bd) {
                best = Some((d2, li));
            }
        }
    }
    best.map(|(_, li)| li)
}

/// Squared distance (i128 mm²) from point P to segment AB — integer-exact (clamped i128 projection, no
/// float), so the rail-attack proximity test is determinism-safe. Degenerate AB (a == b) ⇒ |PA|².
fn seg_dist2(px: i64, py: i64, ax: i64, ay: i64, bx: i64, by: i64) -> i128 {
    let (px, py) = (px as i128, py as i128);
    let (ax, ay) = (ax as i128, ay as i128);
    let (bx, by) = (bx as i128, by as i128);
    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);
    let ab2 = abx * abx + aby * aby;
    if ab2 == 0 {
        return apx * apx + apy * apy;
    }
    let t = (apx * abx + apy * aby).clamp(0, ab2); // projection param × ab2, clamped to the segment
    let (cx, cy) = (ax + abx * t / ab2, ay + aby * t / ab2);
    let (dx, dy) = (px - cx, py - cy);
    dx * dx + dy * dy
}
