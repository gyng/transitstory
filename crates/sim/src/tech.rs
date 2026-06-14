//! S11 — the tech tree: a small fixed set of UPGRADES the realm buys with TRIBUTE (the same war-chest
//! that funds legions, `army::maybe_launch`). Each tech is one bit in `World.tech_unlocked` (a hashed
//! bitset); unlocking is a `Command::UnlockTech` that spends tribute once and flips the bit permanently.
//! Each unlocked bit gates a buff to an EXISTING fantasy lever — production (forge), war (legion cost),
//! or defence (decadence creep) — so it sharpens the loop without adding a new system.
//!
//! **Golden-neutral effects.** Every effect READS its bit and falls back to the shipped constant when
//! unset, so transit (no `UnlockTech` command — the transit ruleset rejects it ⇒ `tech_unlocked` stays
//! 0) and the arcadia golden (its fixed log predates tech) are behaviourally byte-identical. Only the
//! appended `tech_unlocked` field in `Canonical` shifts the hash — a one-time re-pin, behaviour unchanged.
//!
//! The bitset is `u32` (room for 32 techs); the three below are S11's slice. Costs are tribute units
//! (a legion is `army::LAUNCH_COST` = 8), so a tech trades against fielding legions — the spend decision
//! IS the depth. Tunable; externalise to `CityData` if a balance sweep wants per-city tech costs.

/// A tech: the bit it sets + its tribute cost. The id (its index in [`TECHS`]) is what the Command
/// carries — stable across saves, so the command log replays identically.
#[derive(Clone, Copy, Debug)]
pub struct Tech {
    /// The `tech_unlocked` bit this tech sets (1 << bit). Stable — never renumber a shipped tech.
    pub bit: u8,
    /// Tribute spent to unlock (deducted from `world.tribute`, the same pool legions draw from).
    pub cost: i64,
}

/// FORGE MASTERY (id 0): raw production rate ×2 — every source fills its buffer twice as fast, so the
/// whole supply chain (and the tribute it feeds) accelerates. The flywheel tech.
pub const FORGE_MASTERY: usize = 0;
/// CONSCRIPTION (id 1): legions cost HALF the tribute to field (`army::maybe_launch`) — more armies per
/// unit of supply, so conquest outpaces a faster rot. The war tech.
pub const CONSCRIPTION: usize = 1;
/// SAPPERS (id 2): the decadence tide creeps at HALF rate (`decadence_field::step`) — buys runway against
/// the lose condition. The defence tech.
pub const SAPPERS: usize = 2;

/// The shipped tech table. Index = the id a `Command::UnlockTech { tech }` carries. Order is FIXED
/// (a save records ids); append new techs, never reorder.
pub const TECHS: [Tech; 3] = [
    Tech { bit: 0, cost: 24 }, // FORGE_MASTERY
    Tech { bit: 1, cost: 40 }, // CONSCRIPTION
    Tech { bit: 2, cost: 56 }, // SAPPERS
];

/// True iff the tech at `id` (an index into [`TECHS`]) is unlocked in this bitset. Unknown ids → false
/// (so an effect reading a not-yet-shipped id is simply inert). The single read every effect uses.
#[inline]
pub fn is_unlocked(tech_unlocked: u32, id: usize) -> bool {
    TECHS.get(id).map(|t| tech_unlocked & (1u32 << t.bit) != 0).unwrap_or(false)
}
