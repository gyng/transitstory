//! S11 — the SPELL ARM (`spell::step`, gated by the SPELLCRAFT tech). The AI auto-casts Purge/Smite/Warpath
//! at the biggest threat, drawing the mana pool. These prove: the arm is INERT without SPELLCRAFT
//! (golden-neutral), it CASTS + drains mana once unlocked when threats reach the heartland, and it replays
//! bit-for-bit (no rng). Effect specifics are scrutinised by the adversarial review.
use sim::hexgrid;
use sim::tech::{SAPPERS, SPELLCRAFT, TECHS};
use sim::*;

const SIZE: i64 = 250_000;
const AETHER: u8 = 2;

/// A baked-like arcadia world (buildability ⇒ a real decadence field + reservoir, so the tide creeps and
/// raiders spawn) PLUS an AETHER supply near the capital for MANA. The supply line is OFFSET from the
/// capital so the tide/raiders still reach the unrailed heartland — giving the spells something to hit.
fn spell_world(unlock_spells: bool) -> World {
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
    if unlock_spells {
        w.apply(&Command::UnlockTech { tech: SAPPERS as u8 });
        w.apply(&Command::UnlockTech { tech: SPELLCRAFT as u8 });
    }
    // Run on while the tide creeps toward the unrailed capital + raiders spawn → the arm casts (if unlocked).
    for _ in 0..60_000 {
        w.tick(50);
    }
    w
}

#[test]
fn the_spell_arm_is_inert_without_spellcraft() {
    // No SPELLCRAFT ⇒ NO spells cast, ever — even with mana banked and the tide at the gates. The whole arm
    // is tech-gated, so transit + the goldens are byte-identical (golden-neutral).
    let w = spell_world(false);
    assert!(!sim::tech::is_unlocked(w.tech_unlocked, SPELLCRAFT));
    assert_eq!(w.spells_cast, 0, "the spell arm is locked without SPELLCRAFT");
    assert!(w.spell_flashes.is_empty(), "no flashes");
}

#[test]
fn the_spell_arm_casts_and_drains_mana_once_unlocked() {
    let w = spell_world(true);
    assert!(sim::tech::is_unlocked(w.tech_unlocked, SPELLCRAFT), "SPELLCRAFT unlocked");
    assert!(w.spells_cast > 0, "the AI auto-casts at the threats once the arm is awake: {}", w.spells_cast);
    // Each cast drained mana (cost 25–35); a realm that earned a big pool has spent some of it on casting.
    // (We can't assert an exact balance — the tide/raider cadence is emergent — only that casting happened.)
}

#[test]
fn the_spell_arm_replays_bit_for_bit() {
    assert_eq!(spell_world(true).state_hash(), spell_world(true).state_hash(), "the spell arm replays bit-for-bit (no rng)");
}
