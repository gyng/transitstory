//! Passenger boarding/alighting. When a vehicle arrives at a station (recorded by
//! `vehicle::advance`), alight onboard passengers whose destination is this stop, then board
//! waiting passengers FIFO up to the trainset capacity (load factor). Leftover passengers
//! keep waiting and accumulate (the money-free slice's difficulty signal).
use crate::trainset::spec as trainset_spec;
use crate::world::World;

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

        let line = &lines[vehicles.line[i].index()];
        let cap = line
            .trainset
            .map(|t| trainset_spec(t.spec).capacity as usize)
            .unwrap_or(0);

        // Alight: drop onboard passengers whose destination is this stop.
        let before = vehicles.onboard_dest[i].len();
        vehicles.onboard_dest[i].retain(|&d| d.index() != s);
        let alighted = before - vehicles.onboard_dest[i].len();
        if s < alightings.len() {
            alightings[s] += alighted as u64;
        }

        // Board: FIFO up to capacity (leftover passengers keep waiting).
        while vehicles.onboard_dest[i].len() < cap {
            match waiting[s].pop_front() {
                Some(dest) => {
                    vehicles.onboard_dest[i].push(dest);
                    *ridership_total += 1;
                    if s < boardings.len() {
                        boardings[s] += 1;
                    }
                }
                None => break,
            }
        }
        vehicles.onboard[i] = vehicles.onboard_dest[i].len() as u16;
    }
}
