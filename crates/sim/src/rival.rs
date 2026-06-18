//! #13 RIVAL HOSTS — the symmetric enemy AI's mustered legions (P1d). The rival hold (the faction-1
//! barracks seeded by `Command::SeedRival`) musters HOSTS funded by `rival_manpower` that march OVERLAND
//! at the player's CAPTURED towns and RE-GARRISON them — the territory-oscillation pressure (you take, the
//! rival re-contests, you re-take). DISTINCT from the rot's raiders: the rival's own, crimson + telegraphed,
//! sourced from its hold (not the decadence reservoir) and funded by its OWN treasury, not the decadence
//! clock. Rail-riding + dynamic expansion arrive with the AI builder (P2); for now the host marches overland.
//!
//! FAIR by construction (the antidote to "too aggro"): a host only re-garrisons an UNDEFENDED captured town —
//! a town held inside the player's rail cordon is safe (mirrors the reclaimer raider's rule), so the player
//! can DEFEND conquered ground by railing it, and the rival re-contests only the exposed marches.
//!
//! Determinism: integer arc/dist, index-ordered iteration, NO rng (a fixed muster cadence + the lowest-index
//! nearest captured town). The FREE 2-D position is authoritative (off-rail ⇒ the position IS the state,
//! like raiders) ⇒ hashed in `Canonical`. Golden-neutral: no rival hold (transit + the goldens + the
//! rival-less balance scenarios) ⇒ no muster ⇒ the SoA stays empty (an appended-empty re-pin, then
//! byte-identical).
use crate::world::World;

pub const MARCHING: u8 = 0;
pub const DONE: u8 = 1; // arrived (re-garrisoned, or cut down at a defended town) — slot recyclable

/// Hard cap on rival-host SLOTS (DONE slots recycle ⇒ bounded, no sawtooth). Small: a focused threat, not a
/// swarm (the rot's raiders are the swarm; the rival is a deliberate adversary).
const MAX_HOSTS: usize = 8;
/// Host march pace (mm/s) — a touch faster than the rot's raiders (a purposeful army, not creeping marauders).
const HOST_SPEED_MM_S: i64 = 110_000;
/// `rival_manpower` to muster one host (mirrors the player's `army::LAUNCH_COST` — the rival pays the same
/// price the player does: symmetry). Saved up between musters when the treasury is short.
const MUSTER_COST: i64 = 8;
/// Muster cadence (ms): a host every ~90 sim-s while the rival can afford one AND there is captured ground to
/// re-contest. Authoritative (gates the next muster) ⇒ hashed.
const MUSTER_PERIOD_MS: i64 = 90_000;
/// Within this range (mm) of the target town counts as ARRIVED (re-garrison + despawn).
const ARRIVE_MM: i64 = 2_000_000;
/// Re-garrison HP a host restores to a re-contested captured town — a LIGHT re-contest (mirrors the
/// reclaimer's `RECLAIM_GARRISON`), so re-taking is a quick re-siege, not a fresh conquest. `towns_captured`
/// is NOT touched (cumulative ⇒ the monotonic Standing gauge is safe).
const REGARRISON: i64 = 500;

/// Authoritative (hashed) free 2-D march state. `tx/ty` is the target town's position (set at muster).
#[derive(Clone, Default)]
pub struct RivalHostSoA {
    pub x_mm: Vec<i64>,
    pub y_mm: Vec<i64>,
    pub state: Vec<u8>,
    pub tx_mm: Vec<i64>,
    pub ty_mm: Vec<i64>,
}

impl RivalHostSoA {
    pub fn len(&self) -> usize {
        self.x_mm.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x_mm.is_empty()
    }
    /// Live (MARCHING) hosts — the bounded-population figure (DONE slots don't count).
    pub fn live(&self) -> usize {
        self.state.iter().filter(|&&s| s == MARCHING).count()
    }
    fn spawn(&mut self, x: i64, y: i64, tx: i64, ty: i64) -> Option<usize> {
        if let Some(i) = self.state.iter().position(|&s| s == DONE) {
            self.x_mm[i] = x;
            self.y_mm[i] = y;
            self.state[i] = MARCHING;
            self.tx_mm[i] = tx;
            self.ty_mm[i] = ty;
            Some(i)
        } else if self.len() < MAX_HOSTS {
            self.x_mm.push(x);
            self.y_mm.push(y);
            self.state.push(MARCHING);
            self.tx_mm.push(tx);
            self.ty_mm.push(ty);
            Some(self.len() - 1)
        } else {
            None // at cap — skip (logged-by-omission)
        }
    }
}

/// The rival hold's position (the first faction-1 barracks), or `None` if no rival realm exists.
fn rival_hold(world: &World) -> Option<(i64, i64)> {
    world.stations.iter().enumerate().find_map(|(i, s)| {
        (s.faction == 1 && !s.removed && world.is_barracks.get(i).copied().unwrap_or(false))
            .then_some((s.pos.x_mm, s.pos.y_mm))
    })
}

