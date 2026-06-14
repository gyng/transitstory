//! S11 ECONOMY SPLIT — the single `tribute` becomes three channels: GOLD (every delivery, unchanged),
//! MANA (minted alongside gold by AETHER chains), MANPOWER (minted alongside gold by INGOT/ARMS chains).
//! Gold's VOLUME is untouched (the war-chest balance is preserved); mana/manpower are additive specialised
//! yields that gate channel-specific tech. These prove the minting (by commodity), the gold invariant, and
//! that a tech can only be bought with ITS channel — all through the command/tick path.
use sim::tech::{channel_of, Channel, FORGE_MASTERY, PRODUCTION_SURGE, TECHS};
use sim::*;

const ORE: u8 = 0;
const GRAIN: u8 = 1;
const AETHER: u8 = 2;
const INGOT: u8 = 4;

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

#[test]
fn channel_of_maps_commodities_to_their_yield() {
    assert_eq!(channel_of(0), Channel::Gold, "ORE → gold");
    assert_eq!(channel_of(1), Channel::Manpower, "GRAIN → manpower (V3: food → soldiers)");
    assert_eq!(channel_of(2), Channel::Mana, "AETHER → mana");
    assert_eq!(channel_of(3), Channel::Gold, "FUEL → gold");
    assert_eq!(channel_of(4), Channel::Manpower, "INGOT (a war good) → manpower");
    assert_eq!(channel_of(7), Channel::Manpower, "a final war good → manpower");
}

/// A single-commodity source → sink line (commodity `comm`), run to deliver + consume into the channels.
fn run_supply(comm: u8, ticks: usize) -> World {
    let mut w = World::new(
        11,
        arcadia(vec![
            DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: comm }, // source
            DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 90.0, commodity: 0 }, // sink (consume-all)
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
    for _ in 0..ticks {
        w.tick(50);
    }
    w
}

#[test]
fn aether_mints_mana_alongside_gold_but_ore_does_not() {
    // An AETHER chain mints MANA — and gold (tribute) at the SAME volume (the split is additive, not a
    // division): every aether unit consumed mints both, so mana == gold here.
    let aether = run_supply(AETHER, 6000);
    assert!(aether.mana > 0, "an aether chain mints mana: {}", aether.mana);
    assert_eq!(aether.mana, aether.tribute, "gold's volume is UNCHANGED — mana is additive (mana == gold for a pure-aether sink)");
    assert_eq!(aether.manpower, 0, "aether mints no manpower");

    // An ORE (gold) chain mints gold ONLY — no mana, no manpower (the demo/golden path ⇒ byte-identical).
    let ore = run_supply(ORE, 6000);
    assert!(ore.tribute > 0, "an ore chain mints gold: {}", ore.tribute);
    assert_eq!(ore.mana, 0, "ore mints no mana (the golden stays gold-only)");
    assert_eq!(ore.manpower, 0, "ore mints no manpower");
}

/// The 3-stage war chain (ore → forge → INGOT → arms-town + aether → arms-town): the arms town consumes
/// INGOT + AETHER by Liebig, minting MANPOWER (ingot) + MANA (aether) + GOLD. Stations: 0 ore, 1 forge,
/// 2 arms town, 3 aether source.
fn run_war_chain(ticks: usize) -> World {
    let cells = vec![
        DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: ORE },
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 90.0, dest_w: 0.0, commodity: INGOT }, // forge makes INGOT
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: ORE }, // …from ORE
        DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 0.0, dest_w: 70.0, commodity: INGOT }, // arms town needs INGOT
        DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 0.0, dest_w: 70.0, commodity: AETHER }, // …+ AETHER
        DemandCell { x_mm: 3_000_000, y_mm: 1_500_000, origin_w: 90.0, dest_w: 2.0, commodity: AETHER }, // aether source
    ];
    let mut w = World::new(11, arcadia(cells));
    for (x, y) in [(0, 0), (1_500_000, 0), (3_000_000, 0), (3_000_000, 1_500_000)] {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
    }
    for (li, a, b) in [(0u32, 0u32, 1u32), (1, 1, 2), (2, 3, 2)] {
        w.apply(&Command::CreateLine { color: li + 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(li), station: StationId(a), after: None });
        w.apply(&Command::AddStop { line: LineId(li), station: StationId(b), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(li), spec: 0, count: 4 });
        w.apply(&Command::SetHeadway { line: LineId(li), headway_ms: 120_000 });
    }
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..ticks {
        w.tick(50);
    }
    w
}

#[test]
fn the_arms_chain_mints_manpower() {
    let w = run_war_chain(15000);
    assert!(w.manpower > 0, "the arms town consuming INGOT mints manpower: {}", w.manpower);
    assert!(w.mana > 0, "…and consuming AETHER mints mana: {}", w.mana);
    assert!(w.tribute > 0, "…and gold is minted too: {}", w.tribute);
}

