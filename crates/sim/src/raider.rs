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
const DEFENSE_RANGE_MM: i64 = 4_000_000;
/// Within this range (mm) of the capital counts as ARRIVED (deepen the rot + despawn).
const ARRIVE_MM: i64 = 2_000_000;
/// Base spawn period (ms). The cadence SHORTENS as decadence rises (decadence-fed spawning — "their
/// economy IS the decadence"): `period = BASE / (1 + decadence/SCALE)`, floored. So raiders are rare early
/// (the conquest window is unthreatened) and swarm late (the pressure the player must out-run). Tunable.
const SPAWN_BASE_MS: i64 = 90_000;
const SPAWN_DECADENCE_SCALE: i64 = 4_000;
const SPAWN_MIN_MS: i64 = 12_000;

/// Separate Structure-of-Arrays for raiders. Authoritative (hashed) FREE 2-D position (they march off-rail,
/// so the position IS the state — unlike legions, whose `s_mm` is the authority and x/y are render-only).
#[derive(Clone, Default)]
pub struct RaiderSoA {
    pub x_mm: Vec<i64>,
    pub y_mm: Vec<i64>,
    pub state: Vec<u8>,
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
    /// Spawn a raider at `(x, y)`, RECYCLING the lowest-index DONE slot if one exists, else pushing a new
    /// slot (only up to `MAX_RAIDERS`). Recycling is what bounds the SoA system-wide. Returns false if the
    /// cap is hit (no DONE slot + already full ⇒ the spawn is skipped).
    fn spawn_at(&mut self, x: i64, y: i64) -> bool {
        if let Some(i) = self.state.iter().position(|&s| s == DONE) {
            self.x_mm[i] = x;
            self.y_mm[i] = y;
            self.state[i] = MARCHING;
            true
        } else if self.len() < MAX_RAIDERS {
            self.x_mm.push(x);
            self.y_mm.push(y);
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
    world.raider_cursor = world.raider_cursor.wrapping_add(1);
    let axial = world.decadence_field.cells[cell as usize];
    let p = crate::hexgrid::center_of(axial, size);
    world.raiders.spawn_at(p.x_mm, p.y_mm); // skipped silently if the cap is hit (bounded)
}

/// MARCH: advance each MARCHING raider STRAIGHT at the capital by `step` mm (integer-exact via `isqrt`).
/// Distance-to-capital is monotone non-increasing — the no-livelock guarantee.
fn march(world: &mut World, dt_ms: i64) {
    let (cx, cy) = (world.city.capital_x_mm, world.city.capital_y_mm);
    let step = RAIDER_SPEED_MM_S.saturating_mul(dt_ms.max(0)) / 1000;
    for i in 0..world.raiders.len() {
        if world.raiders.state[i] != MARCHING {
            continue;
        }
        let (x, y) = (world.raiders.x_mm[i], world.raiders.y_mm[i]);
        let (dx, dy) = (cx - x, cy - y);
        let dist = (dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))).isqrt();
        if dist <= step || dist == 0 {
            world.raiders.x_mm[i] = cx; // close enough — snap to the capital (resolve handles arrival)
            world.raiders.y_mm[i] = cy;
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
    let def2 = DEFENSE_RANGE_MM.saturating_mul(DEFENSE_RANGE_MM);
    let arr2 = ARRIVE_MM.saturating_mul(ARRIVE_MM);
    for i in 0..world.raiders.len() {
        if world.raiders.state[i] != MARCHING {
            continue;
        }
        let (x, y) = (world.raiders.x_mm[i], world.raiders.y_mm[i]);
        if intercepted(world, x, y, def2) {
            world.raiders.state[i] = DONE; // the rail network cuts it down
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
        }
    }
}

/// True iff a station ON A BUILT LINE sits within `def2` (squared mm) of `(x, y)` — the rail network's
/// defensive reach. Mirrors the PURGE rule (only railed stations count, not unconnected baked nodes).
fn intercepted(world: &World, x: i64, y: i64, def2: i64) -> bool {
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
    false
}
