//! Ruleset seam (fantasy-fork.md): the port that selects WHICH GAME the engine constructs. Chosen
//! once from `CityData.ruleset` at `World::new` and FROZEN (never a Command), exactly mirroring the
//! proven `Router` seam (`routing/mod.rs`). Two boxes live on `World` beside `router`:
//!   * `Box<dyn Ruleset>` — the mode: scoring gauge, command validity, (fantasy) the per-tick trailer.
//!   * `Box<dyn Demand>`  — how trips/commodities are born: gravity vs agents vs (S6) supply-chain.
//!
//! `TransitRuleset` + `GravityDemand`/`AgentDemand` ship today; `ArcadiaRuleset` + `SupplyChainDemand`
//! attach as sibling impls (S6+) with ZERO change to `World::apply`'s signature — the whole point of
//! the seam. Until S2 (THE CARVE) the transit logic still lives in `demand.rs`/`world.rs` and these
//! impls merely DELEGATE to it; the boxes are constructed but `tick.rs`/`apply` do not call them yet,
//! so the transit golden hash is byte-identical (the boxes are inert).
//!
//! DETERMINISM CONTRACT (identical to `routing/mod.rs`): every method MUST be deterministic —
//! index-ordered iteration only (NO `std::HashMap`/`HashSet` iteration), `i64` ms/mm, and any RNG use
//! draws from `world.rng` in a FIXED order. The replay-equality + golden-hash gates depend on it.
use crate::command::Command;
use crate::world::World;

mod arcadia;
mod transit;
pub use arcadia::{ArcadiaRuleset, SupplyChainDemand};
pub use transit::{AgentDemand, GravityDemand, TransitRuleset};

/// How trips (transit) or commodity tokens (fantasy) are born each tick. The `agent_demand` if/else
/// in `tick.rs` folds into this at S2: `GravityDemand` vs `AgentDemand` vs (S6) `SupplyChainDemand`.
/// Methods take `&mut self` so a model can own per-tick state (S6's supply-chain buffers, the agent
/// population) — the caller uses a take-out swap to satisfy the borrow checker (`tick.rs` already
/// does this for `world.population`).
pub trait Demand {
    /// Recompute capture/eligibility before spawning (transit: catchment capture). Deterministic.
    fn prepare(&mut self, world: &mut World);
    /// Periodic growth pass (transit: daily transit-oriented demand growth). Deterministic.
    fn grow(&mut self, world: &mut World);
    /// Production phase (fantasy: the Forge-Line consume→fire→push — sources accrue, forges convert).
    /// Runs each tick BEFORE `spawn` (so a node ships what it has produced). Default no-op: transit has
    /// no production, so `GravityDemand`/`AgentDemand` skip it and `forge_stock` stays empty for them.
    /// Deterministic; index-ordered; integer-exact accrual.
    fn produce(&mut self, _world: &mut World, _dt_ms: i64) {}
    /// Emit this tick's trips/commodities. MUST draw from `world.rng` in a FIXED order (the carve's
    /// load-bearing constraint — `demand.rs` destructures `ref mut rng` in lockstep).
    fn spawn(&mut self, world: &mut World, dt_ms: i64);
}

/// A neutral no-op `Demand` installed TRANSIENTLY by the take-out swap in `tick.rs`/`apply` while
/// the real box is borrowed out to call its `&mut World` methods (a `Demand` method can't take
/// `&mut self` and `&mut world` when the box IS a field of `world`). Never observed by a running
/// model and never persisted — the real box is restored before the swap returns. Boxing this ZST
/// does not allocate, so the swap is free.
pub(crate) struct NoopDemand;

impl Demand for NoopDemand {
    fn prepare(&mut self, _world: &mut World) {}
    fn grow(&mut self, _world: &mut World) {}
    fn spawn(&mut self, _world: &mut World, _dt_ms: i64) {}
}

/// The game mode: scoring gauge, command validity, and (fantasy, S8+) the per-tick war/economy
/// trailer. Selected from `CityData.ruleset`, frozen at construction.
pub trait Ruleset {
    /// The 0–100 progress gauge for this mode (transit: monotonic coverage; fantasy: a split gauge,
    /// each channel with its own monotonicity invariant).
    fn coverage_score(&self, world: &World) -> u8;
    /// The fantasy per-tick war/economy trailer (S8): accrue→launch→retarget→move→grind→flip. Runs
    /// after the demand/movement phases each tick while running. Default no-op — transit has no war, so
    /// `TransitRuleset` skips it and the army SoA stays empty (the call is golden-neutral; only the
    /// hashed army FIELDS shift the transit pin). Deterministic; keyed RNG (`seed ^ WAR_CONST`) only.
    fn war_step(&self, _world: &mut World, _dt_ms: i64) {}

    /// Reject a `Command` not meaningful in this mode (S3 disjoint-save guard) — runs at the TOP of
    /// `World::apply`, BEFORE any mutation or the save-log push, so a rejected cross-mode command
    /// neither mutates state nor pollutes a save. Default: accept all (transit accepts every existing
    /// Command). Must be a PURE function of `cmd` (no RNG, no state read that could diverge).
    fn validate(&self, _cmd: &Command) -> Result<(), String> {
        Ok(())
    }
}

/// A neutral no-op `Ruleset` installed TRANSIENTLY by the take-out swap in `tick.rs` while the real
/// box is borrowed out to call `war_step(&mut world)` (a `&self` method can't be called on
/// `world.ruleset` while also passing `&mut world` — the field would be aliased). Never observed; the
/// real box is restored before the swap returns. `coverage_score` is never called on it.
pub(crate) struct NoopRuleset;

impl Ruleset for NoopRuleset {
    fn coverage_score(&self, _world: &World) -> u8 {
        0
    }
}

/// Canonical ruleset tag: the empty string (the `CityData::default()` value native tests use) means
/// the transit game, identical to the explicit `"transit"` a JSON city carries. Collapsing the two
/// spellings here lets tests and cities name the same mode, and makes the disjoint-save guard compare
/// MODES, not spellings (`"" ` vs `"transit"` must not falsely trip it).
pub fn canon(tag: &str) -> &str {
    if tag.is_empty() {
        "transit"
    } else {
        tag
    }
}

/// The mode dispatch, in ONE place: construct the `(ruleset, demand)` boxes for a city's frozen
/// ruleset tag. Called by `World::new`. Unknown tags fall back to transit — no other ruleset ships
/// yet; `"arcadia"` wires in here at S6 as a single new arm (the only edit needed to light up the
/// fantasy mode). Determinism-neutral: neither box is hashed, and selection is a pure function of the
/// construction-time tag.
pub(crate) fn select(tag: &str) -> (Box<dyn Ruleset>, Box<dyn Demand>) {
    match canon(tag) {
        "arcadia" => (Box::new(ArcadiaRuleset), Box::new(SupplyChainDemand)),
        _ => (Box::new(TransitRuleset), Box::new(GravityDemand)),
    }
}
