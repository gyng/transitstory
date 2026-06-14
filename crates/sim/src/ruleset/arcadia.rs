//! The fantasy 4X-logistics ruleset ("Against the Dark" / Arcadia) + its supply-chain demand model —
//! the SECOND mode, attaching as sibling impls of the same `Ruleset`/`Demand` seam transit uses
//! (fantasy-fork.md, fantasy-game-design.md). Selected by `CityData.ruleset == "arcadia"` at
//! construction; the engine (RaptorRouter + VehicleSoA::advance + board_alight + the hex grid lattice)
//! is reused UNCHANGED — a commodity cart is a vehicle, a node is a station, a commodity token is a
//! `Pax`. This is the whole point of the ruleset-at-construction fork: a new game with **zero** change
//! to `World::apply`'s signature or the movement core.
//!
//! **Slice scope (S6a).** This first cut proves the fork lights up end-to-end on the hex lattice,
//! deterministically, reusing the substrate. `SupplyChainDemand` therefore reuses the proven
//! catchment+spawn machinery (commodity flow rides the same routed-`Pax` path source→sink). The
//! supply-chain-SPECIFIC behaviour layers on behind this same seam, NOT half-built into it:
//!   * S6b — the baked `arcadia_world.json` + frontend load (the first VISIBLE checkpoint).
//!   * S7  — commodity ids in the unhashed `Pax.citizen_id`, the ≤8-commodity Forge-Line recipes,
//!           per-input i64 buffers (new hashed Canonical), the Liebig consume→fire→push tick phase.
//!   * S11 — the split supply/war coverage gauge (each channel its own monotonicity invariant).
//! Every one of those is a future edit to THIS file / new hashed fields, never a change to `tick.rs`'s
//! phase order or the movement core.
use super::{Demand, Ruleset};
use crate::command::Command;
use crate::world::World;

/// The fantasy game mode. For S6a its scoring reuses the transit coverage gauge (a stand-in until the
/// S11 split supply/war gauge) and it accepts every command (the cross-mode `validate` teeth engage in
/// S7, once fantasy-specific commands exist to reject in a transit save and vice-versa).
pub struct ArcadiaRuleset;

impl Ruleset for ArcadiaRuleset {
    fn coverage_score(&self, world: &World) -> u8 {
        // Placeholder: the served-demand fraction, same shape as transit. S11 replaces this with the
        // split gauge (supply-served % blended with the decadence-front distance, each monotonic).
        world.coverage_score()
    }

    fn validate(&self, _cmd: &Command) -> Result<(), String> {
        // S7: reject transit-only build commands here (and PlaceNode/BuildRoute in TransitRuleset), so
        // a cross-mode command is refused before it mutates or joins the save. Until the fantasy
        // command vocab exists, every command is the shared build/run set ⇒ accept all.
        Ok(())
    }

    fn war_step(&self, world: &mut World, dt_ms: i64) {
        // The war trailer (locked order): launch (tribute-funded) → march → siege grind→flip. Each
        // sub-phase is integer + index-ordered ⇒ deterministic. Supply-gated siege + bounties + the
        // army↔train single-track admission (occ_claim) are the next refinements.
        crate::army::maybe_launch(world);
        crate::army::advance_armies(world, dt_ms);
        crate::army::siege(world);
        // Decadence (S9): the global lose-meter pressure — pushed back by conquest.
        crate::decadence::step(world, dt_ms);
        // Decadence (S10b): the SPATIAL tide — the per-cell creep CA over the baked board (no-op until a
        // baked world supplies terrain). Runs PARALLEL to the scalar meter; the lose-condition rewire is
        // S10b-2. Pushed back spatially by the player's rail network (PURGE).
        crate::decadence_field::step(world, dt_ms);
    }
}

/// The supply-chain demand model: commodity tokens born at SOURCE nodes (high origin weight) flow to
/// SINK nodes / towns (high dest weight). Unlike transit gravity, logistics flow at a STEADY rate with
/// a fixed source→sink direction — there is no day/night commuter rush. So `spawn` calls the shared
/// `spawn_modulated` body with steady `(mult=1.0, bias=1.0)` instead of the time-of-day values, reusing
/// `RaptorRouter`/`advance`/`board_alight` and the routing caches unchanged. S7 layers the
/// commodity-id tagging, per-recipe rates, and per-input buffer (Liebig) limiting on top.
pub struct SupplyChainDemand;

impl Demand for SupplyChainDemand {
    fn prepare(&mut self, world: &mut World) {
        crate::demand::prepare(world);
    }
    fn grow(&mut self, world: &mut World) {
        crate::demand::grow(world);
    }
    fn produce(&mut self, world: &mut World, dt_ms: i64) {
        // The Forge-Line production phase: sources accrue raw commodity into their buffers (S7a). The
        // buffer→spawn gate, sink deposit, and 2-input recipes layer on next behind this seam.
        crate::forge::produce(world, dt_ms);
    }
    fn spawn(&mut self, world: &mut World, dt_ms: i64) {
        use crate::forge::N_COMMODITIES;
        // Steady source→sink commodity flow (mult=1.0 ⇒ constant volume; bias=1.0 ⇒ spawn at sources,
        // route to sinks), GATED by production (S7b/S7e): a node ships only what it has PRODUCED — and a
        // source ships ITS commodity (the one `produce` accrues = `station_commodity`), so the gate reads
        // each node's own commodity buffer, ships against it, writes the drained buffer back. `produce`
        // ran earlier this tick, so the budget reflects post-production stock.
        let n = world.stations.len();
        // Snapshot each node's output commodity BEFORE the mutable spawn call (no borrow of `world` held
        // across it). A source ships its own commodity, so the gate reads that slot.
        let comms: Vec<usize> =
            (0..n).map(|s| world.station_commodity.get(s).copied().unwrap_or(0) as usize % N_COMMODITIES).collect();
        let mut budget: Vec<i64> =
            (0..n).map(|s| world.forge_stock.get(s * N_COMMODITIES + comms[s]).copied().unwrap_or(0)).collect();
        crate::demand::spawn_modulated(world, dt_ms, 1.0, 1.0, Some(&mut budget));
        for s in 0..n {
            if let Some(slot) = world.forge_stock.get_mut(s * N_COMMODITIES + comms[s]) {
                *slot = budget[s];
            }
        }
    }
}
