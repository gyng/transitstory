//! The war machine's legions (fantasy, S8). AI-launched armies that ride the player's rail network
//! toward enemy towns, besiege, and flip them. They live in a SEPARATE SoA from `VehicleSoA` — the
//! binding condition (fantasy-build-plan.md #2): `dispatch` rebuilds the shared `VehicleSoA` from
//! scratch on every `SetHeadway` (`v.clear()`), which would TELEPORT a legion mid-march. An army OWNS
//! its arc-length position here, untouched by dispatch. Movement is a plain constant-speed march (no
//! dwell, no boarding, no follow-clamp — those are passenger concerns); single-track admission via the
//! existing `occ_claim` and the siege/flip grind arrive in S8b.
//!
//! Determinism: integer arc-length, index-ordered iteration, keyed RNG (`seed ^ WAR_CONST`) when the
//! launch/targeting needs randomness (S8b). The authoritative fields are hashed in `Canonical`; the
//! cartesian `x_mm/y_mm` are render-only (derived from `s_mm`), excluded from the hash like vehicles'.
use crate::ids::LineId;
use crate::world::World;

/// Marching pace in mm per sim-second (a legion riding the rails). Balance knob — tuned via the
/// headless harness (tests/balance.rs) so the first conquest lands inside the design's ~60–120 s
/// "bites" window rather than dragging past it.
const ARMY_SPEED_MM_S: i64 = 50_000;
/// Tribute to field one legion — ties the war machine to the supply economy (feed towns → tribute →
/// armies). The non-derivable launch knob; balance-swept later (externalised to `CityData` then). S8a:
/// a flat, low cost so a modest supply network fields its first legion fast (harness-tuned: with the
/// production-gated tribute rate, this funds a legion in ~1 game-minute → the loop bites in-window).
const LAUNCH_COST: i64 = 8;
/// A defended town's BASE resistance (siege HP) — every town defends at least this much. Knob.
const RESISTANCE: i64 = 500;
/// S11 FRONTIER GARRISONS (the design's gate-safe Tier-1-LITE enemy: "static town garrisons"). A town's
/// resistance rises with its DEPTH in the decadence frontier — a town at the far edge (deep in the rot)
/// adds up to this much HP on top of `RESISTANCE`, so the expansion arc grades from soft (near the
/// capital) to hard (the corrupted marches). STATIC + deterministic (set once from the field's
/// distance-to-capital gradient — no mobile AI ⇒ none of the rival's livelock/sawtooth gate-blind risk).
/// 0 when there is no decadence field (transit + demo arcadia) ⇒ flat `RESISTANCE` ⇒ golden-neutral.
const GARRISON_MAX: i64 = 500;
/// Hard cap on concurrent legions — bounds the separate SoA (a runaway-launch backstop; the proper
/// per-tick bench gate is S10). Launches past this are skipped (logged-by-omission).
const MAX_ARMIES: usize = 256;

/// Army state.
pub const MARCHING: u8 = 0;
pub const BESIEGING: u8 = 1;
pub const DONE: u8 = 2; // captured/garrisoned/disbanded — inert, kept index-stable (never removed)

/// Separate Structure-of-Arrays for legions. Authoritative (hashed) fields + render-only cartesian.
#[derive(Clone, Default)]
pub struct ArmySoA {
    pub line: Vec<LineId>,
    pub path: Vec<u8>,
    /// Arc-length position along the route (mm) — the army OWNS this (never rebuilt by dispatch).
    pub s_mm: Vec<i64>,
    pub dir: Vec<i8>,
    pub strength: Vec<i64>,
    /// Target town (StationId) for the siege (S8b); carried now so the SoA layout is stable.
    pub target: Vec<u32>,
    pub state: Vec<u8>,
    /// Render-only cartesian (derived from `s_mm`); NOT hashed.
    pub x_mm: Vec<i64>,
    pub y_mm: Vec<i64>,
}

impl ArmySoA {
    pub fn len(&self) -> usize {
        self.s_mm.len()
    }
    pub fn is_empty(&self) -> bool {
        self.s_mm.is_empty()
    }
    /// Field a legion on `line`/`path` at arc-length `s_mm`, marching in `dir`.
    pub(crate) fn push(&mut self, line: LineId, path: u8, s_mm: i64, dir: i8, strength: i64, target: u32) {
        self.line.push(line);
        self.path.push(path);
        self.s_mm.push(s_mm);
        self.dir.push(dir);
        self.strength.push(strength);
        self.target.push(target);
        self.state.push(MARCHING);
        self.x_mm.push(0);
        self.y_mm.push(0);
    }
}

