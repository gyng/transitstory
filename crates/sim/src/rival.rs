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

// --- P2: the rival BUILDS its own rail toward the player's capital (the literal "build tracks, same rules") ---
/// `rival_tribute` spent per track EXTENSION. When the budget (seeded at SeedRival) runs dry the rival stops
/// expanding — its own supply economy (minting more) comes in a later phase.
const BUILD_COST: i64 = 50;
/// Build cadence (ms): a deliberate extension every ~2 sim-min (slower than musters — laying rail is a
/// commitment). Authoritative (gates the next build) ⇒ hashed.
const BUILD_PERIOD_MS: i64 = 120_000;
/// How far each extension reaches toward the capital (mm) — snapped to a hex-cell centre (a valid node).
const BUILD_STEP_MM: i64 = 8_000_000;
/// Stop extending when the rail-head is within this of the capital (mm) — the rival creeps to your doorstep,
/// it does not build ONTO your seat (taking the capital is a win-condition concern for a later phase).
const BUILD_STOP_MM: i64 = 6_000_000;
/// The rival line's colour (u32 RGB) — crimson, matching the hold + the host dots (the rival realm's hue).
const RIVAL_LINE_COLOR: u32 = 0x00be_3737;

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

/// One rival tick: muster → march → resolve (the hosts, P1d) → build (extend its rail, P2). A no-op (beyond
/// the empty-SoA hash) without a rival hold.
pub fn step(world: &mut World, dt_ms: i64) {
    muster(world, dt_ms);
    march(world, dt_ms);
    resolve(world);
    build(world, dt_ms);
}

/// The rival hold's STATION INDEX (the first faction-1 barracks), or None.
fn rival_hold_id(world: &World) -> Option<usize> {
    world.stations.iter().enumerate().find_map(|(i, s)| {
        (s.faction == 1 && !s.removed && world.is_barracks.get(i).copied().unwrap_or(false)).then_some(i)
    })
}

/// The rival's LINE (the first faction-1, non-removed line), or None (no rail laid yet).
fn rival_line(world: &World) -> Option<usize> {
    world.lines.iter().position(|l| l.faction == 1 && !l.removed)
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

/// BUILD (P2): the rival creeps its rail ONE segment toward the player's capital — funded by `rival_tribute`,
/// at a deliberate cadence. The literal "the enemy builds tracks, by the SAME rules": a new faction-1 station
/// is placed via the player's own `PlaceStation` path (so the per-station arrays grow correctly), then the
/// rival's line is extended onto it (the first build roots a crimson line at the hold). Deterministic: a
/// fixed cadence + a capital-ward step snapped to the hex grid, no rng. Stops on a dry budget, at the
/// capital's doorstep, or against impassable terrain (the player's coast/range is a natural wall).
fn build(world: &mut World, dt_ms: i64) {
    let Some(hold) = rival_hold_id(world) else { return };
    world.rival_build_accum_ms = world.rival_build_accum_ms.saturating_add(dt_ms.max(0));
    if world.rival_build_accum_ms < BUILD_PERIOD_MS || world.rival_tribute < BUILD_COST {
        return; // not yet, or the build budget is spent (the rival's expansion has run its course)
    }
    let (cx, cy) = (world.city.capital_x_mm, world.city.capital_y_mm);
    // The rail-head: the rival line's last stop, else the hold (the first build roots the line at the hold).
    let line = rival_line(world);
    let head_id = match line {
        Some(lid) => world.lines[lid].stops.last().copied().map(|s| s.index()).unwrap_or(hold),
        None => hold,
    };
    let head = world.stations[head_id].pos;
    let (dx, dy) = (cx - head.x_mm, cy - head.y_mm);
    let dist = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)).isqrt();
    if dist <= BUILD_STOP_MM {
        return; // the rail-head has reached the player's doorstep — stop (don't build onto the capital)
    }
    // Step one segment toward the capital, snapped to a hex-cell centre (a valid track node).
    let step = dist.min(BUILD_STEP_MM);
    let gcm = world.city.grid_cell_mm.max(1);
    let raw = crate::geo_local::PointMm::new(head.x_mm + dx.saturating_mul(step) / dist, head.y_mm + dy.saturating_mul(step) / dist);
    let cell = crate::hexgrid::center_of(crate::hexgrid::axial_of(raw, gcm), gcm);
    // Terrain-bound: never plant a station on WATER/MOUNTAIN (a depot in the sea / on a cliff). The TRACK
    // between stops still routes AROUND water (recompute → line_costed); only the node must sit on land.
    let c = world.classify(cell.x_mm, cell.y_mm);
    if c == crate::city::class::WATER || c == crate::city::biome::MOUNTAIN {
        return; // halted against terrain — re-attempts next cadence (harmless), budget unspent
    }
    // Lay the new rail node via the player's command path (grows the per-station arrays), then flip it rival.
    let before = world.stations.len();
    world.apply(&crate::command::Command::PlaceStation { x_mm: cell.x_mm, y_mm: cell.y_mm, name: Some("Rival Rail".into()) });
    if world.stations.len() == before {
        return; // placement refused (shouldn't happen on passable land) — don't spend
    }
    world.stations[before].faction = 1;
    let new_sid = crate::ids::StationId(before as u32);
    let lid = match line {
        Some(lid) => {
            world.lines[lid].stops.push(new_sid);
            lid
        }
        None => {
            // First build: create the rival's crimson rail line, rooted at the hold, onto the new node.
            world.apply(&crate::command::Command::CreateLine {
                color: RIVAL_LINE_COLOR,
                name: Some("Rival Rail".into()),
                loop_line: false,
                mode: 0,
                literal: false,
            });
            let lid = world.lines.len() - 1;
            world.lines[lid].faction = 1;
            world.lines[lid].stops = vec![crate::ids::StationId(hold as u32), new_sid];
            lid
        }
    };
    world.recompute_line_buildability(crate::ids::LineId(lid as u32));
    world.demand_dirty = true;
    world.rival_tribute -= BUILD_COST;
    world.rival_build_accum_ms = 0;
}
