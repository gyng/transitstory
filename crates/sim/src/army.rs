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
pub(crate) const LAUNCH_COST: i64 = 8;
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

/// Army state. The TRAVEL sub-states (#legion-ride-trains) replace the old free-ride MARCHING with a
/// real decide → walk/wait → ride machine; BESIEGING/DONE are unchanged terminal states. New discriminants
/// are APPENDED (3/4/5) so a legion-free golden's 0/1/2 bytes are untouched.
pub const AT_STATION: u8 = 0; // (was MARCHING) at a station/barracks — decide walk-vs-wait this tick
pub const BESIEGING: u8 = 1;
pub const DONE: u8 = 2; // captured/garrisoned/disbanded — inert, kept index-stable (never removed)
pub const WALKING: u8 = 3; // advancing overland on a straight foot-leg toward the target
pub const WAITING: u8 = 4; // parked at a station, holding for a train on its chosen line+direction
pub const RIDING: u8 = 5; // aboard a real vehicle — position slaved to it (no free arc-length integration)
/// Back-compat alias — launch still enters at AT_STATION (the old MARCHING discriminant 0).
pub const MARCHING: u8 = AT_STATION;

/// Separate Structure-of-Arrays for legions. Authoritative (hashed) fields + render-only cartesian.
#[derive(Clone, Default)]
pub struct ArmySoA {
    pub line: Vec<LineId>,
    pub path: Vec<u8>,
    /// Arc-length position along the route (mm) — the army OWNS this (never rebuilt by dispatch). While
    /// RIDING it is MIRRORED from the carrying vehicle (the legion no longer slides for free).
    pub s_mm: Vec<i64>,
    pub dir: Vec<i8>,
    pub strength: Vec<i64>,
    /// Target town (StationId) for the siege (S8b); carried now so the SoA layout is stable.
    pub target: Vec<u32>,
    pub state: Vec<u8>,
    // --- travel sub-state (#legion-ride-trains), all hashed. ON-LINE model: a legion always lives on its
    // line's arc-length `s_mm` — WALKING advances it at WALK speed (trudging the corridor on foot), RIDING
    // mirrors a boarded train's `s_mm` (a real, capacity-contended ride). So `point_at(s_mm)`, the siege, and
    // the render need no off-rail special-casing. ---
    /// Line chosen while WAITING/RIDING (the legion boards/rides this line; = its own line today), -1 else.
    pub wait_line: Vec<i32>,
    /// Travel direction chosen while WAITING/RIDING (toward the target arc), 0 otherwise.
    pub wait_dir: Vec<i8>,
    /// Carrying vehicle slab index while RIDING, -1 otherwise.
    pub riding_veh: Vec<i32>,
    /// Patience deadline (ms) while WAITING — if no train comes by then, re-decide (usually → WALK); 0 else.
    pub wait_until_ms: Vec<i64>,
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
        self.state.push(AT_STATION);
        self.wait_line.push(-1);
        self.wait_dir.push(0);
        self.riding_veh.push(-1);
        self.wait_until_ms.push(0);
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
    // V3 economy: legions are fielded with MANPOWER (the arms+food economy — grain/ingot), not gold.
    if world.manpower < launch_cost || world.armies.len() >= MAX_ARMIES {
        return;
    }
    // Launch from EVERY BARRACKS on a built route (#war "more legions": each base you build fields its own
    // legion this tick, toward its own line's target — so multiple barracks open multiple fronts; one
    // barracks behaves exactly as before). Collect the candidates in line-index order (deterministic,
    // captured before any mutation), then field each while manpower + the SoA cap allow.
    // Candidate launches: (load, line index, barracks arc-length, target). `load` = the legions already on
    // this base's line, so a tight manpower pool is shared FAIRLY (the neediest base fields first) instead
    // of the lowest-index barracks monopolising it. No new state — recomputed from the live SoA each tick;
    // one barracks ⇒ the single candidate, identical to before.
    let mut launches = world
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.removed)
        .filter_map(|(li, l)| {
            let path = l.paths.first()?;
            if path.length_mm() <= 0 {
                return None;
            }
            // A barracks stop anchors the launch.
            let b_idx = l.stops.iter().position(|s| world.is_barracks.get(s.index()).copied().unwrap_or(false))?;
            let b_arc = path.stop_arclen_mm.get(b_idx).copied().unwrap_or(0);
            // The barracks station's position (#war legibility: the LAUNCH burst fires here).
            let b_pos = world.stations.get(l.stops[b_idx].index()).map(|s| (s.pos.x_mm, s.pos.y_mm)).unwrap_or((0, 0));
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
            let load = world.armies.line.iter().filter(|&&l| l == LineId(li as u32)).count();
            Some((load, li, b_arc, target, b_pos))
        })
        .collect::<Vec<(usize, usize, i64, u32, (i64, i64))>>();
    // Fewest-legions base first (tiebreak: lowest line index) ⇒ deterministic load-balancing.
    launches.sort_by_key(|&(load, li, _, _, _)| (load, li));
    for (_load, li, b_arc, target, b_pos) in launches {
        if world.manpower < launch_cost || world.armies.len() >= MAX_ARMIES {
            break; // out of manpower / at the SoA cap — the rest of the barracks wait for the next tick
        }
        world.manpower -= launch_cost; // V3: legions drawn from manpower
        // Strength stays the nominal LAUNCH_COST (a CONSCRIPTION legion is cheaper, not weaker).
        world.armies.push(LineId(li as u32), 0, b_arc, 1, LAUNCH_COST, target);
        crate::spell::fx_burst(world, crate::spell::FX_LAUNCH, b_pos.0, b_pos.1); // #war: echo the field (was silent)
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
