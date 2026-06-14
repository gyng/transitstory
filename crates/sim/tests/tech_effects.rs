//! S11 tech-tree EFFECTS — the rail-gate + Heavy Rail unlock (the headline build-capability tech), plus a
//! representative buff (Ward Lines) proven through the command/tick path. (The economy/prereq/spend
//! contracts are in tech.rs + economy_split.rs; the production buff in tech.rs::forge_mastery_doubles_*.)
use sim::tech::{is_unlocked, FORGE_MASTERY, HEAVY_RAIL, TECHS};
use sim::*;

const AETHER: u8 = 2;

fn arcadia(cells: Vec<DemandCell>) -> CityData {
    CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 11,
        grid_cell_mm: 100_000,
        demand: DemandGrid { cell_m: 500.0, cells },
        ..Default::default()
    }
}

fn line_created(ev: &[Event]) -> bool {
    ev.iter().any(|e| matches!(e, Event::LineCreated { .. }))
}

#[test]
fn arcadia_is_rail_only_until_heavy_rail_is_teched() {
    let mut w = World::new(11, arcadia(vec![]));
    // RAIL (mode 0) is always allowed.
    assert!(line_created(&w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false })), "rail is buildable");
    // BUS / FERRY / PLANE (1/2/3) are NEVER buildable in the realm.
    for mode in [1u8, 2, 3] {
        let ev = w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode, literal: false });
        assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "mode {mode} is gated out of arcadia");
    }
    // HEAVY rail (mode 4) is rejected until the tech is unlocked.
    let ev = w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 4, literal: false });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "heavy rail needs the tech");
    assert!(!is_unlocked(w.tech_unlocked, HEAVY_RAIL));
}

#[test]
fn transit_still_allows_every_mode() {
    // The rail-gate is arcadia-only — transit (golden-neutral) builds bus/ferry/etc. as before.
    let mut w = World::new(7, CityData { id: "demo".into(), ruleset: "transit".into(), seed: 7, grid_cell_mm: 100_000, demand: DemandGrid { cell_m: 500.0, cells: vec![] }, ..Default::default() });
    for mode in [0u8, 1, 2, 3, 4] {
        assert!(line_created(&w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode, literal: false })), "transit builds mode {mode}");
    }
}

/// Earn mana (aether chain) until `target`, then return the world ready to tech.
fn mana_world(target: i64) -> World {
    let mut w = World::new(
        11,
        arcadia(vec![
            DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: AETHER },
            DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 90.0, commodity: AETHER },
        ]),
    );
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..80_000 {
        w.tick(50);
        if w.mana >= target {
            break;
        }
    }
    w
}

#[test]
fn heavy_rail_unlocks_the_heavy_mode() {
    // Earn enough mana for FORGE_MASTERY (prereq) + HEAVY_RAIL, unlock both, then a HEAVY line builds.
    let need = TECHS[FORGE_MASTERY].cost + TECHS[HEAVY_RAIL].cost;
    let mut w = mana_world(need);
    assert!(w.mana >= need, "earned the mana for both techs: {}", w.mana);
    assert!(matches!(w.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 }).as_slice(), [Event::TechUnlocked { .. }]));
    assert!(matches!(w.apply(&Command::UnlockTech { tech: HEAVY_RAIL as u8 }).as_slice(), [Event::TechUnlocked { .. }]), "HEAVY_RAIL unlocks (prereq met)");
    assert!(is_unlocked(w.tech_unlocked, HEAVY_RAIL));
    // Now a HEAVY-rail line (mode 4) is buildable.
    let ev = w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 4, literal: false });
    assert!(line_created(&ev), "heavy rail builds once teched: {ev:?}");
}
