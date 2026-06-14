//! S11 — the SPELL ARM (the magic counterpart to the legions): mana-funded AUTO-CAST spells the AI fires
//! at the biggest threat, Majesty-style (no micro), unlocked by the SPELLCRAFT tech. Spells draw the SAME
//! mana pool as tech — aether is your science AND your magic — so casting is a live drain on the tech
//! budget (bank for an upgrade vs. let the spells fly).
//!
//! Three fire-and-forget spells; each casts when its threat is present + mana suffices. MANA COST is the
//! rate limiter (no rng, no cooldown state — you can only cast what you've banked):
//!   PURGE FRONT — retreat the decadence tide (clear the corrupted cell nearest the capital + halve its ring).
//!   SMITE       — kill a raider that's slipped the rail cordon and neared the capital.
//!   WARPATH     — empower the most-stalled siege (+50% strength, capped) to crack a deep garrison.
//!
//! Gate-safe + deterministic: integer, index-ordered targeting, NO rng. Inert without SPELLCRAFT, so
//! transit + the goldens keep `spells_cast` 0 and cast nothing — golden-neutral (one re-pin for the
//! appended counter; `spell_flashes` are render-only, never hashed).
use crate::world::World;

pub const PURGE_FRONT: u8 = 0;
pub const SMITE: u8 = 1;
pub const WARPATH: u8 = 2;

const PURGE_COST: i64 = 30;
const SMITE_COST: i64 = 25;
const WARPATH_COST: i64 = 35;
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
/// the next tick + reads the settled field). Ages flashes always; casts only with SPELLCRAFT.
pub(crate) fn step(world: &mut World, dt_ms: i64) {
    let dt = dt_ms.max(0);
    for f in &mut world.spell_flashes {
        f.age_ms += dt;
    }
    world.spell_flashes.retain(|f| f.age_ms < FLASH_MS);

    if !crate::tech::is_unlocked(world.tech_unlocked, crate::tech::SPELLCRAFT) {
        return; // the spell arm is locked
    }
    // Cast in priority order; each cast drains mana so the next sees the reduced pool.
    try_purge(world);
    try_smite(world);
    try_warpath(world);
}

fn cast(world: &mut World, kind: u8, cost: i64, x: i64, y: i64) {
    world.mana = world.mana.saturating_sub(cost);
    world.spells_cast = world.spells_cast.saturating_add(1);
    world.spell_flashes.push(SpellFlash { x_mm: x, y_mm: y, kind, age_ms: 0 });
}

/// PURGE FRONT: clear the corrupted cell NEAREST the capital (+ halve its neighbours), retreating the
/// lose-meter front. Cast only when the front threatens the heartland (dist ≤ LOSE_DIST+2) and mana ≥ cost.
fn try_purge(world: &mut World) {
    if world.mana < PURGE_COST {
        return;
    }
    // Find the front cell + its neighbours/position under the field borrow, then drop it before mutating.
    let found = {
        let field = &world.decadence_field;
        if field.is_empty() {
            return;
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
        match best {
            Some((dist, cell)) if dist <= crate::decadence_field::LOSE_DIST + 2 => {
                let p = crate::hexgrid::center_of(field.cells[cell], world.city.grid_cell_mm.max(1));
                Some((cell, field.neighbors(cell as u32).to_vec(), p.x_mm, p.y_mm))
            }
            _ => None,
        }
    };
    let Some((cell, nbrs, fx, fy)) = found else { return };
    world.decadence_cells[cell] = 0;
    for nb in nbrs {
        let i = nb as usize;
        if i < world.decadence_cells.len() {
            world.decadence_cells[i] /= 2;
        }
    }
    cast(world, PURGE_FRONT, PURGE_COST, fx, fy);
}

/// SMITE: kill the MARCHING raider nearest the capital (within SMITE range) — the one that slipped the rail
/// cordon. Index-ordered tiebreak.
fn try_smite(world: &mut World) {
    if world.mana < SMITE_COST {
        return;
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
    let Some((_, i)) = best else { return };
    let (x, y) = (world.raiders.x_mm[i], world.raiders.y_mm[i]);
    world.raiders.state[i] = crate::raider::DONE; // smitten — the same despawn the cordon uses
    cast(world, SMITE, SMITE_COST, x, y);
}

/// WARPATH: empower the most-stalled siege — the BESIEGING legion whose target retains the most resistance
/// — by +50% strength, capped at 2× launch (so it can't snowball). Skips a legion already at the cap.
fn try_warpath(world: &mut World) {
    if world.mana < WARPATH_COST {
        return;
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
    let Some((_, i)) = best else { return };
    let boosted = (world.armies.strength[i] * 3 / 2).min(2 * crate::army::LAUNCH_COST);
    if boosted <= world.armies.strength[i] {
        return; // already at the cap — don't waste mana
    }
    let t = world.armies.target[i] as usize;
    let (fx, fy) = world
        .stations
        .get(t)
        .map(|s| (s.pos.x_mm, s.pos.y_mm))
        .unwrap_or((world.city.capital_x_mm, world.city.capital_y_mm));
    world.armies.strength[i] = boosted;
    cast(world, WARPATH, WARPATH_COST, fx, fy);
}
