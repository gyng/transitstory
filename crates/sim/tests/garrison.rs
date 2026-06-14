//! S11 FRONTIER GARRISONS — a town's siege resistance scales with its depth in the decadence frontier
//! (the design's gate-safe Tier-1-LITE "static town garrison": no mobile AI, so none of the rival's
//! livelock/sawtooth risk). A town deep in the rot defends harder than one by the capital — the expansion
//! arc grades from soft to hard. STATIC + deterministic; ZERO bonus without a decadence field (transit /
//! demo arcadia), so it is golden-neutral by construction. Tested through the SIEGE + stats path (the
//! garrison surfaces as `per_station.town_resistance`), never by poking internals.
use sim::hexgrid::{self, Axial};
use sim::*;

const SIZE: i64 = 250_000;

/// A synthetic baked-like arcadia world: a `w × h` hex block of PLAIN(10) cells with a `capital` cell, on
/// the bake's lattice + transform — so `DecadenceField::build` yields a real frontier gradient.
fn hex_world(w: i64, h: i64, capital: Axial) -> CityData {
    let mut cells = Vec::new();
    for q in 0..w {
        for r in 0..h {
            let p = hexgrid::center_of((q, r), SIZE);
            cells.push(BuildCell { x_mm: p.x_mm, y_mm: p.y_mm, c: 10 });
        }
    }
    let cap = hexgrid::center_of(capital, SIZE);
    CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12,
        grid_cell_mm: SIZE,
        capital_x_mm: cap.x_mm,
        capital_y_mm: cap.y_mm,
        buildability: BuildabilityGrid { cell_m: SIZE as f64 / 1000.0, cells },
        ..Default::default()
    }
}

/// Read a station's current garrison (remaining siege resistance) from the stats snapshot.
fn garrison(w: &World, station: usize) -> f64 {
    w.stats_snapshot()
        .per_station
        .iter()
        .find(|s| s.station_id == station as u32)
        .map(|s| s.town_resistance)
        .unwrap_or(-1.0)
}

/// Place a station at hex cell `(q, r)`, then size the garrison by running one war tick (siege lazily
/// sizes `town_value` to each station's frontier resistance; with no besieging legion it stays put).
fn world_with_towns(cells: &[Axial]) -> World {
    let mut w = World::new(12, hex_world(12, 12, (0, 0)));
    for &(q, r) in cells {
        let p = hexgrid::center_of((q, r), SIZE);
        w.apply(&Command::PlaceStation { x_mm: p.x_mm, y_mm: p.y_mm, name: None });
    }
    w.apply(&Command::SetRunning { running: true });
    w.tick(50); // war_step → siege sizes town_value to the frontier garrison
    w
}

#[test]
fn a_frontier_town_garrisons_harder_than_one_by_the_capital() {
    // A town one ring out vs one at the far edge of a 12×12 frontier (capital at (0,0)).
    let w = world_with_towns(&[(1, 0), (11, 11)]);
    let near = garrison(&w, 0);
    let far = garrison(&w, 1);
    assert!(near >= 500.0, "every town defends at least the base resistance (near {near})");
    assert!(far > near, "a deeper-frontier town garrisons harder: far {far} > near {near}");
    assert!(far <= 1000.0, "the bonus is capped at base + GARRISON_MAX (far {far})");
}

#[test]
fn no_decadence_field_means_flat_resistance() {
    // The demo arcadia world (no buildability ⇒ no field) garrisons every town at the FLAT base — the
    // golden-neutral guarantee (the demo + golden fixtures are unchanged; no re-pin).
    let mut w = World::new(
        11,
        CityData {
            id: "arcadia".into(),
            ruleset: "arcadia".into(),
            seed: 11,
            grid_cell_mm: 100_000,
            demand: DemandGrid { cell_m: 500.0, cells: vec![] },
            ..Default::default()
        },
    );
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 9_000_000, y_mm: 9_000_000, name: None });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);
    assert_eq!(garrison(&w, 0), 500.0, "no field ⇒ flat base resistance");
    assert_eq!(garrison(&w, 1), 500.0, "no field ⇒ flat base resistance, regardless of position");
}

#[test]
fn a_max_garrison_town_is_still_conquerable() {
    // Even the stiffest frontier garrison falls to a supplied war — conquest stays POSSIBLE (the bonus is
    // a difficulty curve, not a wall). A supply chain near the capital earns tribute; the barracks fields
    // legions that march to the far-edge town (bountied) + grind down its max garrison.
    let cap = hexgrid::center_of((0, 0), SIZE);
    let src = hexgrid::center_of((2, 0), SIZE); // ore source (supply)
    let sink = hexgrid::center_of((4, 0), SIZE); // a near sink the supply feeds → tribute
    let far = hexgrid::center_of((11, 11), SIZE); // the conquest target (deep frontier ⇒ max garrison)
    // The hex frontier (for the field) PLUS a supply demand grid (for tribute).
    let mut city = hex_world(12, 12, (0, 0));
    city.demand = DemandGrid {
        cell_m: 500.0,
        cells: vec![
            DemandCell { x_mm: src.x_mm, y_mm: src.y_mm, origin_w: 90.0, dest_w: 2.0, commodity: 1 }, // GRAIN (V3: → MANPOWER → legions)
            DemandCell { x_mm: sink.x_mm, y_mm: sink.y_mm, origin_w: 2.0, dest_w: 90.0, commodity: 1 },
        ],
    };
    let mut w = World::new(12, city);
    w.apply(&Command::PlaceBarracks { x_mm: cap.x_mm, y_mm: cap.y_mm, name: None }); // 0 capital-barracks
    w.apply(&Command::PlaceStation { x_mm: src.x_mm, y_mm: src.y_mm, name: None }); // 1 ore source
    w.apply(&Command::PlaceStation { x_mm: sink.x_mm, y_mm: sink.y_mm, name: None }); // 2 near sink
    w.apply(&Command::PlaceStation { x_mm: far.x_mm, y_mm: far.y_mm, name: None }); // 3 far town (max garrison)
    // Supply line: source → sink (earns tribute, no barracks ⇒ no launch from it).
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    // Conquest line: barracks → far town (legions launch here, march, besiege); the bounty steers them.
    w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(1), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(1), station: StationId(3), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(1), spec: 0, count: 2 });
    w.apply(&Command::SetHeadway { line: LineId(1), headway_ms: 120_000 });
    w.apply(&Command::PostBounty { station: StationId(3), amount: 5000 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..120_000 {
        w.tick(50);
        if w.towns_captured >= 1 {
            break;
        }
    }
    assert!(w.towns_captured >= 1, "a max-garrison frontier town is still conquerable by a sustained war");
}

#[test]
fn the_garrison_is_deterministic() {
    // The frontier garrison is a pure read of the static field ⇒ identical across two builds.
    let a = world_with_towns(&[(1, 0), (6, 3), (11, 11)]);
    let b = world_with_towns(&[(1, 0), (6, 3), (11, 11)]);
    assert_eq!(a.state_hash(), b.state_hash(), "frontier garrisons replay bit-for-bit");
}
