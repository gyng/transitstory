//! The determinism heart: ONE strict ordered phase pass per integer tick. The order is
//! fixed and documented; iteration is index-ordered over Vec/slab only (no HashMap
//! iteration), time is i64 ms. Phases land incrementally:
//!   1. advance clock            (T2, here)
//!   2. spawn + route passengers (T16a)
//!   3. dispatch trains          (T14)
//!   4. move trains              (T14)
//!   5. alight then board        (T16b)
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
        // Phase 2 — spawn + route passengers (T16a)
        // crate::demand::spawn(world, dt);
        // Phase 4 — move trains along the line.
        crate::vehicle::advance(world, dt);
        // Phase 5 — alight then board (T16b)
        // Phase 6 — accounting / stats (T16b)
    }
}
