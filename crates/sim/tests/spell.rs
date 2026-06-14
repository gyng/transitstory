//! S11 — the SPELL ARM (`spell::cast` on `Command::CastSpell`, gated by the SPELLCRAFT tech). Spells are
//! AUTO-TARGETED (the engine picks the target) but PLAYER-CAST by default; an optional AUTOCAST toggle
//! restores the hands-off Majesty mode. These prove: the arm is INERT without SPELLCRAFT (golden-neutral);
//! unlocking the tech ALONE casts NOTHING (manual default — the player picks when); a `CastSpell` command
//! fires + drains mana; the autocast toggle makes `step` auto-fire; and the whole thing replays bit-for-bit
//! (no rng). Effect specifics are scrutinised by the adversarial review.
use sim::hexgrid;
use sim::spell::{PURGE_COST, PURGE_FRONT};
use sim::tech::{SAPPERS, SPELLCRAFT, TECHS};
use sim::*;

const SIZE: i64 = 250_000;
const AETHER: u8 = 2;

/// A baked-like arcadia world (buildability ⇒ a real decadence field + reservoir, so the tide creeps and
/// raiders spawn) PLUS an AETHER supply near the capital for MANA. The supply line is OFFSET from the
/// capital so the tide/raiders still reach the unrailed heartland — giving the spells something to hit.
/// `unlock` ⇒ buy SAPPERS + SPELLCRAFT; `autocast` ⇒ also toggle autocast ON. After setup it runs a long
/// stretch so the tide corrupts cells (a PURGE target) and mana banks.
fn spell_world(unlock: bool, autocast: bool) -> World {
    let mut cells = Vec::new();
    for q in 0..14 {
        for r in 0..14 {
            let p = hexgrid::center_of((q, r), SIZE);
            cells.push(BuildCell { x_mm: p.x_mm, y_mm: p.y_mm, c: 10 });
        }
    }
    let cap = hexgrid::center_of((0, 0), SIZE);
    let src = hexgrid::center_of((2, 0), SIZE);
    let sink = hexgrid::center_of((4, 0), SIZE);
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12,
        grid_cell_mm: SIZE,
        capital_x_mm: cap.x_mm,
        capital_y_mm: cap.y_mm,
        initial_decadence: 6000,
        buildability: BuildabilityGrid { cell_m: SIZE as f64 / 1000.0, cells },
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: src.x_mm, y_mm: src.y_mm, origin_w: 90.0, dest_w: 2.0, commodity: AETHER },
                DemandCell { x_mm: sink.x_mm, y_mm: sink.y_mm, origin_w: 2.0, dest_w: 90.0, commodity: AETHER },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(12, city);
    w.apply(&Command::PlaceStation { x_mm: cap.x_mm, y_mm: cap.y_mm, name: None }); // 0 capital (NOT railed)
    w.apply(&Command::PlaceStation { x_mm: src.x_mm, y_mm: src.y_mm, name: None }); // 1 aether source
    w.apply(&Command::PlaceStation { x_mm: sink.x_mm, y_mm: sink.y_mm, name: None }); // 2 sink
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
    w.apply(&Command::SetRunning { running: true });
    // Earn mana for both techs (SAPPERS + SPELLCRAFT).
    let need = TECHS[SAPPERS].cost + TECHS[SPELLCRAFT].cost;
    for _ in 0..60_000 {
        w.tick(50);
        if w.mana >= need + 60 {
            break;
        }
    }
    if unlock {
        w.apply(&Command::UnlockTech { tech: SAPPERS as u8 });
        w.apply(&Command::UnlockTech { tech: SPELLCRAFT as u8 });
    }
    if autocast {
        w.apply(&Command::SetAutocast { enabled: true });
    }
    // Run on while the tide creeps toward the unrailed capital + raiders spawn. Autocast ⇒ the arm casts;
    // manual (default) ⇒ NOTHING casts here (the player hasn't issued a CastSpell).
    for _ in 0..60_000 {
        w.tick(50);
    }
    w
}

#[test]
fn the_spell_arm_is_inert_without_spellcraft() {
    // No SPELLCRAFT ⇒ NO spells cast, ever — even with mana banked, autocast asked for, and the tide at the
    // gates. The whole arm is tech-gated, so transit + the goldens are byte-identical (golden-neutral).
    let w = spell_world(false, true);
    assert!(!sim::tech::is_unlocked(w.tech_unlocked, SPELLCRAFT));
    assert_eq!(w.spells_cast, 0, "the spell arm is locked without SPELLCRAFT");
    assert!(w.spell_flashes.is_empty(), "no flashes");
    // And a CastSpell command is rejected (no mutation) without the tech.
    let mut w2 = spell_world(false, false);
    let before = w2.spells_cast;
    let ev = w2.apply(&Command::CastSpell { kind: PURGE_FRONT });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "CastSpell needs SPELLCRAFT: {ev:?}");
    assert_eq!(w2.spells_cast, before, "rejected cast mutates nothing");
}

#[test]
fn casting_is_manual_by_default() {
    // THE new invariant: unlocking SPELLCRAFT does NOT auto-fire. With autocast OFF (the default), the arm
    // casts nothing on its own no matter how long the tide rages — the player picks WHEN (the invest-vs-cast
    // tradeoff). This is what makes the back-half tech tree reachable (no silent mana drain).
    let w = spell_world(true, false);
    assert!(sim::tech::is_unlocked(w.tech_unlocked, SPELLCRAFT), "SPELLCRAFT unlocked");
    assert_eq!(w.spells_cast, 0, "the default is MANUAL — unlocking the tech alone casts nothing");
}

#[test]
fn a_cast_command_fires_a_spell_and_drains_mana() {
    // Manual cast: the realm has SPELLCRAFT, mana, and a crept tide (a PURGE target). One CastSpell command
    // fires exactly one spell and drains exactly its mana cost.
    let mut w = spell_world(true, false);
    assert_eq!(w.spells_cast, 0, "nothing cast yet (manual)");
    assert!(w.mana >= PURGE_COST, "mana banked for a cast: {}", w.mana);
    assert!(
        w.decadence_cells.iter().any(|&c| c >= 1),
        "the tide has corrupted a cell (a PURGE target exists)"
    );
    let mana_before = w.mana;
    let ev = w.apply(&Command::CastSpell { kind: PURGE_FRONT });
    assert!(matches!(ev.as_slice(), [Event::SpellCast { .. }]), "the cast fires: {ev:?}");
    assert_eq!(w.spells_cast, 1, "exactly one spell cast on the command");
    assert_eq!(w.mana, mana_before - PURGE_COST, "the cast drained exactly its mana cost");
}

#[test]
fn autocast_toggle_makes_the_arm_auto_fire() {
    // With autocast toggled ON, the arm fires hands-off at the biggest threat each tick (the Majesty mode).
    let w = spell_world(true, true);
    assert!(w.autocast, "autocast is on");
    assert!(w.spells_cast > 0, "autocast fires at the threats: {}", w.spells_cast);
}

#[test]
fn the_spell_arm_replays_bit_for_bit() {
    // Both the command path (autocast off, the default) and the autocast path replay identically — no rng.
    assert_eq!(
        spell_world(true, false).state_hash(),
        spell_world(true, false).state_hash(),
        "manual realm replays bit-for-bit"
    );
    assert_eq!(
        spell_world(true, true).state_hash(),
        spell_world(true, true).state_hash(),
        "autocast realm replays bit-for-bit"
    );
}
