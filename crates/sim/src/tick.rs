//! The determinism heart: ONE strict ordered phase pass per integer tick. The order is
//! fixed and documented; iteration is index-ordered over Vec/slab only (no HashMap
//! iteration), time is i64 ms. Phases land incrementally:
//!   1. advance clock            (T2, here)
//!   2. spawn + route passengers (T16a)
//!   3. dispatch trains          (T14)
//!   4. move trains              (T14)
//!   5. alight then board        (T16b)
//!   5b. renege (give up waiting) (patience)
//!   6. accounting / stats       (T16b)
//! `tick` must be total and infallible — no panics, clamp/saturate instead. The GameLoop
//! only calls this while running (Build mode does not tick).
use crate::world::World;

pub(crate) fn step(world: &mut World, dt_ms: i64) {
    let dt = dt_ms.max(0);
    // Phase 1 — advance the integer clock.
    world.clock_ms = world.clock_ms.saturating_add(dt);

    // Phase 3 — (re)dispatch vehicles if the network changed (also when Run starts).
    crate::dispatch::dispatch(world);

    // Dynamics only run while Running (Build mode is paused).
    if world.running {
        // Phase 2 — recompute catchment capture if stations changed, then spawn+route pax.
        // Trips come from gravity flow, OR (opt-in) a seed-derived citizen population.
        crate::demand::prepare(world);
        if world.agent_demand {
            // Take the population out to avoid aliasing &mut World, spawn, then put it back.
            if let Some(mut pop) = world.population.take() {
                pop.spawn_trips(world, dt);
                world.population = Some(pop);
            }
        } else {
            crate::demand::spawn(world, dt);
        }
        // Phase 4 — move trains along the line (records station arrivals).
        crate::vehicle::advance(world, dt);
        // Phase 5 — alight then board (capacity-capped).
        crate::pax::board_alight(world);
        // Phase 5b — riders who have waited past the city's patience give up (renege).
        crate::pax::renege(world);
        // Phase 6 — accounting: charge recurring maintenance (opex) when the economy is on.
        world.tick_economy(dt);
        //          The rest of stats is computed on demand in stats_snapshot().
    }
}
