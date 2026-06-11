//! Passengers with multi-leg routes. On arrival at a station a vehicle alights riders whose
//! current leg ends here (re-queueing transferers for their next leg, or counting arrivals),
//! then boards waiting riders whose current leg is on THIS line, FIFO up to capacity.
use crate::routing::Leg;
use crate::world::World;

/// Load-dependent dwell: each boarding/alighting passenger adds time at the stop, capped. This is
/// what makes BUNCHING emergent — a crowded vehicle dwells longer, falls behind, and the (now
/// lighter-loaded) follower catches up. Integer + deterministic. CLOCK-FRAME: 100 sim-ms = 3
/// clock-seconds per boarder. The cap is PER-SPEC (4× the vehicle's base dwell) rather than a
/// constant, so the feedback keeps the same relative strength for a 7-seat metro (≤2.8 s extra)
/// and a 250-seat aircraft (≤4 clock-min extra) alike — a flat cap sized for one mode killed
/// bunching for the other.
const DWELL_PER_PAX_MS: i64 = 100;
const MAX_EXTRA_DWELL_FACTOR: i64 = 4;

#[derive(Clone, Debug)]
pub struct Pax {
    pub legs: Vec<Leg>,
    pub leg: usize,
    /// Clock (ms) when this passenger first spawned — drives end-to-end journey time.
    pub t_spawn_ms: i64,
    /// Clock (ms) the current leg began waiting (reset on transfer) — drives platform wait time.
    pub t_wait_ms: i64,
    /// Citizen index this trip belongs to (agent demand), or `u32::MAX` for an anonymous gravity
    /// trip. Lets the journey inspector name the rider + show their home/work. Not hashed.
    pub citizen_id: u32,
}

impl Pax {
    pub fn cur(&self) -> Leg {
        self.legs[self.leg]
    }
    /// True if the current leg is the last (alighting it = arrival).
    pub fn on_last_leg(&self) -> bool {
        self.leg + 1 >= self.legs.len()
    }
}

/// Drop waiting riders whose current-leg wait has exceeded the city's patience (renege). This
/// is the only difficulty source with teeth in a money-free game: too-infrequent service loses
/// riders outright. Deterministic — integer clock vs integer `t_wait_ms`. Disabled when
/// `patience_ms <= 0` (e.g. `CityData::default()` in native tests).
pub(crate) fn renege(world: &mut World) {
    let patience = world.city.patience_ms;
    if patience <= 0 {
        return;
    }
    let clock = world.clock_ms;
    let mut gave_up = 0u64;
    // Disjoint field borrows: `waiting` and `abandoned_at` are distinct fields of `world`.
    let abandoned_at = &mut world.abandoned_at;
    for (s, q) in world.waiting.iter_mut().enumerate() {
        if q.is_empty() {
            continue;
        }
        let mut kept = std::collections::VecDeque::with_capacity(q.len());
        let mut here = 0u64;
        for pax in q.drain(..) {
            if clock - pax.t_wait_ms > patience {
                here += 1;
            } else {
                kept.push_back(pax);
            }
        }
        *q = kept;
        if here > 0 {
            gave_up += here;
            if let Some(slot) = abandoned_at.get_mut(s) {
                *slot += here; // bucket the renege to the station it happened at
            }
        }
    }
    world.abandoned += gave_up;
}

