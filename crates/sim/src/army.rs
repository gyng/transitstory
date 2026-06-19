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

/// A marginal-rail bias (ms): a legion only WAITS for a train when rail beats walking by at least this
/// much — it models a legion's reluctance to gamble on a train that might be raided. Integer, city-
/// independent. (#legion-ride-trains.)
const WAIT_RISK_MS: i64 = 60_000;
/// How long a WAITING legion holds out for a train before re-deciding (ms). If no train comes (line cut /
/// no service), patience lapses → it re-decides AT_STATION → usually WALKS. Bounds the wait so a legion is
/// never stranded forever on a dead line.
const WAIT_PATIENCE_MS: i64 = 240_000;
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
/// #23 TG2 — the one-time CONQUEST BOUNTY by a captured town's SIZE: a bigger town is a richer prize (tribute
/// to bank + manpower to refuel the next legion), so a bigger siege pays for itself. Computed from the size
/// AT capture and FROZEN (a captured town never grows). 0 for a fresh frontier town (size 0).
const BOUNTY_GOLD_PER_SIZE: i64 = 400; // tribute per size (size-5 ≈ 2000)
const BOUNTY_MANPOWER_BASE: i64 = 4; // manpower for any capture
// #25 was 8 — the linear manpower bounty had no diminishing term: a size-5 capture minted ~44 manpower
// (~5.5 legions), so each conquest funded ~5 more → the next siege sooner → a flattening late-game runway
// where the first town is hard and every subsequent one nearly free. At 3 a size-5 mints 4+15 = 19 (~2.4
// legions): a bigger siege still pays MORE, but the geometric refuel that trivialised the snowball is gone.
const BOUNTY_MANPOWER_PER_SIZE: i64 = 3; // + per size (size-5 = 4+15 ≈ 2.4 legions at LAUNCH_COST 8)
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

/// Is the legion still travelling to its target (deciding / walking / waiting / riding) — as opposed to
/// BESIEGING or DONE? The render draws a forward intent arc for en-route legions; the terminal states collapse
/// it. Shared so the render and any future UI agree on "en route".
pub fn is_en_route(state: u8) -> bool {
    matches!(state, AT_STATION | WALKING | WAITING | RIDING)
}

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
            // Legions already afield on this line (deterministic count): the fairness sort key AND the
            // fan-out cursor below.
            let load = world.armies.line.iter().filter(|&&l| l == LineId(li as u32)).count();
            // TARGET. The player's MAJESTY steering wins: the highest-bounty UNCAPTURED town on the route
            // (tiebreak lowest StationId, deterministic). With NO bounty, FAN OUT instead of conga-lining —
            // round-robin the route's uncaptured TOWNS by `load`, so successive legions open DIFFERENT
            // fronts rather than all marching the same corridor to the far end (the "one long line" fix). No
            // town on the route ⇒ the far-end stop (a resource spur's default direction). Index-ordered +
            // integer ⇒ deterministic.
            let bounty_target = l
                .stops
                .iter()
                .filter(|s| !world.is_barracks.get(s.index()).copied().unwrap_or(false))
                .filter(|s| world.bounty.get(s.index()).copied().unwrap_or(0) > 0)
                .filter(|s| world.town_value.get(s.index()).map(|&v| v > 0).unwrap_or(true))
                .max_by_key(|s| (world.bounty.get(s.index()).copied().unwrap_or(0), core::cmp::Reverse(s.0)))
                .map(|s| s.0);
            let target = match bounty_target {
                Some(t) => t,
                None => {
                    let towns: Vec<u32> = l
                        .stops
                        .iter()
                        .filter(|s| !world.is_barracks.get(s.index()).copied().unwrap_or(false))
                        .filter(|s| world.town_value.get(s.index()).map(|&v| v > 0).unwrap_or(false))
                        .map(|s| s.0)
                        .collect();
                    if towns.is_empty() {
                        l.stops.last().map(|s| s.0)?
                    } else {
                        towns[load % towns.len()]
                    }
                }
            };
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

/// The legion's on-foot pace (mm/s). A legion WALKS at the citizen walk speed (`demand::WALK_SPEED_MM_S`),
/// optionally overridden per-city (`army_speed_mm_s`, externalised so a continent-scale baked map can march
/// legions at continent scale) and sped up by the WAR_MARCH tech (+50%). 0 override ⇒ the walk default, so the
/// demo + golden fixtures are unchanged. (Riding is at the train's own speed, not this — see `army_travel_step`.)
fn walk_speed(world: &World) -> i64 {
    let base = if world.city.army_speed_mm_s > 0 { world.city.army_speed_mm_s } else { crate::demand::WALK_SPEED_MM_S };
    if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::WAR_MARCH) { base * 3 / 2 } else { base }
}

