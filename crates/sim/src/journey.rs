//! Journey inspector query: peek a waiting rider at a station and describe their trip. Under
//! agent demand the rider is a NAMED commuter with a home + workplace; under gravity they're an
//! anonymous trip (just the route). Pure read of `World` (drives the "Commuter" card) — serde-out
//! for the wasm boundary, camelCase to match the other views.
use crate::world::World;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyView {
    /// Citizen id to follow (u32::MAX when anonymous → no follow).
    pub citizen_id: u32,
    /// Rider name (empty when anonymous/gravity).
    pub name: String,
    /// True = anonymous gravity trip (no home/work); false = a named citizen.
    pub anonymous: bool,
    pub home: String, // home station name (nearest to where they live)
    pub work: String, // workplace station name
    pub origin: String, // this trip's first boarding station
    pub dest: String, // this trip's final station
    pub here: String, // station they're waiting at right now
    pub legs: Vec<JourneyLeg>,
    pub leg: u32, // current leg index (which hop they're on)
    pub wait_min: f64, // minutes waited on the current leg
    pub queue_len: u32, // riders waiting at `here` (for the "1 of N" affordance)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyLeg {
    pub line_name: String,
    pub line_color: u32,
    pub board: String,
    pub alight: String,
}

/// Live state of a FOLLOWED citizen — where they are right now (waiting at a station, or riding a
/// vehicle) + their journey progress. Located by scanning the waiting queues + onboard pax for the
/// id (O(pax), but only for one followed citizen on the ~3 Hz tick). None = not currently in
/// transit (arrived / not yet departed).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowView {
    pub name: String,
    pub home: String,
    pub work: String,
    pub dest: String,
    pub onboard: bool, // true = riding a vehicle, false = waiting on a platform
    pub at: String, // station name (waiting) OR line name (onboard) — the headline status
    pub line_color: u32,
    pub station: i32, // waiting station id (else -1) — for the map locator
    pub vehicle: i32, // onboard vehicle index (else -1) — for the map locator
    pub legs: Vec<JourneyLeg>,
    pub leg: u32,
    pub wait_min: f64,
    pub journey_min: f64,
}

/// Locate a followed citizen and describe their live state. None if they aren't in transit now.
pub fn follow(world: &World, citizen_id: u32) -> Option<FollowView> {
    if citizen_id == u32::MAX {
        return None;
    }
    // Waiting on a platform?
    for (s, q) in world.waiting.iter().enumerate() {
        if let Some(pax) = q.iter().find(|p| p.citizen_id == citizen_id) {
            return Some(build_follow(world, pax, false, s as i32, -1));
        }
    }
    // Riding a vehicle?
    for vi in 0..world.vehicles.len() {
        if let Some(pax) = world.vehicles.onboard_pax[vi].iter().find(|p| p.citizen_id == citizen_id) {
            return Some(build_follow(world, pax, true, -1, vi as i32));
        }
    }
    None
}

fn build_follow(world: &World, pax: &crate::pax::Pax, onboard: bool, station: i32, vehicle: i32) -> FollowView {
    let st_name = |id: u32| world.stations.get(id as usize).map(|s| s.name.clone()).unwrap_or_default();
    let line_name = |lid: crate::ids::LineId| {
        world
            .lines
            .get(lid.index())
            .map(|l| if l.name.is_empty() { format!("Line {}", lid.0 + 1) } else { l.name.clone() })
            .unwrap_or_default()
    };
    let legs: Vec<JourneyLeg> = pax
        .legs
        .iter()
        .map(|leg| {
            let l = world.lines.get(leg.line.index());
            JourneyLeg {
                line_name: line_name(leg.line),
                line_color: l.map(|l| l.color).unwrap_or(0x888888),
                board: st_name(leg.board.0),
                alight: st_name(leg.alight.0),
            }
        })
        .collect();
    let cur = pax.legs.get(pax.leg);
    let (name, home, work) = match world.population.as_ref() {
        Some(pop) if pax.citizen_id != u32::MAX => (
            pop.name(pax.citizen_id),
            pop.home_station(pax.citizen_id).map(|s| st_name(s)).unwrap_or_default(),
            pop.work_station(pax.citizen_id).map(|s| st_name(s)).unwrap_or_default(),
        ),
        _ => (String::new(), String::new(), String::new()),
    };
    FollowView {
        name,
        home,
        work,
        onboard,
        dest: legs.last().map(|l| l.alight.clone()).unwrap_or_default(),
        at: if onboard {
            cur.map(|leg| line_name(leg.line)).unwrap_or_default()
        } else {
            st_name(station as u32)
        },
        line_color: cur.and_then(|leg| world.lines.get(leg.line.index())).map(|l| l.color).unwrap_or(0x888888),
        station,
        vehicle,
        legs,
        leg: pax.leg as u32,
        wait_min: (world.clock_ms - pax.t_wait_ms).max(0) as f64 / 60_000.0,
        journey_min: (world.clock_ms - pax.t_spawn_ms).max(0) as f64 / 60_000.0,
    }
}

/// Describe the `nth` waiting rider at `station` (wraps if nth ≥ queue length). None if empty.
pub fn sample(world: &World, station: u32, nth: usize) -> Option<JourneyView> {
    let q = world.waiting.get(station as usize)?;
    if q.is_empty() {
        return None;
    }
    let pax = q.get(nth % q.len())?;
    let st_name = |id: u32| world.stations.get(id as usize).map(|s| s.name.clone()).unwrap_or_default();

    let legs: Vec<JourneyLeg> = pax
        .legs
        .iter()
        .map(|leg| {
            let l = world.lines.get(leg.line.index());
            JourneyLeg {
                line_name: l
                    .map(|l| if l.name.is_empty() { format!("Line {}", leg.line.0 + 1) } else { l.name.clone() })
                    .unwrap_or_default(),
                line_color: l.map(|l| l.color).unwrap_or(0x888888),
                board: st_name(leg.board.0),
                alight: st_name(leg.alight.0),
            }
        })
        .collect();
    let origin = legs.first().map(|l| l.board.clone()).unwrap_or_default();
    let dest = legs.last().map(|l| l.alight.clone()).unwrap_or_default();

    // Identity: a named citizen (agent demand) or an anonymous gravity rider.
    let (name, anonymous, home, work) = match (pax.citizen_id != u32::MAX).then_some(()).and(world.population.as_ref()) {
        Some(pop) if pax.citizen_id != u32::MAX => (
            pop.name(pax.citizen_id),
            false,
            pop.home_station(pax.citizen_id).map(|s| st_name(s)).unwrap_or_default(),
            pop.work_station(pax.citizen_id).map(|s| st_name(s)).unwrap_or_default(),
        ),
        _ => (String::new(), true, String::new(), String::new()),
    };

    Some(JourneyView {
        citizen_id: pax.citizen_id,
        name,
        anonymous,
        home,
        work,
        origin,
        dest,
        here: st_name(station),
        legs,
        leg: pax.leg as u32,
        wait_min: (world.clock_ms - pax.t_wait_ms).max(0) as f64 / 60_000.0,
        queue_len: q.len() as u32,
    })
}
