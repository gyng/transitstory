//! S11 — the tech tree + the ECONOMY (three channels minted by WHICH commodity a town consumes). The
//! channels' JOBS (owner-locked 2026-06-15):
//!   * GOLD ⚜ (mundane: ore/grain/fuel) — the player's economy: bounties + building.
//!   * MANPOWER ⚔ (arms: ingot) — the AI's legions.
//!   * MANA ✦ (aether) — the WHOLE tech+magic arm: **mana is the sole TECH resource** AND fuels the
//!     auto-cast SPELLS. Tech and spells draw the SAME mana pool, so banking for a permanent upgrade vs.
//!     letting spells fire is a live tradeoff. Aether is your "science": no aether, no tech.
//!
//! A tech is one bit in `World.tech_unlocked` (a hashed bitset); `Command::UnlockTech` spends MANA once,
//! checks the tech's PREREQ (a tier gate), and flips the bit permanently — gating a buff to an EXISTING
//! lever (production / war / defence) or a build capability (heavy rail) or the spell arm.
//!
//! **Golden-neutral effects.** Every effect READS its bit and falls back to the shipped constant when
//! unset, so transit (the ruleset rejects `UnlockTech` ⇒ `tech_unlocked` stays 0) and the arcadia golden
//! (its log predates tech) are behaviourally byte-identical — adding techs is just more bits in the
//! existing u32 (no re-pin).
use crate::world::World;

/// The three economy channels (S11 split). Gold is the universal tribute; mana + manpower are the
/// specialised yields. A `Channel` reads/spends its own balance on `World`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Channel {
    Gold,
    Mana,
    Manpower,
}

impl Channel {
    /// This channel's current balance on the world.
    #[inline]
    pub fn balance(self, world: &World) -> i64 {
        match self {
            Channel::Gold => world.tribute,
            Channel::Mana => world.mana,
            Channel::Manpower => world.manpower,
        }
    }
    /// Deduct `amount` from this channel (the afford-gate has already checked balance ≥ amount).
    #[inline]
    pub fn spend(self, world: &mut World, amount: i64) {
        match self {
            Channel::Gold => world.tribute = world.tribute.saturating_sub(amount),
            Channel::Mana => world.mana = world.mana.saturating_sub(amount),
            Channel::Manpower => world.manpower = world.manpower.saturating_sub(amount),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Channel::Gold => "gold",
            Channel::Mana => "mana",
            Channel::Manpower => "manpower",
        }
    }
}

/// Which channel a DELIVERED commodity mints (alongside the universal GOLD base). The roles, tuned so the
/// EARLY 2-stage BREAD town (grain+fuel) feeds ALL THREE channels — every channel is reachable without the
/// hard 3-stage forge chain:
///   AETHER + FUEL → MANA (the tech+magic arm — fuel is arcane fuel; aether the pure source).
///   GRAIN (food → soldiers) + INGOT/ARMS (war materiel, ≥ `forge::FIRST_MID`) → MANPOWER (legions).
///   ORE → GOLD (trade) — and the additive base mints gold on every delivery regardless.
#[inline]
pub fn channel_of(commodity: usize) -> Channel {
    if commodity == crate::forge::AETHER || commodity == crate::forge::FUEL {
        Channel::Mana
    } else if commodity == crate::forge::GRAIN || commodity >= crate::forge::FIRST_MID {
        Channel::Manpower
    } else {
        Channel::Gold
    }
}

/// A tech: the bit it sets, its MANA cost, and its PREREQ (an earlier tech id that must be unlocked first,
/// or -1 for a tier-1 root). All techs cost MANA (mana is the sole tech resource). The id (its index in
/// [`TECHS`]) is what the Command carries — stable across saves, so APPEND only, never reorder.
#[derive(Clone, Copy, Debug)]
pub struct Tech {
    pub bit: u8,
    pub cost: i64,
    pub prereq: i32, // -1 = tier-1 root; else the tech id required first
}