/// LAUNCH (the war_step's first sub-phase): if accrued tribute funds a legion, field one from the
/// first built route (deterministic: lowest line index with a positive-length trunk). Consumes the
/// tribute — the supply economy pays for the army. One per call (cadence refined in S8b). Targeting a
/// specific enemy town is S8b; S8a marches the route to learn the SoA is separate + deterministic.
pub(crate) fn maybe_launch(world: &mut World) {
    // S11 CONSCRIPTION: fielding a legion costs HALF the tribute when the tech is unlocked (same legion,
    // cheaper). 0 ⇒ the shipped `LAUNCH_COST`, byte-identical (transit never runs `war_step`; pre-tech
    // arcadia keeps `tech_unlocked` 0). Floored at 1 so a legion is never free.
    let launch_cost = if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::CONSCRIPTION) {
        (LAUNCH_COST / 2).max(1)
    } else {
        LAUNCH_COST
    };
    if world.tribute < launch_cost || world.armies.len() >= MAX_ARMIES {
        return;
    }
    // Launch from a BARRACKS on a built route (the player's agency: no barracks ⇒ no army). The first
    // such line (lowest index); the legion starts at the barracks's arc-length and marches to the
    // far-end town (its target). Deterministic: index-ordered find, captured before the mutation.
    let launch = world.lines.iter().enumerate().filter(|(_, l)| !l.removed).find_map(|(li, l)| {
        let path = l.paths.first()?;
        if path.length_mm() <= 0 {
            return None;
        }
        // A barracks stop anchors the launch.
        let b_idx = l.stops.iter().position(|s| world.is_barracks.get(s.index()).copied().unwrap_or(false))?;
        let b_arc = path.stop_arclen_mm.get(b_idx).copied().unwrap_or(0);
        // TARGET (Majesty steering): the highest-bounty UNCAPTURED town on this route, excluding the
        // barracks itself (tiebreak: lowest StationId, deterministic). No bounty anywhere ⇒ the route's
        // far-end town (the default conquest direction).
        let target = l
            .stops
            .iter()
            .filter(|s| !world.is_barracks.get(s.index()).copied().unwrap_or(false))
            .filter(|s| world.bounty.get(s.index()).copied().unwrap_or(0) > 0)
            .filter(|s| world.town_value.get(s.index()).map(|&v| v > 0).unwrap_or(true))
            .max_by_key(|s| (world.bounty.get(s.index()).copied().unwrap_or(0), core::cmp::Reverse(s.0)))
            .map(|s| s.0)
            .or_else(|| l.stops.last().map(|s| s.0))?;
        Some((li, b_arc, target))
    });
    if let Some((li, b_arc, target)) = launch {
        world.tribute -= launch_cost;
        // Strength stays the nominal LAUNCH_COST (a CONSCRIPTION legion is cheaper, not weaker).
        world.armies.push(LineId(li as u32), 0, b_arc, 1, LAUNCH_COST, target);
    }
}

/// MARCH (the move sub-phase): advance every marching legion along its route at a constant pace,
/// clamped to the route's arc-length. Owns `s_mm` (never touched by `dispatch::v.clear()`). Integer,
/// index-ordered ⇒ deterministic. The siege trigger (reaching the target) + grind/flip are S8b.
pub(crate) fn advance_armies(world: &mut World, dt_ms: i64) {
    let dt = dt_ms.max(0);
    // March pace is a per-city knob (externalised so the large baked continent's legions move at
    // continent scale). 0 ⇒ the `ARMY_SPEED_MM_S` default, so the demo + golden fixture are unchanged.
    let base = if world.city.army_speed_mm_s > 0 { world.city.army_speed_mm_s } else { ARMY_SPEED_MM_S };
    // S11 WAR_MARCH: legions march +50% faster (×3/2). 0 ⇒ ×1, byte-identical.
    let speed = if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::WAR_MARCH) { base * 3 / 2 } else { base };
    let step = speed.saturating_mul(dt) / 1000;
    for i in 0..world.armies.len() {
        if world.armies.state[i] != MARCHING {
            continue;
        }
        let line_idx = world.armies.line[i].index();
        let path_i = world.armies.path[i] as usize;
        let (total, target_arc) = match world.lines.get(line_idx).and_then(|l| {
            let p = l.paths.get(path_i)?;
            // The target town's arc-length on this route (bounty-steered; may be an intermediate stop).
            let t = world.armies.target[i];
            let arc = l
                .stops
                .iter()
                .position(|s| s.0 == t)
                .and_then(|idx| p.stop_arclen_mm.get(idx).copied())
                .unwrap_or_else(|| p.length_mm());
            Some((p.length_mm(), arc))
        }) {
            Some(v) => v,
            None => continue,
        };
        if total <= 0 {
            continue;
        }
        let dir = world.armies.dir[i] as i64;
        let s = (world.armies.s_mm[i] + dir * step).clamp(0, total);
        world.armies.s_mm[i] = s;
        // Reached the target town's arc-length ⇒ lay siege (an intermediate bounty target halts the
        // legion there; the default last-stop target halts it at the route end).
        if s >= target_arc {
            world.armies.state[i] = BESIEGING;
        }
    }
}

