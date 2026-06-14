//! S11 — the SPELL ARM (the magic counterpart to the legions): mana-funded spells, unlocked by the
//! SPELLCRAFT tech, that are AUTO-TARGETED (the engine picks the target — the front nearest the capital,
//! the breaching raider, the most-stalled siege) but PLAYER-CAST by default. The player chooses WHEN, not
//! WHERE — and "when" is the lever: spells draw the SAME mana pool as tech (aether is your science AND
//! your magic), so casting now is teching later (bank for an upgrade vs. let the spells fly).
//!
//! Casting is a `Command::CastSpell { kind }` ⇒ `cast()`. An optional AUTOCAST toggle
//! (`Command::SetAutocast`) restores the hands-off Majesty mode, where `step()` fires the whole battery
//! each tick at the biggest threat. MANA COST is the rate limiter (no rng, no cooldown state — you can
//! only cast what you've banked):
//!   PURGE FRONT — retreat the decadence tide (clear the corrupted cell nearest the capital + halve its ring).
//!   SMITE       — kill a raider that's slipped the rail cordon and neared the capital.
//!   WARPATH     — empower the most-stalled siege (+50% strength, capped) to crack a deep garrison.
//!
//! Gate-safe + deterministic: integer, index-ordered targeting, NO rng. Inert without SPELLCRAFT, so
//! transit + the goldens keep `spells_cast` 0 and cast nothing — golden-neutral (`autocast` is excluded
//! from `Canonical`; `spell_flashes` are render-only, never hashed).
use crate::world::World;

pub const PURGE_FRONT: u8 = 0;
pub const SMITE: u8 = 1;
pub const WARPATH: u8 = 2;

pub const PURGE_COST: i64 = 14;
pub const SMITE_COST: i64 = 10;
pub const WARPATH_COST: i64 = 18;

/// The MANA cost of a spell `kind` (the TS `SPELLS` table in `codec.ts` mirrors these). Unknown ⇒ i64::MAX
/// (never affordable), so a bad kind can't cast.
pub fn cost_of(kind: u8) -> i64 {
    match kind {
        PURGE_FRONT => PURGE_COST,
        SMITE => SMITE_COST,
        WARPATH => WARPATH_COST,
        _ => i64::MAX,
    }
}
/// A raider this close (mm) to the capital is SMITE-worthy (about to breach) — ~1.5× the raider ARRIVE_MM.
const SMITE_RANGE_MM: i64 = 3_000_000;

/// A render-only spell flash (a brief burst at the cast site). NOT hashed.
#[derive(Clone)]
pub struct SpellFlash {
    pub x_mm: i64,
    pub y_mm: i64,
    pub kind: u8,
    pub age_ms: i64,
}
const FLASH_MS: i64 = 1500;

/// The spell phase (run in `war_step` AFTER the decadence derivation, so a Purge retreats the front for
/// the next tick + reads the settled field). Ages flashes ALWAYS. Only AUTOCAST fires here; the default
/// (manual) path is the `CastSpell` command → [`cast`]. Inert without SPELLCRAFT either way.
pub(crate) fn step(world: &mut World, dt_ms: i64) {
    let dt = dt_ms.max(0);
    for f in &mut world.spell_flashes {
        f.age_ms += dt;
    }
    world.spell_flashes.retain(|f| f.age_ms < FLASH_MS);

    // AUTOCAST (opt-in, off by default): the hands-off Majesty mode — fire the whole battery at the biggest
    // threat each tick. Off ⇒ spells fire only on the player's command. Locked without SPELLCRAFT either
    // way, so transit + the goldens never cast here (golden-neutral).
    if !world.autocast || !crate::tech::is_unlocked(world.tech_unlocked, crate::tech::SPELLCRAFT) {
        return;
    }
    // Purge only when the rot is a real THREAT (≥¼ up the lose meter) so autocast doesn't burn mana on a
    // distant front; smite/warpath self-limit (no-op without a target). Each cast drains mana so the next
    // sees the reduced pool.
    if world.decadence.saturating_mul(4) >= crate::decadence::CAPITAL_THRESHOLD {
        try_purge(world);
    }
    try_smite(world);
    try_warpath(world);
}

/// Cast ONE spell on the player's command (the DEFAULT path). Auto-targets (the engine picks the target),
/// spends mana. Returns true iff it cast (mana sufficed AND a valid target existed). The caller (`apply`)
/// has already checked SPELLCRAFT. NO threat gate — the player chose to cast; if there's a target, it fires.
pub(crate) fn cast(world: &mut World, kind: u8) -> bool {
    match kind {
        PURGE_FRONT => try_purge(world),
        SMITE => try_smite(world),
        WARPATH => try_warpath(world),
        _ => false,
    }
}

