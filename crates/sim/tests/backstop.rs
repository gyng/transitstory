//! Off-rail goods BACKSTOP (#11) — peeps haul goods on foot so a CAPTURED town never fully starves, even
//! with no railway. Scoped to your territory (a town whose garrison conquest ground to 0) so it can't mint
//! free income from the whole map; a fraction of a rail load, so rail still wins. Gated on a baked rate (0
//! ⇒ rail-only ⇒ transit + goldens byte-identical, proven by the determinism/arcadia golden tests). Here we
//! prove the trickle reaches a captured, rail-less town + that rate 0 is inert + that it replays.
use sim::*;

fn backstop_world(rate: i64) -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 1,
        production_micro: 10,
        walk_backstop_micro: rate,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 1 }, // GRAIN source
                DemandCell { x_mm: 2_000_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 1 }, // GRAIN town (sink)
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // grain source
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None }); // grain town (a sink)
    // CAPTURE the town (siege ground its garrison to 0) — the backstop only feeds ground you hold.
    while w.town_value.len() <= 1 {
        w.town_value.push(1000);
    }
    w.town_value[1] = 0;
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Run N 50ms ticks. NO line is ever built, so the ONLY way the town gains supply is the off-rail backstop.
fn run_ticks(w: &mut World, n: usize) {
    for _ in 0..n {
        w.tick(50);
    }
}

#[test]
fn a_captured_railless_town_gets_a_trickle_of_tribute() {
    let mut w = backstop_world(200);
    run_ticks(&mut w, 4000); // long enough for the slow walk to roll whole units → consume → tribute
    assert!(w.tribute > 0, "a captured town with a sourced commodity should trickle tribute on foot ({})", w.tribute);
}

#[test]
fn backstop_off_yields_nothing_without_rail() {
    let mut w = backstop_world(0); // rate 0 ⇒ no backstop
    run_ticks(&mut w, 4000);
    assert_eq!(w.tribute, 0, "no rail + no backstop ⇒ the town starves, zero tribute");
}

#[test]
fn an_unconquered_town_gets_no_backstop() {
    // Same world, but DON'T capture the town (town_value stays > 0): the backstop can't mint free income
    // from territory you don't hold — you must rail to it or conquer it first.
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 1,
        production_micro: 10,
        walk_backstop_micro: 200,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 1 },
                DemandCell { x_mm: 2_000_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 1 },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None });
    w.apply(&Command::SetRunning { running: true });
    run_ticks(&mut w, 4000);
    assert_eq!(w.tribute, 0, "a neutral (un-captured) town yields no backstop income");
}

#[test]
fn rail_dwarfs_the_backstop() {
    // A rail-served captured town earns FAR more than the same town on the backstop alone — the railway
    // industrialises the walking trade (the design promise: rail still wins).
    let mut walk = backstop_world(200);
    run_ticks(&mut walk, 4000);

    let mut rail = backstop_world(200);
    rail.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    rail.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    rail.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    rail.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    rail.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
    run_ticks(&mut rail, 4000);
    assert!(rail.tribute > walk.tribute * 3, "rail ({}) should dwarf the foot-trickle ({})", rail.tribute, walk.tribute);
}

#[test]
fn backstop_replays_deterministically() {
    let run = || {
        let mut w = backstop_world(250);
        run_ticks(&mut w, 6000);
        w.state_hash()
    };
    assert_eq!(run(), run(), "an off-rail backstop replays bit-for-bit");
}
