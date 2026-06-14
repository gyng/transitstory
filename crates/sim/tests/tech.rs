//! S11 — the tech tree (`tech::TECHS` + `Command::UnlockTech`). A tech is bought once with MANA (the sole
//! tech resource), flips a permanent bit in `tech_unlocked`, and gates a buff to an existing lever. These
//! tests prove the COMMAND contract (spend-exactly-once / reject unknown·repeat·broke / transit refuses it)
//! and that an unlocked bit actually CHANGES behaviour (FORGE_MASTERY doubles production), all through the
//! `apply`/`tick` path — never by poking `World` internals. Channel/prereq specifics live in economy_split.rs.
use sim::tech::{FORGE_MASTERY, TECHS};
use sim::*;

const AETHER: u8 = 2; // an AETHER chain mints MANA — the tech resource

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

/// A strong AETHER supply chain (source → sink), the fastest MANA earner: source origin 90, sink dest 90,
/// 1.5 Mm apart (past the catchment), 4 carts on a short headway. Mana is the tech resource.
fn supply_world() -> World {
    let mut w = World::new(
        11,
        arcadia(vec![
            DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: AETHER }, // aether source
            DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 90.0, commodity: AETHER }, // sink
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
    w
}

/// Tick until mana ≥ `target` (or `cap` ticks elapse — a runaway guard). Returns the tick count.
fn tick_until_mana(w: &mut World, target: i64, cap: usize) -> usize {
    for tick in 1..=cap {
        w.tick(50);
        if w.mana >= target {
            return tick;
        }
    }
    cap
}

#[test]
fn unlock_spends_mana_and_sets_the_bit() {
    let mut w = supply_world();
    let cost = TECHS[FORGE_MASTERY].cost;
    tick_until_mana(&mut w, cost, 60_000);
    let before = w.mana;
    assert!(before >= cost, "the supply chain must earn the tech's cost in mana first (got {before})");

    let events = w.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    assert_eq!(w.mana, before - cost, "exactly the tech's mana cost is spent");
    assert!(w.tech_unlocked & (1 << TECHS[FORGE_MASTERY].bit) != 0, "the tech's bit is set");
    assert!(
        matches!(events.as_slice(), [Event::TechUnlocked { tech, balance_left }] if *tech == 0 && *balance_left == w.mana),
        "UnlockTech emits TechUnlocked with the remaining mana: {events:?}"
    );
}

#[test]
fn unlock_rejects_unknown_repeat_and_broke() {
    // UNKNOWN id ⇒ rejected, no mutation.
    let mut w = supply_world();
    tick_until_mana(&mut w, TECHS[FORGE_MASTERY].cost, 60_000);
    let before = w.mana;
    let ev = w.apply(&Command::UnlockTech { tech: 99 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "unknown tech is rejected");
    assert_eq!(w.mana, before, "a rejected unlock spends nothing");
    assert_eq!(w.tech_unlocked, 0, "a rejected unlock sets no bit");

    // REPEAT ⇒ the second unlock of the same tech is rejected (exactly-once spend).
    w.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    let after_first = w.mana;
    let ev = w.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "re-unlocking is rejected");
    assert_eq!(w.mana, after_first, "the repeat spends nothing");

    // BROKE ⇒ a fresh world (0 mana) can't afford a tech.
    let mut poor = supply_world();
    let ev = poor.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "an unaffordable unlock is rejected");
    assert_eq!(poor.mana, 0);
    assert_eq!(poor.tech_unlocked, 0);
}

#[test]
fn transit_refuses_unlock_tech() {
    // The transit ruleset rejects UnlockTech BEFORE it mutates or joins the save (the disjoint-save
    // guard) — so a transit save never carries a fantasy command, and tech_unlocked stays 0.
    let mut w = World::new(
        7,
        CityData {
            id: "demo".into(),
            ruleset: "transit".into(),
            seed: 7,
            grid_cell_mm: 100_000,
            demand: DemandGrid { cell_m: 500.0, cells: vec![] },
            ..Default::default()
        },
    );
    let ev = w.apply(&Command::UnlockTech { tech: 0 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "transit rejects a fantasy command");
    assert_eq!(w.tech_unlocked, 0);
}

#[test]
fn forge_mastery_doubles_production() {
    // EFFECT: unlock FORGE_MASTERY as soon as affordable, then run a long horizon — the doubled raw rate
    // out-earns a control that never unlocks, even after paying the one-time mana cost. Proves the bit
    // changes behaviour (not just sets a flag), through the command path. (Aether production → mana, so the
    // doubled rate shows as more mana by the horizon.)
    const HORIZON: usize = 50_000;
    let cost = TECHS[FORGE_MASTERY].cost;

    let mut control = supply_world();
    for _ in 0..HORIZON {
        control.tick(50);
    }

    let mut mastery = supply_world();
    let mut unlocked = false;
    for _ in 0..HORIZON {
        mastery.tick(50);
        if !unlocked && mastery.mana >= cost {
            mastery.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
            unlocked = true;
        }
    }
    assert!(unlocked, "the mastery run must afford the tech within the horizon");
    assert!(
        mastery.mana > control.mana,
        "FORGE_MASTERY's doubled production out-earns the control (mastery {} > control {})",
        mastery.mana,
        control.mana
    );
}

#[test]
fn the_tech_flow_replays_bit_for_bit() {
    // Determinism: the same supply + auto-unlock sequence yields an identical state_hash twice (so any
    // tech-balance counterexample replays exactly). Covers the hashed `tech_unlocked`/`mana` fields.
    fn run() -> u64 {
        let mut w = supply_world();
        let cost = TECHS[FORGE_MASTERY].cost;
        let mut done = false;
        for _ in 0..20_000 {
            w.tick(50);
            if !done && w.mana >= cost {
                w.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
                done = true;
            }
        }
        w.state_hash()
    }
    assert_eq!(run(), run(), "the tech flow replays bit-for-bit");
}
