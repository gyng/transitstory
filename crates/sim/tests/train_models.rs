//! Depot rework, Stage 1 — the RAIL TRAIN-MODEL catalog. A line buys a model via `AssignTrainset{spec}`:
//! Standard (the default), Heavy (bulk capacity, slower + pricier), or Express (fast + cheap, light). The
//! default (spec 0) is byte-identical to the shipped metro + cost, so existing saves + both golden fixtures
//! replay bit-for-bit (proven by the determinism/arcadia golden tests; the model tradeoffs are proven here).
use sim::trainset::{self, tmode, RAIL_ROSTER, SPECS};
use sim::*;

#[test]
fn rail_roster_default_is_byte_identical_to_the_shipped_metro() {
    // The golden lock: spec 0 must resolve to exactly SPECS[0] (capacity/speed/dwell/length) so a
    // default route's vehicles + state hash are unchanged.
    let def = trainset::spec_for(tmode::RAIL, 0);
    let metro = SPECS[0];
    assert_eq!(def.capacity, metro.capacity);
    assert_eq!(def.v_max_mm_s, metro.v_max_mm_s);
    assert_eq!(def.accel_mm_s2, metro.accel_mm_s2);
    assert_eq!(def.decel_mm_s2, metro.decel_mm_s2);
    assert_eq!(def.dwell_ms, metro.dwell_ms);
    assert_eq!(def.length_mm, metro.length_mm);
    // RAIL_COST[0] must equal the flat per-train cost so default capital is byte-identical.
    assert_eq!(trainset::train_cost(tmode::RAIL, 0, 15_000_000), 15_000_000);
    // …and every other mode keeps the flat cost regardless of spec id.
    assert_eq!(trainset::train_cost(tmode::HEAVY, 2, 15_000_000), 15_000_000);
}

#[test]
fn the_three_rail_models_are_a_real_tradeoff() {
    assert_eq!(RAIL_ROSTER.len(), 3);
    let std = trainset::spec_for(tmode::RAIL, 0);
    let heavy = trainset::spec_for(tmode::RAIL, 1);
    let express = trainset::spec_for(tmode::RAIL, 2);
    // Heavy hauls more but is slower; Express is faster but lighter — no model strictly dominates.
    assert!(heavy.capacity > std.capacity, "Heavy carries more");
    assert!(heavy.v_max_mm_s < std.v_max_mm_s, "Heavy is slower");
    assert!(express.v_max_mm_s > std.v_max_mm_s, "Express is faster");
    assert!(express.capacity < std.capacity, "Express is lighter");
    // Cost tracks the tradeoff: Heavy pricier, Express cheaper.
    assert!(trainset::train_cost(tmode::RAIL, 1, 15_000_000) > 15_000_000, "Heavy costs more to buy");
    assert!(trainset::train_cost(tmode::RAIL, 2, 15_000_000) < 15_000_000, "Express costs less");
    // Out-of-range spec ids clamp (never panic).
    assert_eq!(trainset::spec_for(tmode::RAIL, 99).capacity, express.capacity);
}

/// Build a 2-train RAIL line on the given model and read its capital cost (afford-gate off → no rejection).
fn line_capital_with_model(spec: u8) -> i64 {
    let city = CityData {
        id: "t".into(),
        seed: 1,
        demand: DemandGrid { cell_m: 500.0, cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 10.0, dest_w: 10.0, commodity: 0 }] },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec, count: 2 });
    w.lines[0].capital_cost
}

#[test]
fn choosing_a_model_changes_the_lines_build_cost() {
    let std = line_capital_with_model(0);
    let heavy = line_capital_with_model(1);
    let express = line_capital_with_model(2);
    // Heavy's 2 trains cost 2×(27−15)=24M more than Standard; Express 2×(15−11)=8M less. Track cost is shared.
    assert_eq!(heavy - std, 2 * (27_000_000 - 15_000_000), "Heavy rolling stock costs more");
    assert_eq!(std - express, 2 * (15_000_000 - 11_000_000), "Express rolling stock costs less");
}

#[test]
fn assigning_models_replays_deterministically() {
    let run = || {
        let mut w = {
            let city = CityData { id: "t".into(), seed: 3, demand: DemandGrid { cell_m: 500.0, cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 30.0, dest_w: 30.0, commodity: 0 }, DemandCell { x_mm: 3_000_000, y_mm: 0, origin_w: 30.0, dest_w: 30.0, commodity: 0 }] }, ..Default::default() };
            World::new(9, city)
        };
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None });
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 1, count: 3 }); // Heavy
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 60_000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..400 { w.tick(50); }
        w
    };
    assert_eq!(run().state_hash(), run().state_hash(), "a Heavy-model line replays bit-for-bit");
}