#[test]
fn tech_is_bought_only_with_mana() {
    // ALL tech costs MANA (mana is the sole tech resource). A gold-only (ore) realm — even RICH in gold —
    // can't buy any tech; only an aether (mana) realm can. Aether is your science.
    assert_eq!(sim::tech::TECH_CHANNEL, Channel::Mana);
    let mut ore = run_supply(ORE, 12000);
    assert!(ore.tribute >= TECHS[FORGE_MASTERY].cost, "the ore realm is rich in gold…");
    assert_eq!(ore.mana, 0, "…but has no mana");
    let ev = ore.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "…so gold alone can't buy a tech");
    assert_eq!(ore.tech_unlocked, 0);

    let mut aether = run_supply(AETHER, 12000);
    assert!(aether.mana >= TECHS[FORGE_MASTERY].cost, "the aether realm earned mana: {}", aether.mana);
    let mana_before = aether.mana;
    let ev = aether.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    assert!(matches!(ev.as_slice(), [Event::TechUnlocked { .. }]), "mana buys tech: {ev:?}");
    assert_eq!(aether.mana, mana_before - TECHS[FORGE_MASTERY].cost, "exactly the cost is spent FROM MANA");
    assert!(sim::tech::is_unlocked(aether.tech_unlocked, FORGE_MASTERY), "FORGE_MASTERY is unlocked");
}

#[test]
fn a_tier_2_tech_needs_its_prerequisite() {
    // PRODUCTION_SURGE (tier 2) requires FORGE_MASTERY (its tier-1 spine) first, even with mana to spare.
    let mut w = run_supply(AETHER, 30000); // mana-rich
    assert!(w.mana >= TECHS[PRODUCTION_SURGE].cost + TECHS[FORGE_MASTERY].cost, "enough mana for both: {}", w.mana);
    let ev = w.apply(&Command::UnlockTech { tech: PRODUCTION_SURGE as u8 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "the tier-2 tech is rejected without its prereq");
    assert!(!sim::tech::is_unlocked(w.tech_unlocked, PRODUCTION_SURGE));
    // Unlock the spine, then the branch succeeds.
    w.apply(&Command::UnlockTech { tech: FORGE_MASTERY as u8 });
    let ev = w.apply(&Command::UnlockTech { tech: PRODUCTION_SURGE as u8 });
    assert!(matches!(ev.as_slice(), [Event::TechUnlocked { .. }]), "with the prereq, the branch unlocks: {ev:?}");
    assert!(sim::tech::is_unlocked(w.tech_unlocked, PRODUCTION_SURGE));
}

#[test]
fn the_split_economy_replays_bit_for_bit() {
    assert_eq!(run_war_chain(9000).state_hash(), run_war_chain(9000).state_hash(), "the split economy replays bit-for-bit");
}

/// V3: legions are fielded with MANPOWER, not gold. A barracks realm rich in GOLD (ore) fields NO legions;
/// one supplied with GRAIN (→ manpower) does. (Grain makes the army economy accessible without a forge.)
#[test]
fn legions_cost_manpower_not_gold() {
    fn realm(comm: u8) -> World {
        let mut w = World::new(
            11,
            arcadia(vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: comm },
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 90.0, commodity: comm },
            ]),
        );
        w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..15000 {
            w.tick(50);
        }
        w
    }
    let gold = realm(ORE);
    assert!(gold.tribute > 0, "the ore realm is rich in gold: {}", gold.tribute);
    assert_eq!(gold.armies.len(), 0, "gold alone fields NO legions (legions cost manpower)");
    let grain = realm(GRAIN);
    assert!(grain.manpower > 0, "the grain realm earned manpower: {}", grain.manpower);
    assert!(grain.armies.len() >= 1, "grain (manpower) fields legions: {}", grain.armies.len());
}

/// V3: posting a bounty costs GOLD (the treasury funds a decree). No gold ⇒ rejected; with gold ⇒ posts
/// and deducts the flat cost. (Clearing a bounty — amount 0 — is free, so it isn't blocked when broke.)
#[test]
fn bounties_cost_gold() {
    const BOUNTY_COST: i64 = 10; // mirrors world.rs
    let mut w = run_supply(ORE, 6000); // earns gold
    assert!(w.tribute >= BOUNTY_COST, "earned gold for a decree: {}", w.tribute);
    let before = w.tribute;
    let ev = w.apply(&Command::PostBounty { station: StationId(0), amount: 100 });
    assert!(matches!(ev.as_slice(), [Event::BountyPosted { .. }]), "with gold the bounty posts: {ev:?}");
    assert_eq!(w.tribute, before - BOUNTY_COST, "posting deducts the flat gold cost");

    // A gold-less realm can't post a bounty (but clearing is still free).
    let mut poor = World::new(11, arcadia(vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 1.0, dest_w: 1.0, commodity: ORE }]));
    poor.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    assert!(matches!(poor.apply(&Command::PostBounty { station: StationId(0), amount: 100 }).as_slice(), [Event::Rejected { .. }]), "no gold ⇒ bounty rejected");
    assert!(matches!(poor.apply(&Command::PostBounty { station: StationId(0), amount: 0 }).as_slice(), [Event::BountyPosted { .. }]), "clearing (0) is free");
}