/// SIEGE (the grind→flip sub-phase): each BESIEGING legion grinds its target town's resistance by its
/// strength per tick; when resistance hits 0 the town FLIPS (captured) — counted EXACTLY ONCE (the
/// gate-blind hazard: a legion arriving at an already-captured town must not re-count it, it just
/// garrisons → DONE). Integer, index-ordered ⇒ deterministic. Supply-gated siege + bounties are the
/// next refinements. `town_value` is lazily sized to the node count, each new town at full `RESISTANCE`.
/// A station's FRONTIER garrison (S11): `RESISTANCE` + a bonus scaled by how deep in the decadence
/// frontier it sits (its hop-distance to the capital over the field, normalised by the tide's full span).
/// 0 bonus — flat `RESISTANCE` — when there is no field (transit / demo arcadia) or the station isn't on
/// the domain / is unreachable. Pure read of the STATIC field topology ⇒ deterministic + golden-neutral.
pub(crate) fn garrison_resistance(world: &World, station_idx: usize) -> i64 {
    let field = &world.decadence_field;
    if field.is_empty() || field.max_dist == 0 {
        return RESISTANCE;
    }
    let Some(st) = world.stations.get(station_idx) else { return RESISTANCE };
    let size = world.city.grid_cell_mm.max(1);
    let Some(&cell) = field.index.get(&crate::hexgrid::axial_of(st.pos, size)) else { return RESISTANCE };
    let dist = field.dist_to_capital[cell as usize];
    if dist == u32::MAX {
        return RESISTANCE; // off the capital-connected frontier ⇒ no garrison bonus
    }
    // Linear in frontier depth: 0 at the capital, GARRISON_MAX at the far edge. Integer-exact.
    RESISTANCE + (GARRISON_MAX.saturating_mul(dist as i64) / field.max_dist as i64)
}

pub(crate) fn siege(world: &mut World) {
    let n = world.stations.len();
    while world.town_value.len() < n {
        let idx = world.town_value.len();
        // A newly-revealed town defends at its FRONTIER garrison (base + depth-scaled bonus). The
        // immutable read finishes before the push (no borrow overlap).
        let r = garrison_resistance(world, idx);
        world.town_value.push(r);
    }
    for i in 0..world.armies.len() {
        if world.armies.state[i] != BESIEGING {
            continue;
        }
        let t = world.armies.target[i] as usize;
        if t >= world.town_value.len() {
            world.armies.state[i] = DONE; // no such town (e.g. a bulldozed target)
            continue;
        }
        if world.town_value[t] > 0 {
            // S11 siege techs: SIEGE_DOCTRINE grinds all sieges +50%; BOUNTY_MASTERY grinds a BOUNTIED
            // target an ADDITIONAL +50% (focus-fire your steered target). Multiplicative, integer. 0 techs
            // ⇒ ×1 ⇒ the shipped grind, byte-identical.
            let mut strength = world.armies.strength[i].max(1);
            if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::SIEGE_DOCTRINE) {
                strength = strength * 3 / 2;
            }
            if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::BOUNTY_MASTERY)
                && world.bounty.get(t).copied().unwrap_or(0) > 0
            {
                strength = strength * 3 / 2;
            }
            world.town_value[t] = (world.town_value[t] - strength).max(0);
            if world.town_value[t] == 0 {
                // The town falls — counted ONCE, on the grind→flip transition.
                world.towns_captured = world.towns_captured.saturating_add(1);
                world.armies.state[i] = DONE;
            }
        } else {
            // Town already captured ⇒ this legion garrisons, never a second capture count.
            world.armies.state[i] = DONE;
        }
    }
}
