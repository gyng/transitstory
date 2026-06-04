//! Passengers with multi-leg routes. On arrival at a station a vehicle alights riders whose
//! current leg ends here (re-queueing transferers for their next leg, or counting arrivals),
//! then boards waiting riders whose current leg is on THIS line, FIFO up to capacity.
use crate::routing::Leg;
use crate::trainset::spec_for_mode;
use crate::world::World;

/// Load-dependent dwell: each boarding/alighting passenger adds time at the stop, capped. This is
/// what makes BUNCHING emergent — a crowded vehicle dwells longer, falls behind, and the (now
/// lighter-loaded) follower catches up. Integer + deterministic.
const DWELL_PER_PAX_MS: i64 = 250;
const MAX_EXTRA_DWELL_MS: i64 = 80_000;

#[derive(Clone, Debug)]
pub struct Pax {
    pub legs: Vec<Leg>,
    pub leg: usize,
    /// Clock (ms) when this passenger first spawned — drives end-to-end journey time.
    pub t_spawn_ms: i64,
    /// Clock (ms) the current leg began waiting (reset on transfer) — drives platform wait time.
    pub t_wait_ms: i64,
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
    for q in world.waiting.iter_mut() {
        if q.is_empty() {
            continue;
        }
        let mut kept = std::collections::VecDeque::with_capacity(q.len());
        for pax in q.drain(..) {
            if clock - pax.t_wait_ms > patience {
                gave_up += 1;
            } else {
                kept.push_back(pax);
            }
        }
        *q = kept;
    }
    world.abandoned += gave_up;
}

pub(crate) fn board_alight(world: &mut World) {
    let World {
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
        ..
    } = *world;

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
                let spec = spec_for_mode(l.mode);
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
                } else {
                    let mut p = pax;
                    p.leg += 1; // transfer: next leg boards here
                    p.t_wait_ms = clock_ms; // starts waiting for the next leg now
                    waiting[s].push_back(p);
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
            if wants_this && vehicles.onboard_pax[i].len() < cap {
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
                // A rider who wanted THIS line but found it full was left behind (real pressure).
                if wants_this {
                    *denied_boardings += 1;
                }
                requeue.push_back(pax);
            }
        }
        waiting[s] = requeue;
        vehicles.onboard[i] = vehicles.onboard_pax[i].len() as u16;

        // Load-dependent dwell: extend the (already-set) base dwell by the boarding/alighting
        // load, so a crowded vehicle falls behind and bunching emerges on an overloaded line.
        if base_dwell > 0 {
            let extra = (DWELL_PER_PAX_MS * (alighted_here + boarded_here)).min(MAX_EXTRA_DWELL_MS);
            vehicles.dwell_until_ms[i] = clock_ms + base_dwell + extra;
        }
    }
}