/// The target town's arc-length on the legion's route (bounty-steered; may be an intermediate stop), and the
/// route's total length. `None` if the route/path is gone (a bulldozed line). Legions launch on path 0 (the
/// trunk), so `l.stops` is this path's stop list. Shared by every travel sub-state so they agree on the goal.
fn target_arc(world: &World, i: usize) -> Option<(i64, i64)> {
    let l = world.lines.get(world.armies.line[i].index())?;
    let p = l.paths.get(world.armies.path[i] as usize)?;
    let t = world.armies.target[i];
    let arc = l
        .stops
        .iter()
        .position(|s| s.0 == t)
        .and_then(|idx| p.stop_arclen_mm.get(idx).copied())
        .unwrap_or_else(|| p.length_mm());
    Some((p.length_mm(), arc))
}

/// TRAVEL (#legion-ride-trains): the legion's decide → walk / wait → ride → arrive machine, replacing the old
/// free-ride march. A legion always lives on its line's arc-length `s_mm` (the ON-LINE model). Per state:
/// - **AT_STATION** — decide walk-vs-wait: compare a WALK estimate (corridor distance ÷ walk speed) against a
///   RAIL estimate (`headway/2` cold-start wait + in-vehicle ride at the train's top speed). Take rail only if
///   it beats walking by `WAIT_RISK_MS` AND the legion fits a seat (1 seat per strength) AND the line is
///   serviced + not raided; else WALK. (Already at the target arc ⇒ BESIEGE.)
/// - **WALKING** — trudge the corridor on foot: advance `s_mm` toward the target at `walk_speed`. On arrival ⇒
///   BESIEGE. (Committed — a walking legion does not re-evaluate; it chose the foot-leg.)
/// - **WAITING** — hold at the station for a train (the actual board is `army_board`, after pax board). If
///   patience lapses (no train came) ⇒ re-decide AT_STATION (usually WALK now).
/// - **RIDING** — `s_mm` MIRRORS the carrying vehicle (no free arc-length integration — a real, contended
///   ride). On the vehicle reaching the target arc ⇒ alight + BESIEGE. If the ride is lost (a dispatch
///   `v.clear()` rebuild / line removed) ⇒ drop to AT_STATION at the last known spot and re-decide.
///
/// Integer + index-ordered ⇒ deterministic. Owns `s_mm` (never touched by `dispatch::v.clear()`).
pub(crate) fn army_travel_step(world: &mut World, dt_ms: i64) {
    let dt = dt_ms.max(0);
    let clock = world.clock_ms;
    let wspeed = walk_speed(world).max(1);
    let walk_step = wspeed.saturating_mul(dt) / 1000;
    let n_v = world.vehicles.len();
    for i in 0..world.armies.len() {
        match world.armies.state[i] {
            AT_STATION => {
                let Some((_total, t_arc)) = target_arc(world, i) else {
                    world.armies.state[i] = DONE;
                    continue;
                };
                let s = world.armies.s_mm[i];
                if s == t_arc {
                    world.armies.state[i] = BESIEGING; // already there ⇒ besiege (preserves the old arrive→siege edge)
                    world.armies.wait_line[i] = -1;
                    world.armies.wait_dir[i] = 0;
                    world.armies.riding_veh[i] = -1;
                    continue;
                }
                let li = world.armies.line[i].index();
                let loop_line = world.lines[li].paths.get(world.armies.path[i] as usize).map(|p| p.loop_line).unwrap_or(false);
                let dir: i8 = if loop_line || t_arc >= s { 1 } else { -1 };
                let remaining = (t_arc - s).abs();
                let walk_est = remaining.saturating_mul(1000) / wspeed;
                // RAIL estimate: only finite when the line runs trains, isn't raided, and the legion FITS a seat
                // (1 seat per strength — a legion too big for the stock can never board ⇒ it must walk).
                let line = &world.lines[li];
                let serviced = line.trainset.map(|t| t.count > 0).unwrap_or(false) && !world.line_disabled(li);
                let wait_est = if serviced {
                    let spec = line.vehicle_spec();
                    if world.armies.strength[i].max(1) > spec.capacity as i64 {
                        i64::MAX
                    } else {
                        let ride_ms = remaining.saturating_mul(1000) / spec.v_max_mm_s.max(1);
                        // #23 the cold-start wait is HALF the arrival spacing — which the dispatcher sets to
                        // round-trip / train-count (dispatch.rs), NOT the stored headway_ms slider (it never reads
                        // it; the old headway_ms/2 could be the 120 s MAX while 8 trains arrive seconds apart, or
                        // vice-versa). This uses the ASSIGNED count (uncapped) — on a capped or multi-path line the
                        // real spacing is wider, so it slightly OVER-estimates service and errs toward walking;
                        // post-cap/per-path fidelity is a logged follow-up. [State-affecting but GOLDEN-NEUTRAL in
                        // the pinned scenario — same walk/ride verdict, no re-pin; verified arcadia/determinism/
                        // position_fingerprint/legion_rides/balance.]
                        let count = line.trainset.map(|t| t.count.max(1)).unwrap_or(1) as i64;
                        let path_total = line.paths.get(world.armies.path[i] as usize).map(|p| p.length_mm()).unwrap_or(0);
                        let round = if loop_line { path_total } else { 2 * path_total };
                        let spacing_ms = round.saturating_mul(1000) / spec.v_max_mm_s.max(1) / count.max(1);
                        (spacing_ms / 2).saturating_add(ride_ms)
                    }
                } else {
                    i64::MAX
                };
                let take_rail = wait_est != i64::MAX && wait_est.saturating_add(WAIT_RISK_MS) <= walk_est;
                world.armies.dir[i] = dir;
                if take_rail {
                    world.armies.state[i] = WAITING;
                    world.armies.wait_line[i] = li as i32;
                    world.armies.wait_dir[i] = dir;
                    world.armies.wait_until_ms[i] = clock.saturating_add(WAIT_PATIENCE_MS);
                } else {
                    world.armies.state[i] = WALKING;
                    world.armies.wait_line[i] = -1;
                    world.armies.wait_dir[i] = 0;
                }
            }
            WALKING => {
                let Some((total, t_arc)) = target_arc(world, i) else {
                    world.armies.state[i] = DONE;
                    continue;
                };
                // March by DAY, make CAMP by night (#daynight): an overland foot-march holds position
                // through the dark and resumes at dawn. Rail-borne legions (RIDING) ride on — your rail is
                // the 24/7 logistics; only the foot-march rests. Integer day-phase ⇒ deterministic. The
                // render reads this same hold (WALKING + night) to pitch a campfire.
                if !crate::tod::is_daylight(clock) {
                    continue; // camped until dawn — no advance
                }
                let dir = world.armies.dir[i] as i64;
                let ns = world.armies.s_mm[i] + dir * walk_step;
                let reached = (dir >= 0 && ns >= t_arc) || (dir < 0 && ns <= t_arc);
                if reached {
                    world.armies.s_mm[i] = t_arc;
                    world.armies.state[i] = BESIEGING;
                } else {
                    world.armies.s_mm[i] = ns.clamp(0, total);
                }
            }
            WAITING => {
                if clock >= world.armies.wait_until_ms[i] {
                    world.armies.state[i] = AT_STATION; // no train came — re-decide (usually WALK now)
                    world.armies.wait_line[i] = -1;
                    world.armies.wait_dir[i] = 0;
                }
            }
            RIDING => {
                let rv = world.armies.riding_veh[i];
                let valid = rv >= 0
                    && (rv as usize) < n_v
                    && world.vehicles.line[rv as usize] == world.armies.line[i]
                    && world.vehicles.path[rv as usize] == world.armies.path[i];
                if !valid {
                    // Lost the ride (a SetHeadway's dispatch rebuild cleared the vehicles, or the line was
                    // bulldozed) → fall off at the last known arc-length and re-decide next tick. Never teleport.
                    world.armies.state[i] = AT_STATION;
                    world.armies.riding_veh[i] = -1;
                    world.armies.wait_line[i] = -1;
                    world.armies.wait_dir[i] = 0;
                    continue;
                }
                let Some((_total, t_arc)) = target_arc(world, i) else {
                    world.armies.state[i] = DONE;
                    continue;
                };
                let vs = world.vehicles.s_mm[rv as usize];
                let dir = world.armies.dir[i] as i64;
                let reached = (dir >= 0 && vs >= t_arc) || (dir < 0 && vs <= t_arc);
                if reached {
                    world.armies.s_mm[i] = t_arc; // alight at the target ⇒ besiege
                    world.armies.state[i] = BESIEGING;
                    world.armies.riding_veh[i] = -1;
                    world.armies.wait_line[i] = -1;
                    world.armies.wait_dir[i] = 0;
                } else {
                    world.armies.s_mm[i] = vs; // mirror the carrying vehicle — no free slide
                }
            }
            _ => {} // BESIEGING / DONE — terminal, frozen
        }
    }
}

