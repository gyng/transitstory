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
        // Phase 2 — once per in-game day the city grows; then recompute capture if anything
        // changed; then spawn+route this tick's trips. All three run through the demand SEAM
        // (fantasy-fork.md): take the box out so it can borrow `&mut World` without aliasing the
        // field it lives in, run the model, then put it back (`NoopDemand` is a transient ZST
        // placeholder, never observed). The gravity-vs-agents split now lives in the box
        // (`GravityDemand` vs `AgentDemand::spawn`) — NOT an `agent_demand` if/else here. The exact
        // grow→prepare→spawn order (and each model's internal `world.rng` draw order) is preserved.
        let mut demand = std::mem::replace(&mut world.demand, Box::new(crate::ruleset::NoopDemand));
        demand.grow(world);
        demand.prepare(world);
        demand.produce(world, dt); // fantasy production (Forge-Line); no-op for transit
        demand.spawn(world, dt);
        world.demand = demand;
        // Phase 4 — move trains along the line (records station arrivals).
        crate::vehicle::advance(world, dt);
        // Phase 5 — alight then board (capacity-capped).
        crate::pax::board_alight(world);
        // Phase 5b — riders who have waited past the city's patience give up (renege).
        crate::pax::renege(world);
        // Phase 6 — accounting: charge recurring maintenance (opex) when the economy is on.
        world.tick_economy(dt);
        //          The rest of stats is computed on demand in stats_snapshot().

        // Phase 7 — the fantasy war trailer (S8): accrue→launch→march→(S8b grind→flip). Through the
        // ruleset SEAM via a take-out swap (so it can borrow `&mut World`); `NoopRuleset` is a transient
        // placeholder. No-op for transit (the army SoA stays empty), so transit is byte-identical.
        let ruleset = std::mem::replace(&mut world.ruleset, Box::new(crate::ruleset::NoopRuleset));
        ruleset.war_step(world, dt);
        world.ruleset = ruleset;
    }
}
