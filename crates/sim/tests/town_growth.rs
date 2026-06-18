//! #23 TG2 — the town-growth COUPLINGS. A grown town FORTIFIES (its siege HP rises with size, tracked as it
//! grows) and FREEZES on capture (size + garrison never move once taken). The growth driver + golden-
//! neutrality are pinned by the (war-less) goldens; winnability by the balance gate; here we pin the coupling
//! arithmetic directly — GARRISON_PER_SIZE = 150, MAX_SIZE = 5. The one-time conquest BOUNTY fires at the
//! siege grind→flip and is exercised by balance.rs (which captures towns); its arithmetic is the same shape.
use sim::city::CityData;
use sim::world::World;

fn arcadia_world() -> World {
    World::new(7, CityData { ruleset: "arcadia".into(), ..Default::default() })
}

#[test]
fn growing_a_town_fortifies_its_garrison() {
    let mut w = arcadia_world();
    // a live town (a sink): town_value sized to a base garrison, size 0 (as siege() seeds it on tick 1).
    w.town_value = vec![500];
    w.town_size = vec![0];
    w.town_growth_accum = vec![0];
    w.grow_town(0, 2_400); // 2 × UNITS_PER_SIZE worth of supply delivered
    assert_eq!(w.town_size[0], 2, "two size steps from the delivered supply");
    assert_eq!(w.town_value[0], 500 + 150 * 2, "garrison HP rises +GARRISON_PER_SIZE per size gained (a fed town fortifies)");
}

#[test]
fn growth_and_fortification_cap_at_max_size() {
    let mut w = arcadia_world();
    w.town_value = vec![500];
    w.town_size = vec![4];
    w.town_growth_accum = vec![0];
    w.grow_town(0, 6_000); // 5 steps' worth, but size caps at MAX_SIZE=5 (real gain = 1)
    assert_eq!(w.town_size[0], 5, "size caps at MAX_SIZE");
    assert_eq!(w.town_value[0], 500 + 150, "garrison rises only by the CAPPED gain (+1 size), not the raw steps");
}

#[test]
fn a_captured_town_freezes() {
    let mut w = arcadia_world();
    w.town_value = vec![0]; // captured (the conquest sentinel)
    w.town_size = vec![3];
    w.town_growth_accum = vec![0];
    w.grow_town(0, 10_000); // a flood of supply
    assert_eq!(w.town_size[0], 3, "a captured town (town_value==0) FREEZES — size never moves");
    assert_eq!(w.town_value[0], 0, "and never re-fortifies (stays the captured sentinel)");
}