/// BOARD (#legion-ride-trains): seat WAITING legions onto dwelling vehicles — a real, capacity-contended ride.
/// Runs AFTER `pax::board_alight` (so paying citizens always win the seat contention; legions take leftovers)
/// and BEFORE `siege`. A legion boards a vehicle iff: the vehicle is dwelling at a stop, on the legion's own
/// line+path, travelling the legion's chosen direction, parked at the legion's stop arc, the line is serviced,
/// and there are `strength` free seats (1 seat per strength). Seats already held by riding legions count against
/// capacity, derived fresh from the hashed `riding_veh`+`strength` (no separate stored seat cache to drift).
/// Index-ordered over vehicles, then over legions (FIFO by launch order) ⇒ deterministic.
pub(crate) fn army_board(world: &mut World) {
    let clock = world.clock_ms;
    let n_v = world.vehicles.len();
    if world.armies.is_empty() || n_v == 0 {
        return;
    }
    // Seats already taken per vehicle: paying pax + legions currently riding it (strength = seats).
    let mut occupied: Vec<i64> = (0..n_v).map(|v| world.vehicles.onboard_pax[v].len() as i64).collect();
    for a in 0..world.armies.len() {
        if world.armies.state[a] == RIDING {
            let rv = world.armies.riding_veh[a];
            if rv >= 0 && (rv as usize) < n_v {
                occupied[rv as usize] = occupied[rv as usize].saturating_add(world.armies.strength[a].max(1));
            }
        }
    }
    for v in 0..n_v {
        if clock >= world.vehicles.dwell_until_ms[v] {
            continue; // not dwelling — can't board (matches the pax board signal, derived post-`at_station` reset)
        }
        let line_id = world.vehicles.line[v];
        let path_i = world.vehicles.path[v] as usize;
        let vdir = world.vehicles.dir[v];
        let vs = world.vehicles.s_mm[v];
        let (cap, serviced) = {
            let Some(line) = world.lines.get(line_id.index()) else { continue };
            let Some(path) = line.paths.get(path_i) else { continue };
            if !path.stop_arclen_mm.iter().any(|&arc| arc == vs) {
                continue; // dwelling between stops (shouldn't happen) — only board at a stop arc
            }
            (line.vehicle_spec().capacity as i64, line.trainset.map(|t| t.count > 0).unwrap_or(false))
        };
        if !serviced {
            continue;
        }
        for a in 0..world.armies.len() {
            if world.armies.state[a] != WAITING
                || world.armies.line[a] != line_id
                || world.armies.path[a] as usize != path_i
                || world.armies.wait_dir[a] != vdir
                || world.armies.s_mm[a] != vs
            {
                continue;
            }
            let need = world.armies.strength[a].max(1);
            if occupied[v].saturating_add(need) > cap {
                continue; // not enough free seats — this legion waits for a roomier service (or its patience lapses)
            }
            occupied[v] = occupied[v].saturating_add(need);
            world.armies.state[a] = RIDING;
            world.armies.riding_veh[a] = v as i32;
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
                // #23 TG2 — mint the one-time CONQUEST BOUNTY from the town's SIZE at capture (frozen: the town
                // stops growing now). A bigger prize banks more tribute + manpower to fund the next legion.
                let sz = world.town_size.get(t).copied().unwrap_or(0);
                world.tribute = world.tribute.saturating_add(BOUNTY_GOLD_PER_SIZE.saturating_mul(sz));
                world.manpower = world.manpower.saturating_add(BOUNTY_MANPOWER_BASE + BOUNTY_MANPOWER_PER_SIZE.saturating_mul(sz));
                world.armies.state[i] = DONE;
            }
        } else {
            // Town already captured ⇒ this legion garrisons, never a second capture count.
            world.armies.state[i] = DONE;
        }
    }
}
