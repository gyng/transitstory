//! The classic transit ruleset + its two demand models. **The carve is complete (S2):** `tick.rs`
//! and `apply` now reach demand + scoring ONLY through these impls (the box), and the
//! gravity-vs-agents `agent_demand` if/else has folded into `spawn` polymorphism. The impls DELEGATE
//! to the `demand::*` module, which is the gravity/agent implementation `GravityDemand`/`AgentDemand`
//! wrap — delegation (not a physical body move) keeps the `world.rng` draw order byte-identical by
//! construction, so the transit golden hash is unchanged. A fantasy `SupplyChainDemand` is a true
//! sibling `impl Demand` in its own module; nothing in `tick.rs`/`apply` hardcodes a demand model.
use super::{Demand, Ruleset};
use crate::world::World;

/// The shipped transit game: gravity/agent demand, the monotonic coverage gauge, every existing
/// Command valid.
pub struct TransitRuleset;

impl Ruleset for TransitRuleset {
    fn coverage_score(&self, world: &World) -> u8 {
        world.coverage_score()
    }

    fn validate(&self, cmd: &crate::command::Command) -> Result<(), String> {
        // The disjoint-save guard's first real teeth (S8): a fantasy-only command must be refused in
        // the transit game — before it mutates or joins the save — so a transit save can never contain
        // a `PlaceBarracks` (which would replay against the wrong `apply` in a fantasy world).
        use crate::command::Command;
        match cmd {
            Command::PlaceBarracks { .. }
            | Command::PostBounty { .. }
            | Command::UnlockTech { .. }
            | Command::CastSpell { .. }
            | Command::SetAutocast { .. } => {
                Err("that is a fantasy (arcadia) command, not valid in the transit game".into())
            }
            _ => Ok(()),
        }
    }
}

/// Gravity flow — the DEFAULT demand model: catchment-captured cell weights spawn trips routed by
/// the `Router` seam.
pub struct GravityDemand;

impl Demand for GravityDemand {
    fn prepare(&mut self, world: &mut World) {
        crate::demand::prepare(world);
    }
    fn grow(&mut self, world: &mut World) {
        crate::demand::grow(world);
    }
    fn spawn(&mut self, world: &mut World, dt_ms: i64) {
        crate::demand::spawn(world, dt_ms);
    }
}

/// Seed-derived citizen agents (home/work on a schedule), swapped in by `SetDemandMode{agents:true}`.
/// At S2 the population moves OUT of `world.population` and INTO this box; for now it mirrors
/// `tick.rs`'s take-out dance so the impl is correct if called.
pub struct AgentDemand;

impl Demand for AgentDemand {
    fn prepare(&mut self, world: &mut World) {
        crate::demand::prepare(world);
    }
    fn grow(&mut self, world: &mut World) {
        crate::demand::grow(world);
    }
    fn spawn(&mut self, world: &mut World, dt_ms: i64) {
        // Take the population out to avoid aliasing &mut World, spawn, then put it back (the exact
        // dance `tick.rs` does today).
        if let Some(mut pop) = world.population.take() {
            pop.spawn_trips(world, dt_ms);
            world.population = Some(pop);
        }
    }
}
