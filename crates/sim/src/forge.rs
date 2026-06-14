//! The Forge-Line: the fantasy supply chain's per-node commodity BUFFERS + the production tick phase
//! (fantasy-game-design.md §2, fantasy-build-plan.md S7). Inert for transit (`forge_stock` stays
//! empty; `GravityDemand`/`AgentDemand` have no `produce` step), live for arcadia. This is the first
//! HASHED fantasy state — `forge_stock` is folded into `Canonical`, so adding it RE-PINS the transit
//! golden once (binding condition #1) then leaves transit byte-identical.
//!
//! **S7a scope (this slice).** The buffer state + a steady source-production phase, integer-exact and
//! deterministic. A node that is a net SOURCE (captured origin weight > captured dest weight — it makes
//! more than it draws) accrues its raw commodity into its buffer, capped. The buffer→spawn gate (ship
//! only what you've produced), the deposit-at-sink, and the multi-stage 2-input recipes (Liebig:
//! output rate = min input rate) layer on next behind this same `Demand::produce` seam, NOT half-built
//! into it.
use crate::world::World;

/// The Forge-Line commodity set (fantasy-game-design.md §2): two disjoint chains.
/// Raw `0..4`, mid `4..6`, final `6..8`. The enum is FIXED so buffer indices are stable across saves
/// (the flat `forge_stock` layout is `station * N_COMMODITIES + commodity`, a Canonical byte order).
pub const N_COMMODITIES: usize = 8;
pub const ORE: usize = 0; // raw → INGOT → ARMS (the war chain)
#[allow(dead_code)]
pub const GRAIN: usize = 1; // raw → FLOUR → BREAD (the town chain)
#[allow(dead_code)]
pub const AETHER: usize = 2;
#[allow(dead_code)]
pub const FUEL: usize = 3;
/// First non-raw commodity index: `0..FIRST_MID` are raws (mined/grown at sources); `FIRST_MID..` are
/// PROCESSED goods (mids 4..6, finals 6..8) a PROCESSOR node makes from raws (S7e multi-stage). A station
/// whose output commodity (`station_commodity`) is ≥ this is a processor, not a raw source.
pub const FIRST_MID: usize = 4;

/// Per-node buffer capacity (units). The ONE non-derivable knob the build plan flags for playtest —
/// too small starves shipping, too large hides the throb. Externalised here so a balance sweep can
/// vary it without touching logic. S7a default; tuned once the headless harness lands (post-S6).
pub const BUFFER_CAP: i64 = 1_000;

/// Production accrual: micro-units of raw commodity per (source-weight-unit · ms). Integer fixed-point
/// (`forge_accum` holds the sub-unit remainder) so production is exact and deterministic — NO float in
/// the hashed result. A weight-50 source makes ~`50 * RATE_MICRO_PER_WEIGHT_MS` µ-units/ms. Tunable.
const RATE_MICRO_PER_WEIGHT_MS: i64 = 2;
const MICRO: i64 = 1_000_000;

