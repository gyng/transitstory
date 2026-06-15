//! Terrain-aware build cost (#terrain) + the fantasy GOLD build economy (#economy). Laying rail through
//! rough country costs more (route around the ridge or pay), and in arcadia building/extending SPENDS gold
//! from the realm treasury (afford-gated) — so a build is an ROI decision, not a free click. Both are
//! golden-neutral by construction (PLAIN/transit classes stay ×1; the gold charge is off unless a baked
//! `build_gold_divisor` > 0), proven by the determinism/arcadia golden tests; here we prove the behaviour.
use sim::*;

/// A world whose buildability grid tags a horizontal corridor (cells q=0..=5 at r∈{-1,0,1}) with `c`.
fn world_with_corridor(c: u8, ruleset: &str, initial_gold: i64, build_gold_divisor: i64) -> World {
    let mut cells = Vec::new();
    if c != 0 {
        for q in 0..=5i64 {
            for r in -1..=1i64 {
                cells.push(BuildCell { x_mm: q * 1_000_000, y_mm: r * 1_000_000, c });
            }
        }
    }
    let city = CityData {
        id: "t".into(),
        ruleset: ruleset.into(),
        seed: 1,
        initial_gold,
        build_gold_divisor,
        buildability: BuildabilityGrid { cell_m: 1000.0, cells },
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 10.0, dest_w: 10.0, commodity: 0 }],
        },
        ..Default::default()
    };
    World::new(7, city)
}

/// Build a 5 km horizontal line (stations at x=0 and x=5_000_000) and return its capital cost.
fn line_capital(c: u8) -> i64 {
    let mut w = world_with_corridor(c, "transit", 0, 0);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.lines[0].capital_cost
}

#[test]
fn terrain_makes_rough_country_cost_more() {
    use sim::city::biome;
    let plain = line_capital(biome::PLAIN);
    let open = line_capital(0); // no grid ⇒ OPEN ⇒ the ×1 baseline
    let forest = line_capital(biome::FOREST);
    let hill = line_capital(biome::HILL);
    let mountain = line_capital(biome::MOUNTAIN);
    assert!(plain > 0, "the line has track to cost");
    assert_eq!(plain, open, "PLAIN is the ×1 baseline — byte-identical to OPEN (golden-neutral guarantee)");
    assert!(forest > plain, "forest (×1.4) costs more than plain");
    assert!(hill > forest, "hills (×1.9) cost more than forest");
    assert!(mountain > hill, "mountains (×3.2) cost the most");
    // Sanity on the multiplier magnitude (allow for per-segment integer truncation).
    assert!(mountain >= plain * 3 && mountain <= plain * 33 / 10, "mountain ≈ ×3.2 of plain (got {mountain} vs {plain})");
}

#[test]
fn arcadia_building_spends_gold_from_the_treasury() {
    // Generous treasury, gold-cost on: a build succeeds and the treasury drops by the line's gold price.
    let mut w = world_with_corridor(sim::city::biome::PLAIN, "arcadia", 1_000_000, 1_000);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    let before = w.tribute;
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None }); // 1 stop, no segment ⇒ free
    assert_eq!(w.tribute, before, "the first stop lays no track ⇒ no charge");
    let evs = w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None }); // the segment
    assert!(!evs.iter().any(|e| matches!(e, Event::Rejected { .. })), "an affordable extension commits");
    assert!(w.tribute < before, "building the segment spent gold from the treasury (before={before}, now={})", w.tribute);
    assert_eq!(w.lines[0].stops.len(), 2, "the stop attached");
}

#[test]
fn arcadia_rejects_a_build_it_cannot_afford() {
    // A treasury of 1 gold can't pay for a 5 km line: the extension is rejected and the treasury is untouched.
    let mut w = world_with_corridor(sim::city::biome::PLAIN, "arcadia", 1, 1_000);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    let evs = w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    assert!(evs.iter().any(|e| matches!(e, Event::Rejected { .. })), "a build beyond the treasury is rejected");
    assert_eq!(w.tribute, 1, "a rejected build spends nothing");
    assert_eq!(w.lines[0].stops.len(), 1, "the unaffordable stop did not attach");
}

#[test]
fn divisor_zero_keeps_building_free() {
    // build_gold_divisor 0 (the default) ⇒ no gold cost ⇒ building never touches the treasury (golden-neutral).
    let mut w = world_with_corridor(sim::city::biome::PLAIN, "arcadia", 50, 0);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    let evs = w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    assert!(!evs.iter().any(|e| matches!(e, Event::Rejected { .. })), "free building always commits");
    assert_eq!(w.tribute, 50, "with no divisor, building is free — treasury untouched");
}
