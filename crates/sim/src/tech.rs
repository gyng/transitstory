//! S11 — the tech tree + the ECONOMY SPLIT. A tech is bought with one of THREE channels (the split of the
//! old single `tribute`): GOLD (minted by every delivery — the universal war-chest, unchanged), MANA
//! (minted alongside gold by AETHER chains), and MANPOWER (minted alongside gold by INGOT/ARMS chains).
//! Gold is untouched in volume, so the war-chest balance is preserved; mana + manpower are ADDITIVE
//! specialised yields that gate channel-specific tech — so the COMPOSITION of your supply network matters
//! (arcane aether → mana → defensive tech; arms → manpower → military tech), not just its raw volume.
//!
//! Each tech is one bit in `World.tech_unlocked` (a hashed bitset); unlocking spends its channel once and
//! flips the bit permanently, gating a buff to an EXISTING lever (production / war / defence).
//!
//! **Golden-neutral effects.** Every effect READS its bit and falls back to the shipped constant when
//! unset, so transit (the ruleset rejects `UnlockTech` ⇒ `tech_unlocked`/`mana`/`manpower` stay 0) and the
//! arcadia golden (its log predates tech + delivers only ORE ⇒ gold-only) are behaviourally byte-identical.
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

/// Which channel a DELIVERED commodity mints (alongside gold). AETHER → mana; a processed war good
/// (INGOT/ARMS, ≥ `forge::FIRST_MID`) → manpower; everything mundane (ore/grain/fuel) → gold only.
#[inline]
pub fn channel_of(commodity: usize) -> Channel {
    if commodity == crate::forge::AETHER {
        Channel::Mana
    } else if commodity >= crate::forge::FIRST_MID {
        Channel::Manpower
    } else {
        Channel::Gold
    }
}

/// A tech: the bit it sets, its cost, and the CHANNEL that cost is paid in (S11 — so a military tech wants
/// a manpower economy, an arcane tech a mana economy). The id (its index in [`TECHS`]) is what the Command
/// carries — stable across saves.
#[derive(Clone, Copy, Debug)]
pub struct Tech {
    pub bit: u8,
    pub cost: i64,
    pub channel: Channel,
}

/// FORGE MASTERY (id 0): raw production ×2 — the flywheel tech, bought with GOLD (any supply funds it).
pub const FORGE_MASTERY: usize = 0;
/// CONSCRIPTION (id 1): legions cost HALF the tribute — the war tech, bought with MANPOWER (supply an
/// arms chain to afford it).
pub const CONSCRIPTION: usize = 1;
/// SAPPERS (id 2): the decadence tide creeps at HALF rate — the defence tech, bought with MANA (supply an
/// aether chain to afford it).
pub const SAPPERS: usize = 2;

/// The shipped tech table. Index = the id a `Command::UnlockTech { tech }` carries. Order is FIXED.
pub const TECHS: [Tech; 3] = [
    Tech { bit: 0, cost: 24, channel: Channel::Gold }, // FORGE_MASTERY
    Tech { bit: 1, cost: 40, channel: Channel::Manpower }, // CONSCRIPTION
    Tech { bit: 2, cost: 56, channel: Channel::Mana }, // SAPPERS
];

/// True iff the tech at `id` (an index into [`TECHS`]) is unlocked in this bitset. Unknown ids → false.
#[inline]
pub fn is_unlocked(tech_unlocked: u32, id: usize) -> bool {
    TECHS.get(id).map(|t| tech_unlocked & (1u32 << t.bit) != 0).unwrap_or(false)
}