/// The production phase (`SupplyChainDemand::produce`): each net-source node accrues its raw commodity
/// into its buffer, capped. Integer-exact (a per-node µ-unit remainder), index-ordered ⇒ deterministic.
/// Sizes the arcadia buffers lazily on first run (transit never calls this, so `forge_stock` stays
/// empty there). S7a produces ORE at sources; per-recipe production is S7b.
pub(crate) fn produce(world: &mut World, dt_ms: i64) {
    let n = world.stations.len();
    if n == 0 {
        return;
    }
    // Lazily size the arcadia buffers to the current node count (stations only grow; index-stable).
    if world.forge_stock.len() != n * N_COMMODITIES {
        world.forge_stock.resize(n * N_COMMODITIES, 0);
    }
    if world.forge_accum.len() != n {
        world.forge_accum.resize(n, 0);
    }
    let dt = dt_ms.max(0);
    // S11 production tech: PRODUCTION_SURGE ⇒ ×3, else FORGE_MASTERY ⇒ ×2, else ×1 (the shipped accrual,
    // byte-identical when no tech). Read once (copy out of the borrow).
    let prod_mult = if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::PRODUCTION_SURGE) {
        3
    } else if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::FORGE_MASTERY) {
        2
    } else {
        1
    };
    // Base rate is a per-city knob (0 ⇒ the slow default — golden-neutral; the baked world sets it snappy).
    let base_micro = if world.city.production_micro > 0 { world.city.production_micro } else { RATE_MICRO_PER_WEIGHT_MS };
    let rate = base_micro * prod_mult;
    // S11 AETHERIC_FONT: aether chains mint +50% MANA (×3/2). 0 ⇒ ×1, byte-identical.
    let mana_num = if crate::tech::is_unlocked(world.tech_unlocked, crate::tech::AETHERIC_FONT) { 3 } else { 2 };
    for s in 0..n {
        let comm = (world.station_commodity.get(s).copied().unwrap_or(0) as usize).min(N_COMMODITIES - 1);
        let base = s * N_COMMODITIES;
        // S7e MULTI-STAGE: a node whose output is a PROCESSED good (mid/final, comm ≥ FIRST_MID) is a
        // PROCESSOR — it CONVERTS its recipe inputs (the raws shipped in + deposited in its buffers) into
        // its output good by Liebig (min over inputs), capped by the output buffer's headroom. The output
        // then ships onward (it IS this node's `station_commodity`) to the next stage / the final sink.
        // Throughput is gated by the raw INFLOW (the upstream source's rate-limited production), so the
        // conversion needs no extra rate limit. A raw output (comm < FIRST_MID) is a plain mined source,
        // accrued below — so commodity-0 / raw-only worlds never enter this branch (byte-identical).
        if comm >= FIRST_MID {
            if let Some(recipe) = world.station_recipe.get(s).cloned() {
                // A processor never consumes its OWN output — exclude `comm` from the inputs, so a node
                // that happens to capture a little dest weight of the good it makes can't self-include it
                // in the Liebig min and deadlock (avail would be min(.., output=0) = 0). Robust to map
                // authoring; without this a forge cell tagged with its own output's dest would freeze.
                let inputs: Vec<usize> = recipe.iter().map(|&c| c as usize).filter(|&c| c != comm).collect();
                if !inputs.is_empty() {
                    let avail = inputs.iter().map(|&c| world.forge_stock[base + c]).min().unwrap_or(0);
                    let make = avail.min(BUFFER_CAP - world.forge_stock[base + comm]).max(0);
                    if make > 0 {
                        for &c in &inputs {
                            world.forge_stock[base + c] -= make;
                        }
                        world.forge_stock[base + comm] += make;
                    }
                }
            }
            continue;
        }
        let co = world.captured_origin.get(s).copied().unwrap_or(0.0);
        let cd = world.captured_dest.get(s).copied().unwrap_or(0.0);
        // A net SOURCE makes more than it draws. The f32→i64 cast happens ONCE per node per tick and
        // feeds only the integer accumulator (the hashed stock never reads a float directly).
        let net = (co - cd) as i64;
        if net <= 0 {
            continue;
        }
        let acc = &mut world.forge_accum[s];
        *acc = acc.saturating_add(net.saturating_mul(rate).saturating_mul(dt));
        let units = *acc / MICRO;
        if units <= 0 {
            continue;
        }
        *acc -= units * MICRO;
        // S7e: a source accrues ITS commodity (the dominant origin-commodity of its captured cells),
        // not always ORE — so a grain source makes GRAIN, an ore source ORE, etc. ORE for any station
        // without a per-commodity tag (commodity 0), so single-commodity worlds are unchanged.
        let slot = base + comm;
        world.forge_stock[slot] = (world.forge_stock[slot] + units).min(BUFFER_CAP);
    }

    // Town consumption (the Liebig consume, single-input S7d): a net-SINK node (a town: captured dest
    // weight > origin) consumes the commodity DELIVERED into its buffer → global TRIBUTE (the supply
    // score, the game's core payoff). A node is either a net source or a net sink, never both, so this
    // never double-counts production. Multi-input recipes (e.g. ORE+? → ARMS) generalise this consume.
    for s in 0..n {
        // A PROCESSOR (output ≥ FIRST_MID, S7e multi-stage) is a net-sink for its RAW inputs, but it
        // SHIPS its processed output onward — it must NOT consume into tribute here (the produce phase
        // above already converted its inputs → output). Only true final sinks consume into tribute.
        let comm = (world.station_commodity.get(s).copied().unwrap_or(0) as usize).min(N_COMMODITIES - 1);
        if comm >= FIRST_MID {
            continue;
        }
        let co = world.captured_origin.get(s).copied().unwrap_or(0.0);
        let cd = world.captured_dest.get(s).copied().unwrap_or(0.0);
        if cd <= co {
            continue; // only towns (net sinks) consume into tribute
        }
        let base = s * N_COMMODITIES;
        let recipe = world.station_recipe.get(s);
        let multi = recipe.map(|r| r.len() >= 2).unwrap_or(false);
        if multi {
            // S7e-2 LIEBIG: a sink with a real ≥2-commodity recipe (a BREAD town = grain+fuel, an ARMS
            // barracks = ingot+aether) yields output = MIN over its required inputs — the scarcer input
            // throttles, so you must supply BOTH chains. Consume `limit` of EACH required commodity;
            // non-recipe goods delivered here are left untouched (the sink doesn't want them).
            let r = recipe.unwrap();
            let limit = r.iter().map(|&c| world.forge_stock[base + c as usize]).min().unwrap_or(0);
            if limit > 0 {
                let mut mana = 0i64;
                let mut manpower = 0i64;
                for &c in r {
                    world.forge_stock[base + c as usize] -= limit;
                    // S11 ECONOMY SPLIT: each consumed input mints its specialised channel alongside gold.
                    match crate::tech::channel_of(c as usize) {
                        crate::tech::Channel::Mana => mana = mana.saturating_add(limit * mana_num / 2),
                        crate::tech::Channel::Manpower => manpower = manpower.saturating_add(limit),
                        crate::tech::Channel::Gold => {}
                    }
                }
                world.tribute = world.tribute.saturating_add(limit); // GOLD — once per consume, unchanged
                world.mana = world.mana.saturating_add(mana);
                world.manpower = world.manpower.saturating_add(manpower);
            }
        } else {
            // Single/empty recipe ⇒ consume-all (S7e-1). Commodity-0 worlds take this path ⇒ byte-identical
            // (ORE is a GOLD commodity ⇒ mana/manpower stay 0 ⇒ the golden re-pin is appended zeros only).
            let mut got = 0i64;
            let mut mana = 0i64;
            let mut manpower = 0i64;
            for c in 0..N_COMMODITIES {
                let amt = world.forge_stock[base + c];
                if amt > 0 {
                    got = got.saturating_add(amt);
                    match crate::tech::channel_of(c) {
                        crate::tech::Channel::Mana => mana = mana.saturating_add(amt * mana_num / 2),
                        crate::tech::Channel::Manpower => manpower = manpower.saturating_add(amt),
                        crate::tech::Channel::Gold => {}
                    }
                }
                world.forge_stock[base + c] = 0;
            }
            if got > 0 {
                world.tribute = world.tribute.saturating_add(got); // GOLD, unchanged
                world.mana = world.mana.saturating_add(mana);
                world.manpower = world.manpower.saturating_add(manpower);
            }
        }
    }
}
