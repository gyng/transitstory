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
    // Phase 1 — advance the integer clock.
    world.clock_ms = world.clock_ms.saturating_add(dt_ms.max(0));

    // Phases 2-6 are implemented in T14/T16. They run unconditionally here; the caller
    // (GameLoop) decides when to tick. Kept as explicit no-ops so the ordering is visible.
    // crate::demand::spawn(world, dt_ms);
    // crate::dispatch::dispatch(world, dt_ms);
    // crate::vehicle::advance(world, dt_ms);
    // crate::pax::board_alight(world);
    // crate::stats::accumulate(world);
}
