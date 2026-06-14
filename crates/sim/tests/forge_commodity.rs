//! S7e — multi-commodity Forge-Line. A source produces ITS commodity (the dominant origin-commodity of
//! its captured cells), the cart CARRIES that commodity, and the sink receives + consumes it into tribute.
//! The single-commodity (ORE / commodity 0) path is byte-identical (the goldens stay pinned) — these tests
//! prove a NON-ORE commodity (GRAIN) flows produce→ship→deliver→consume end-to-end, deterministically.
use sim::*;

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

const GRAIN: u8 = 1;
const FUEL: u8 = 3;
const ORE: u8 = 0;
const AETHER: u8 = 2;
const INGOT: u8 = 4; // a MID good (>= forge::FIRST_MID): a PROCESSOR forges it from ore

/// Build a BREAD town (recipe = grain+fuel: two dest cells tagged grain & fuel at 1.5 Mm) fed by a grain
/// source (at 0) and — if `with_fuel` — a fuel source (at 3 Mm), all on one line. The town consumes by
/// LIEBIG: tribute = min(grain, fuel) delivered, so it scores only when BOTH chains reach it.
fn run_bread_town(with_fuel: bool, ticks: usize) -> World {
    let mut cells = vec![
        DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: GRAIN }, // grain source
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: GRAIN }, // town needs grain…
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: FUEL }, // …and fuel (BREAD)
    ];
    if with_fuel {
        cells.push(DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: FUEL }); // fuel source
    }
    let mut w = World::new(11, arcadia(cells));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    if with_fuel {
        w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    if with_fuel {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..ticks {
        w.tick(50);
    }
    w
}

#[test]
fn liebig_a_bread_town_needs_both_grain_and_fuel() {
    // BOTH chains reach the town ⇒ Liebig min(grain,fuel) > 0 ⇒ tribute flows.
    let fed = run_bread_town(true, 8000);
    assert_eq!(fed.station_recipe[1], vec![GRAIN, FUEL], "the town requires BREAD = grain + fuel");
    assert!(fed.tribute > 0, "a town supplied BOTH grain and fuel produces bread → tribute: {}", fed.tribute);

    // Only grain reaches it (no fuel source) ⇒ Liebig is throttled to min(grain, 0) = 0 ⇒ NO tribute,
    // even though grain is delivered: you MUST build both chains. The disjoint-chain pressure has teeth.
    let starved = run_bread_town(false, 8000);
    assert_eq!(starved.tribute, 0, "grain alone yields no bread (the missing fuel throttles output): {}", starved.tribute);
}

/// Build a GRAIN source + a sink (kept > the ~500 m catchment apart so the chain actually flows), connect
/// them with a line, and run. Returns the finished world.
fn run_grain(ticks: usize) -> World {
    let mut w = World::new(
        11,
        arcadia(vec![
            DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: GRAIN }, // GRAIN source
            DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 0 }, // sink
        ]),
    );
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..ticks {
        w.tick(50);
    }
    w
}

#[test]
fn a_grain_source_produces_ships_and_delivers_grain() {
    let w = run_grain(6000);
    // The source's OUTPUT commodity is GRAIN (derived from its captured origin cells), not ORE.
    assert_eq!(w.station_commodity[0], GRAIN, "the source produces its dominant origin-commodity (GRAIN)");
    // GRAIN flowed source→cart→sink→tribute: the supply loop closes on a NON-ORE commodity. (The source
    // ONLY produces GRAIN — commodity 1 — so any tribute is proof GRAIN was produced, shipped, delivered,
    // and consumed; the ORE slot 0 at the source is never touched.)
    assert!(w.tribute > 0, "GRAIN flowed produce→ship→deliver→consume into tribute: {}", w.tribute);
}

#[test]
fn multi_commodity_flow_replays_bit_for_bit() {
    // The multi-commodity race is deterministic (same seed + log ⇒ identical state_hash, twice).
    assert_eq!(run_grain(5000).state_hash(), run_grain(5000).state_hash(), "GRAIN flow replays bit-for-bit");
}

/// S7e MULTI-STAGE (raw → mid → final): a 3-stage war chain. ORE is mined at a source, FORGED into INGOT
/// at a PROCESSOR node (consumes ore → makes ingot → ships it on), and an ARMS town consumes INGOT +
/// AETHER by Liebig → tribute. There is NO ingot source, so tribute proves the processor actually
/// converted ore → ingot AND commodity-aware routing carried the ore to the forge (not past it to the
/// town). Stations: 0=ore source, 1=forge, 2=arms town, 3=aether source.
fn run_multistage(ticks: usize) -> World {
    run_multistage_cfg(ticks, 0.0)
}
/// `forge_ingot_dest` tags the forge's own OUTPUT (INGOT) with this much DEST weight — 0 is clean
/// authoring; a positive value is the self-recipe footgun (the forge "wants" the good it makes). The
/// engine must forge regardless (it excludes the self-commodity from a processor's inputs).
fn run_multistage_cfg(ticks: usize, forge_ingot_dest: f32) -> World {
    let cells = vec![
        DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: ORE }, // ore source (A)
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 90.0, dest_w: forge_ingot_dest, commodity: INGOT }, // forge (B) makes INGOT…
        DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: ORE }, // …from ORE shipped in
        DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 0.0, dest_w: 70.0, commodity: INGOT }, // arms town (C) needs INGOT…
        DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 0.0, dest_w: 70.0, commodity: AETHER }, // …+ AETHER (Liebig)
        DemandCell { x_mm: 3_000_000, y_mm: 1_500_000, origin_w: 90.0, dest_w: 2.0, commodity: AETHER }, // aether source (D)
    ];
    let mut w = World::new(11, arcadia(cells));
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // 0 ore source
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None }); // 1 forge
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None }); // 2 arms town
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 1_500_000, name: None }); // 3 aether source
    // The player wires the chain: ore A→B, ingot B→C, aether D→C.
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
fn multistage_ore_is_forged_to_ingot_then_consumed() {
    let w = run_multistage(15000);
    assert!(w.has_multistage, "the world uses a processed good (INGOT) ⇒ commodity-aware routing is active");
    assert_eq!(w.station_commodity[1], INGOT, "the forge's output commodity is INGOT (a processor, not a raw source)");
    assert_eq!(w.station_recipe[2], vec![AETHER, INGOT], "the arms town requires AETHER + INGOT (Liebig)");
    // INGOT has no source — tribute can only flow if the forge converted ore → ingot and shipped it on,
    // AND the ore reached the forge (commodity-aware routing) rather than over-shooting to the town.
    assert!(w.tribute > 0, "the 3-stage chain ore→INGOT→arms-town closed → tribute: {}", w.tribute);
}

#[test]
fn multistage_flow_replays_bit_for_bit() {
    assert_eq!(run_multistage(9000).state_hash(), run_multistage(9000).state_hash(), "the multi-stage flow replays bit-for-bit");
}

#[test]
fn a_processor_never_consumes_its_own_output() {
    // Robustness (footgun guard): tag the forge with DEST weight of its OWN output (INGOT) — so INGOT is
    // in its derived recipe. A naive Liebig over the recipe would min in the (initially 0) output buffer
    // and DEADLOCK (make=0 forever). The engine excludes the self-commodity from a processor's inputs, so
    // the forge still converts ore → ingot and the chain closes.
    let w = run_multistage_cfg(15000, 60.0);
    assert!(w.station_recipe[1].contains(&INGOT), "the forge's recipe DOES include its own output (the footgun)");
    assert!(w.tribute > 0, "the processor still forges despite a self-recipe (no deadlock): {}", w.tribute);
}