// ── Tier-1 roots (the three spines) ──
/// FORGE MASTERY (id 0): raw production ×2 — the economy spine's flywheel.
pub const FORGE_MASTERY: usize = 0;
/// CONSCRIPTION (id 1): legions cost HALF — the war spine.
pub const CONSCRIPTION: usize = 1;
/// SAPPERS (id 2): the decadence tide creeps at HALF rate — the defence/magic spine.
pub const SAPPERS: usize = 2;
// ── Tier-2 (branches; each needs its spine) ──
/// PRODUCTION SURGE (id 3, ← FORGE_MASTERY): raw production ×3 total — the greedy flywheel.
pub const PRODUCTION_SURGE: usize = 3;
/// BOUNTY MASTERY (id 4, ← CONSCRIPTION): a bountied town pulls legions ×2 and is ground +50% faster.
pub const BOUNTY_MASTERY: usize = 4;
/// HEAVY RAIL (id 5, ← FORGE_MASTERY): unlocks the heavy-rail build mode in arcadia.
pub const HEAVY_RAIL: usize = 5;
/// SIEGE DOCTRINE (id 6, ← CONSCRIPTION): besieging legions grind +50%.
pub const SIEGE_DOCTRINE: usize = 6;
/// STANDING GARRISON (id 7, ← CONSCRIPTION): captured towns cut down raiders (conquest → defence).
pub const STANDING_GARRISON: usize = 7;
/// WAR MARCH (id 8, ← CONSCRIPTION): legions march +50% faster.
pub const WAR_MARCH: usize = 8;
/// AETHERIC FONT (id 9, ← SAPPERS): aether mints +50% mana — the tech/spell economy multiplier.
pub const AETHERIC_FONT: usize = 9;
/// WARD LINES (id 10, ← SAPPERS): arcane wards on the rails cut raiders down from +50% range
/// (`raider::DEFENSE_RANGE` ×3/2). A SAFE defence buff — deliberately NOT a passive purge-radius bump,
/// which at radius ≥ LOSE_DIST would make a lone capital unloseable (the active Purge spell handles the tide).
pub const WARD_LINES: usize = 10;
/// ARCANE AWAKENING (id 11, ← SAPPERS): unlocks the SPELL ARM (`spell::step`) — mana auto-cast Purge/Smite/
/// Warpath at the biggest threat. The mana apex; spells drain the same pool as tech.
pub const SPELLCRAFT: usize = 11;

/// The shipped tech table. Index = the id a `Command::UnlockTech { tech }` carries. APPEND only.
// Costs are MANA, tuned against the baked world's economy (playtest-calibrated): a tier-1 spine ~early,
// the spell arm a mid-game spike. Tunable via the harness/playtest.
pub const TECHS: [Tech; 12] = [
    Tech { bit: 0, cost: 18, prereq: -1 }, // FORGE_MASTERY
    Tech { bit: 1, cost: 18, prereq: -1 }, // CONSCRIPTION
    Tech { bit: 2, cost: 16, prereq: -1 }, // SAPPERS (cheapest spine — the survival pick)
    Tech { bit: 3, cost: 30, prereq: FORGE_MASTERY as i32 }, // PRODUCTION_SURGE
    Tech { bit: 4, cost: 28, prereq: CONSCRIPTION as i32 }, // BOUNTY_MASTERY
    Tech { bit: 5, cost: 36, prereq: FORGE_MASTERY as i32 }, // HEAVY_RAIL
    Tech { bit: 6, cost: 32, prereq: CONSCRIPTION as i32 }, // SIEGE_DOCTRINE
    Tech { bit: 7, cost: 30, prereq: CONSCRIPTION as i32 }, // STANDING_GARRISON
    Tech { bit: 8, cost: 30, prereq: CONSCRIPTION as i32 }, // WAR_MARCH
    Tech { bit: 9, cost: 24, prereq: SAPPERS as i32 }, // AETHERIC_FONT (boots the mana economy → cheaper)
    Tech { bit: 10, cost: 28, prereq: SAPPERS as i32 }, // WARD_LINES
    Tech { bit: 11, cost: 40, prereq: SAPPERS as i32 }, // SPELLCRAFT (the spell arm — the mana apex)
];

/// The channel every tech is paid in — MANA (the sole tech resource).
pub const TECH_CHANNEL: Channel = Channel::Mana;

/// True iff the tech at `id` (an index into [`TECHS`]) is unlocked in this bitset. Unknown ids → false.
#[inline]
pub fn is_unlocked(tech_unlocked: u32, id: usize) -> bool {
    TECHS.get(id).map(|t| tech_unlocked & (1u32 << t.bit) != 0).unwrap_or(false)
}

/// True iff `id`'s prereq is satisfied (no prereq, or the prereq tech is unlocked).
#[inline]
pub fn prereq_met(tech_unlocked: u32, id: usize) -> bool {
    match TECHS.get(id) {
        Some(t) if t.prereq >= 0 => is_unlocked(tech_unlocked, t.prereq as usize),
        Some(_) => true, // tier-1 root, no prereq
        None => false,
    }
}