pub(crate) fn board_alight(world: &mut World) {
    let World {
        ref stations,
        ref lines,
        ref mut vehicles,
        ref mut waiting,
        ref mut ridership_total,
        ref mut boardings,
        ref mut alightings,
        clock_ms,
        ref mut total_journey_ms,
        ref mut journey_samples,
        ref mut total_wait_ms,
        ref mut wait_samples,
        ref mut denied_boardings,
        ref mut denied_at,
        ref mut recent_alight,
        ..
    } = *world;

    // Prune render-only walk-out breadcrumbs older than the walk window (and cap the buffer). This
    // touches no hashed state — recent_alight is excluded from Canonical, so it's determinism-free.
    let cutoff = clock_ms - crate::render_buf::PEEP_WALK_MS;
    while recent_alight.front().map(|r| r.t_ms < cutoff).unwrap_or(false) {
        recent_alight.pop_front();
    }
    while recent_alight.len() > crate::render_buf::MAX_RECENT_ALIGHT {
        recent_alight.pop_front();
    }

    for i in 0..vehicles.len() {
        let st = vehicles.at_station[i];
        if st < 0 {
            continue;
        }
        vehicles.at_station[i] = -1;
        let s = st as usize;
        if s >= waiting.len() {
            continue;
        }
        let line_id = vehicles.line[i];
        let (cap, base_dwell) = lines
            .get(line_id.index())
            .filter(|l| l.trainset.is_some())
            .map(|l| {
                let spec = l.vehicle_spec();
                (spec.capacity as usize, spec.dwell_ms)
            })
            .unwrap_or((0, 0));

        // Alight: riders whose current leg ends at this station leave the vehicle; transferers
        // advance a leg and re-queue here, arrivals are counted.
        let mut alighted_here: i64 = 0;
        let mut still_aboard: Vec<Pax> = Vec::with_capacity(vehicles.onboard_pax[i].len());
        for pax in std::mem::take(&mut vehicles.onboard_pax[i]) {
            if pax.cur().alight.index() == s {
                alighted_here += 1;
                if pax.on_last_leg() {
                    if s < alightings.len() {
                        alightings[s] += 1;
                    }
                    // Completed trip: fold end-to-end journey time into the running average.
                    *total_journey_ms += (clock_ms - pax.t_spawn_ms).max(0) as u64;
                    *journey_samples += 1;
                    // Render-only breadcrumb: this rider now walks out of the station (peeps). NOT
                    // hashed — recent_alight is excluded from Canonical, so this stays determinism-free.
                    recent_alight.push_back(crate::world::RecentAlight {
                        station: s as u32,
                        citizen: pax.citizen_id,
                        t_ms: clock_ms,
                    });
                } else {
                    let mut p = pax;
                    p.leg += 1; // transfer: advance to the next leg
                    let b = p.cur().board.index(); // where the next leg boards
                    if b == s {
                        // Same-station interchange (shared stop): re-queue here, no walk.
                        p.t_wait_ms = clock_ms;
                        waiting[s].push_back(p);
                    } else if b < waiting.len() {
                        // Footpath interchange: walk s→b. `t_wait_ms` is set to the arrival time at b,
                        // so the rider is "still walking" (and unboardable) until the clock reaches it.
                        let d = stations[s].pos.dist_mm(&stations[b].pos);
                        p.t_wait_ms = clock_ms.saturating_add(crate::demand::walk_ms(d));
                        waiting[b].push_back(p);
                    }
                }
            } else {
                still_aboard.push(pax);
            }
        }
        vehicles.onboard_pax[i] = still_aboard;

        // Board: waiting riders whose current leg is on THIS line, FIFO up to capacity;
        // others (waiting for a different line, or left behind) keep their order.
        let mut boarded_here: i64 = 0;
        let mut requeue = std::collections::VecDeque::with_capacity(waiting[s].len());
        while let Some(pax) = waiting[s].pop_front() {
            let wants_this = pax.cur().line == line_id;
            // A rider whose `t_wait_ms` is in the future is still WALKING in over a footpath — not
            // yet on the platform, so it can neither board nor be counted as passed-by.
            let ready = pax.t_wait_ms <= clock_ms;
            if wants_this && ready && vehicles.onboard_pax[i].len() < cap {
                // Boarded: fold platform wait time into the running average.
                *total_wait_ms += (clock_ms - pax.t_wait_ms).max(0) as u64;
                *wait_samples += 1;
                boarded_here += 1;
                vehicles.onboard_pax[i].push(pax);
                *ridership_total += 1;
                if s < boardings.len() {
                    boardings[s] += 1;
                }
            } else {
                // A rider who wanted THIS line, was on the platform, but found it full was left
                // behind (real pressure). Still-walking riders are skipped, not denied.
                if wants_this && ready {
                    *denied_boardings += 1;
                    if let Some(slot) = denied_at.get_mut(s) {
                        *slot += 1; // bucket the full-train pass-by to this station
                    }
                }
                requeue.push_back(pax);
            }
        }
        waiting[s] = requeue;
        vehicles.onboard[i] = vehicles.onboard_pax[i].len() as u16;

        // Load-dependent dwell: extend the (already-set) base dwell by the boarding/alighting
        // load, so a crowded vehicle falls behind and bunching emerges on an overloaded line.
        if base_dwell > 0 {
            let extra = (DWELL_PER_PAX_MS * (alighted_here + boarded_here)).min(MAX_EXTRA_DWELL_FACTOR * base_dwell);
            vehicles.dwell_until_ms[i] = clock_ms + base_dwell + extra;
        }
    }
}