fn fire(world: &mut World, kind: u8, cost: i64, x: i64, y: i64) {
    world.mana = world.mana.saturating_sub(cost);
    world.spells_cast = world.spells_cast.saturating_add(1);
    world.spell_flashes.push(SpellFlash { x_mm: x, y_mm: y, kind, age_ms: 0 });
}

/// PURGE FRONT: clear the corrupted cell NEAREST the capital (+ halve its neighbours), retreating the
/// lose-meter front. Fires when a corrupted cell exists and mana ≥ cost (the autocast caller adds the
/// threat gate). Returns true iff it cast.
fn try_purge(world: &mut World) -> bool {
    if world.mana < PURGE_COST {
        return false;
    }
    // Find the corrupted cell NEAREST the capital + its neighbours/position under the field borrow, then
    // drop the borrow before mutating.
    let found = {
        let field = &world.decadence_field;
        if field.is_empty() {
            return false;
        }
        let n = world.decadence_cells.len().min(field.dist_to_capital.len());
        let mut best: Option<(u32, usize)> = None; // (dist, cell)
        for c in 0..n {
            if world.decadence_cells[c] >= crate::decadence_field::FRONT_THRESHOLD {
                let d = field.dist_to_capital[c];
                if d != u32::MAX && best.map_or(true, |(bd, _)| d < bd) {
                    best = Some((d, c));
                }
            }
        }
        best.map(|(_, cell)| {
            let p = crate::hexgrid::center_of(field.cells[cell], world.city.grid_cell_mm.max(1));
            (cell, field.neighbors(cell as u32).to_vec(), p.x_mm, p.y_mm)
        })
    };
    let Some((cell, nbrs, fx, fy)) = found else { return false };
    world.decadence_cells[cell] = 0;
    for nb in nbrs {
        let i = nb as usize;
        if i < world.decadence_cells.len() {
            world.decadence_cells[i] /= 2;
        }
    }
    fire(world, PURGE_FRONT, PURGE_COST, fx, fy);
    true
}

/// SMITE: kill the MARCHING raider nearest the capital (within SMITE range) — the one that slipped the rail
/// cordon. Index-ordered tiebreak. Returns true iff it cast.
fn try_smite(world: &mut World) -> bool {
    if world.mana < SMITE_COST {
        return false;
    }
    let (cx, cy) = (world.city.capital_x_mm, world.city.capital_y_mm);
    let range2 = SMITE_RANGE_MM.saturating_mul(SMITE_RANGE_MM);
    let mut best: Option<(i64, usize)> = None; // (dist², slot)
    for i in 0..world.raiders.len() {
        if world.raiders.state[i] != crate::raider::MARCHING {
            continue;
        }
        let (dx, dy) = (cx - world.raiders.x_mm[i], cy - world.raiders.y_mm[i]);
        let d2 = dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy));
        if d2 <= range2 && best.map_or(true, |(bd, _)| d2 < bd) {
            best = Some((d2, i));
        }
    }
    let Some((_, i)) = best else { return false };
    let (x, y) = (world.raiders.x_mm[i], world.raiders.y_mm[i]);
    world.raiders.state[i] = crate::raider::DONE; // smitten — the same despawn the cordon uses
    fire(world, SMITE, SMITE_COST, x, y);
    true
}

/// WARPATH: empower the most-stalled siege — the BESIEGING legion whose target retains the most resistance
/// — by +50% strength, capped at 2× launch (so it can't snowball). Skips a legion already at the cap.
/// Returns true iff it cast.
fn try_warpath(world: &mut World) -> bool {
    if world.mana < WARPATH_COST {
        return false;
    }
    let mut best: Option<(i64, usize)> = None; // (town_value, army)
    for i in 0..world.armies.len() {
        if world.armies.state[i] != crate::army::BESIEGING {
            continue;
        }
        let t = world.armies.target[i] as usize;
        let tv = world.town_value.get(t).copied().unwrap_or(0);
        if tv > 0 && best.map_or(true, |(btv, _)| tv > btv) {
            best = Some((tv, i));
        }
    }
    let Some((_, i)) = best else { return false };
    let boosted = (world.armies.strength[i] * 3 / 2).min(2 * crate::army::LAUNCH_COST);
    if boosted <= world.armies.strength[i] {
        return false; // already at the cap — don't waste mana
    }
    let t = world.armies.target[i] as usize;
    let (fx, fy) = world
        .stations
        .get(t)
        .map(|s| (s.pos.x_mm, s.pos.y_mm))
        .unwrap_or((world.city.capital_x_mm, world.city.capital_y_mm));
    world.armies.strength[i] = boosted;
    fire(world, WARPATH, WARPATH_COST, fx, fy);
    true
}
