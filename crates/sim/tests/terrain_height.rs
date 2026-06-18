//! #15 terrain HEIGHT — the gameplay LEVER (a climb-cost surcharge + a valley-preferring router, both keyed
//! on a discrete height band derived from the baked biome byte). Two load-bearing invariants:
//!   • GOLDEN-NEUTRAL on the flat: band 0 (PLAIN + every transit class) ⇒ climb == 0 ⇒ exact integer identity,
//!     so a flat line costs the byte-for-byte same as before #15 (full byte-identity of every existing fixture
//!     is pinned by determinism.rs/arcadia.rs; this pins the focused unit).
//!   • MONOTONE: a route over higher/rougher ground never costs LESS than the flat one — the lever points the
//!     right way (route around the ridge, or pay to cross it). docs/terrain-height.md §4.
use sim::*;

/// Capital cost of a straight line laid across a corridor whose cells (q = 0..bands.len() at r ∈ {-1,0,1}) are
/// tagged by `bands` (empty ⇒ no grid ⇒ OPEN/band 0). Per-cell biomes ⇒ the straight line CLIMBS between
/// bands. "transit" ruleset so the polyline runs straight through the tagged cells (mirrors build_economy.rs).
fn corridor_world(bands: &[u8]) -> World {
    let mut cells = Vec::new();
    for (q, &c) in bands.iter().enumerate() {
        for r in -1..=1i64 {
            cells.push(BuildCell { x_mm: q as i64 * 1_000_000, y_mm: r * 1_000_000, c });
        }
    }
    let city = CityData {
        id: "t".into(),
        ruleset: "transit".into(),
        buildability: BuildabilityGrid { cell_m: 1000.0, cells },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    // The line is ALWAYS the same 5 km span (cells q=0..=5); `bands` only tags the terrain it crosses (empty ⇒
    // all OPEN). A fixed length so OPEN and a tagged corridor are compared over identical geometry.
    let end = 5_000_000;
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: end, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w
}

fn corridor_capital(bands: &[u8]) -> i64 {
    corridor_world(bands).lines[0].capital_cost
}

/// The SLOWEST per-vertex speed cap on the line — the climb's grade cap if any span climbs (Effect B).
fn corridor_min_speed_cap(bands: &[u8]) -> i64 {
    corridor_world(bands).lines[0].paths[0].speed_cap_mm_s.iter().copied().min().unwrap_or(i64::MAX)
}

#[test]
fn flat_terrain_adds_no_height_cost() {
    use sim::city::biome;
    // PLAIN (band 0) must equal OPEN (no grid): the climb surcharge + router tie-break are exact no-ops on the
    // flat — the golden-neutrality guarantee in test form.
    let open = corridor_capital(&[]);
    let plain = corridor_capital(&[biome::PLAIN; 6]);
    assert!(plain > 0, "the line has track to cost");
    assert_eq!(open, plain, "flat PLAIN must cost exactly the OPEN baseline (height adds nothing on band 0)");
}

#[test]
fn climbing_a_ridge_never_costs_less_than_the_plain() {
    use sim::city::biome;
    // Equal-length routes; one crosses a HILL→MOUNTAIN→HILL ridge (climbs), one stays on PLAIN. The ridge route
    // pays the biome multiplier AND the #15 climb surcharge ⇒ strictly dearer. The decision the owner asked
    // for: route around the high ground, or pay to cross it.
    let plain = corridor_capital(&[biome::PLAIN; 6]);
    let ridge = corridor_capital(&[
        biome::PLAIN, biome::PLAIN, biome::HILL, biome::MOUNTAIN, biome::HILL, biome::PLAIN,
    ]);
    assert!(ridge > plain, "crossing a ridge must cost MORE than the flat route (ridge {ridge} vs plain {plain})");
}

#[test]
fn climbing_caps_speed_below_the_flat_route() {
    use sim::city::biome;
    // Effect B: a line climbing PLAIN→HILL→MOUNTAIN has its ascent spans speed-capped, so its slowest span is
    // slower than a flat PLAIN line's. The route-for-speed lever: a valley line runs faster than one over the
    // ridge. (Flat stays uncapped ⇒ golden-neutral.)
    let flat = corridor_min_speed_cap(&[biome::PLAIN; 6]);
    let climb = corridor_min_speed_cap(&[
        biome::PLAIN, biome::PLAIN, biome::HILL, biome::MOUNTAIN, biome::MOUNTAIN, biome::MOUNTAIN,
    ]);
    assert!(climb < flat, "climbing into the mountains must cap speed BELOW the flat route (climb {climb} vs flat {flat})");
}