/// One rival-host tick: muster → march → resolve. A no-op (beyond the empty-SoA hash) without a rival hold.
pub fn step(world: &mut World, dt_ms: i64) {
    muster(world, dt_ms);
    march(world, dt_ms);
    resolve(world);
}

/// MUSTER: while the rival hold can afford a host AND the player holds captured ground, field one from the
/// hold at the muster cadence, aimed at the nearest captured town. Deterministic (fixed cadence, no rng).
fn muster(world: &mut World, dt_ms: i64) {
    let Some((hx, hy)) = rival_hold(world) else {
        return; // no rival realm ⇒ never musters (golden-neutral)
    };
    world.rival_muster_accum_ms = world.rival_muster_accum_ms.saturating_add(dt_ms.max(0));
    if world.rival_muster_accum_ms < MUSTER_PERIOD_MS || world.rival_manpower < MUSTER_COST {
        return; // not yet, or can't afford — the timer/treasury saves up
    }
    let Some((_, tx, ty)) = crate::raider::nearest_captured_town(world, hx, hy) else {
        return; // no captured ground to re-contest yet (early game) — hold the muster
    };
    if world.rival_hosts.spawn(hx, hy, tx, ty).is_some() {
        world.rival_muster_accum_ms = 0;
        world.rival_manpower -= MUSTER_COST;
    }
}

/// MARCH: step each host toward its target town along the terrain-cost hex path (swinging around
/// water/mountains via the same router the rails use), mirroring the raider march. Integer-exact.
fn march(world: &mut World, dt_ms: i64) {
    let step = HOST_SPEED_MM_S.saturating_mul(dt_ms.max(0)) / 1000;
    let gcm = world.city.grid_cell_mm;
    for i in 0..world.rival_hosts.len() {
        if world.rival_hosts.state[i] != MARCHING {
            continue;
        }
        let (x, y) = (world.rival_hosts.x_mm[i], world.rival_hosts.y_mm[i]);
        let (tx, ty) = (world.rival_hosts.tx_mm[i], world.rival_hosts.ty_mm[i]);
        let (wx, wy) = if gcm > 0 {
            let from = crate::hexgrid::axial_of(crate::geo_local::PointMm::new(x, y), gcm);
            let to = crate::hexgrid::axial_of(crate::geo_local::PointMm::new(tx, ty), gcm);
            if from == to {
                (tx, ty)
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
        let (dxt, dyt) = (tx - x, ty - y);
        let dist_t = (dxt.saturating_mul(dxt).saturating_add(dyt.saturating_mul(dyt))).isqrt();
        if dist_t <= step || dist_t == 0 {
            world.rival_hosts.x_mm[i] = tx;
            world.rival_hosts.y_mm[i] = ty;
            continue;
        }
        let (dx, dy) = (wx - x, wy - y);
        let dist = (dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))).isqrt();
        if dist <= step || dist == 0 {
            world.rival_hosts.x_mm[i] = wx;
            world.rival_hosts.y_mm[i] = wy;
        } else {
            world.rival_hosts.x_mm[i] = x + dx.saturating_mul(step) / dist;
            world.rival_hosts.y_mm[i] = y + dy.saturating_mul(step) / dist;
        }
    }
}

/// RESOLVE: a host that has reached its target RE-GARRISONS the nearest captured town — UNLESS that town is
/// inside the player's rail cordon (defended), in which case the host is repelled (cut down) with no effect.
/// So a railed conquest is safe; only exposed holdings are re-contestable. Index-ordered ⇒ deterministic.
fn resolve(world: &mut World) {
    let arr2 = ARRIVE_MM.saturating_mul(ARRIVE_MM);
    let def2 = crate::raider::DEFENSE_RANGE_MM.saturating_mul(crate::raider::DEFENSE_RANGE_MM);
    for i in 0..world.rival_hosts.len() {
        if world.rival_hosts.state[i] != MARCHING {
            continue;
        }
        let (x, y) = (world.rival_hosts.x_mm[i], world.rival_hosts.y_mm[i]);
        let (tx, ty) = (world.rival_hosts.tx_mm[i], world.rival_hosts.ty_mm[i]);
        let (dx, dy) = (tx - x, ty - y);
        if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) > arr2 {
            continue; // still marching
        }
        // Arrived: re-garrison the nearest captured town here, but only if it's UNDEFENDED (a railed
        // holding stays the player's). Either way the host spends itself (DONE).
        if let Some((t, ttx, tty)) = crate::raider::nearest_captured_town(world, x, y) {
            if !crate::raider::intercepted(world, ttx, tty, def2) {
                if let Some(v) = world.town_value.get_mut(t) {
                    *v = REGARRISON; // raise it back — the player must re-take it (towns_captured untouched)
                }
            }
        }
        world.rival_hosts.state[i] = DONE;
    }
}
