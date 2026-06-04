//! Passengers with multi-leg routes. On arrival at a station a vehicle alights riders whose
//! current leg ends here (re-queueing transferers for their next leg, or counting arrivals),
//! then boards waiting riders whose current leg is on THIS line, FIFO up to capacity.
use crate::routing::Leg;
use crate::trainset::spec as trainset_spec;
use crate::world::World;

#[derive(Clone, Debug)]
pub struct Pax {
    pub legs: Vec<Leg>,
    pub leg: usize,
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
            .and_then(|l| l.trainset)
            .map(|t| trainset_spec(t.spec).capacity as usize)
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
                } else {
                    let mut p = pax;
                    p.leg += 1; // transfer: next leg boards here
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
            if vehicles.onboard_pax[i].len() < cap && pax.cur().line == line_id {
                vehicles.onboard_pax[i].push(pax);
                *ridership_total += 1;
                if s < boardings.len() {
                    boardings[s] += 1;
                }
            } else {
                requeue.push_back(pax);
            }
        }
        waiting[s] = requeue;
        vehicles.onboard[i] = vehicles.onboard_pax[i].len() as u16;
    }
}
