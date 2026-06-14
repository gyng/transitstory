//! Decadence — the spreading corruption that is the game's LOSE condition (fantasy-game-design.md).
//! Left unchecked it overruns the realm (reaches the capital ⇒ you lose); your conquest pushes it
//! back. This is the pressure that makes the supply→conquest flywheel *urgent*: expand faster than the
//! rot, or be consumed.
//!
//! **S9 scope (this slice): a global corruption PRESSURE + the lose threshold.** Integer, deterministic.
//! It captures the gameplay tension (race conquest vs corruption) abstractly. The SPATIAL frontier — a
//! hex contested-cell field creeping from the map edges toward the anchored capital corner, plus
//! decadence RAIDERS that walk `roadnav` and sever supply edges — is S10 (the area-control CA, the
//! largest subsystem). That will make `decadence` here a derived summary of / be replaced by the field;
//! the lose condition + pushback semantics stay. Fantasy-only: transit never runs `war_step` ⇒
//! `decadence` stays 0, byte-identical (only the appended hashed field re-pins the transit golden once).
use crate::world::World;

/// Corruption growth per sim-second when nothing holds it back. A balance knob (externalised to
/// `CityData` for the sweep later); S9 default.
const BASE_GROWTH_PER_S: i64 = 50;
/// How hard each captured town pushes the rot back, per sim-second. Conquest is the brake.
const CLEAR_PER_TOWN_PER_S: i64 = 300;
/// Decadence at/above this has reached the capital — the realm falls. Knob; S9 default sized so an
/// idle realm is overrun in a few game-minutes — slower than a modest supply→conquest chain can field
/// and march a legion — so conquest is a viable brake (the game is winnable). Balance-swept later.
const CAPITAL_THRESHOLD: i64 = 20_000;

/// The decadence sub-phase of `war_step`: corruption spreads at `BASE_GROWTH`, pushed back by held
/// (captured) towns. Net rate can go negative — a conquering realm claws ground back — clamped at 0
/// (you can't bank surplus). Integer + dt-scaled ⇒ deterministic.
pub(crate) fn step(world: &mut World, dt_ms: i64) {
    let dt = dt_ms.max(0);
    // Growth is a per-CITY knob (externalised so a large baked continent — slow two-chain tribute, long
    // supply lines — can press far gentler than the tiny demo). 0 ⇒ the `BASE_GROWTH_PER_S` default, so
    // every existing city / golden fixture / native test is byte-identical (the small-demo balance).
    let growth = if world.city.decadence_growth_per_s > 0 {
        world.city.decadence_growth_per_s
    } else {
        BASE_GROWTH_PER_S
    };
    let pushback = world.towns_captured.saturating_mul(CLEAR_PER_TOWN_PER_S);
    let net = growth - pushback; // /sec; negative when conquest outpaces the rot
    // Integer fixed-point (mirrors `forge::produce`): accumulate `net·dt` milli-units and extract whole
    // units, keeping the sub-unit remainder so a gentle rate (< 20/s ⇒ < 1 unit per 50 ms tick) accrues
    // EXACTLY instead of truncating to 0 — the bug that froze the baked continent's 6/s lose meter. The
    // demo's 50/s now accrues at a true 50/s (was 40/s under the old `/1000` truncation), so the arcadia
    // golden re-pins; transit never runs `war_step`, so `net`/`decadence` stay 0 and the accumulator is
    // excluded from `Canonical` ⇒ the transit golden is untouched. `div_euclid` floors toward −∞ so a
    // negative `net` (conquest outpacing the rot) drains exactly and the remainder stays in [0, 1000).
    world.decadence_accum = world.decadence_accum.saturating_add(net.saturating_mul(dt));
    let units = world.decadence_accum.div_euclid(1000);
    world.decadence_accum -= units * 1000; // = rem_euclid(1000) ∈ [0, 1000) — bounded, exact
    world.decadence = (world.decadence + units).max(0); // clamp ≥ 0: you can't bank surplus pushback
}

/// True once the corruption has reached the capital — the realm has fallen. A pure read; the GameLoop
/// / frontend surfaces it as the loss state. (S10 makes this "the decadence field touched the capital
/// cell"; the threshold semantics are the same.)
pub fn is_lost(world: &World) -> bool {
    world.decadence >= CAPITAL_THRESHOLD
}

/// Decadence as a 0–100 fraction of the capital threshold — the lose-meter gauge fill (100 = fallen).
/// The HUD renders this directly so the threshold constant never has to be mirrored on the frontend.
pub fn pct(world: &World) -> f64 {
    (world.decadence as f64 / CAPITAL_THRESHOLD as f64 * 100.0).clamp(0.0, 100.0)
}
