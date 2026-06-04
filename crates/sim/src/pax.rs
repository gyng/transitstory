//! Passengers with multi-leg routes. On arrival at a station a vehicle alights riders whose
//! current leg ends here (re-queueing transferers for their next leg, or counting arrivals),
//! then boards waiting riders whose current leg is on THIS line, FIFO up to capacity.
use crate::routing::Leg;
use crate::trainset::spec_for_mode;
use crate::world::World;

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
        let cap = lines
            .get(line_id.index())
            .filter(|l| l.trainset.is_some())
            .map(|l| spec_for_mode(l.mode).capacity as usize)
            .unwrap_or(0);

        // Alight: riders whose current leg ends at this station leave the vehicle; transferers
        // advance a leg and re-queue here, arrivals are counted.
        let mut still_aboard: Vec<Pax> = Vec::with_capacity(vehicles.onboard_pax[i].len());
        for pax in std::mem::take(&mut vehicles.onboard_pax[i]) {
            if pax.cur().alight.index() == s {
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
        let mut requeue = std::collections::VecDeque::with_capacity(waiting[s].len());
        while let Some(pax) = waiting[s].pop_front() {
            let wants_this = pax.cur().line == line_id;
            if wants_this && vehicles.onboard_pax[i].len() < cap {
                // Boarded: fold platform wait time into the running average.
                *total_wait_ms += (clock_ms - pax.t_wait_ms).max(0) as u64;
                *wait_samples += 1;
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
    }
}
