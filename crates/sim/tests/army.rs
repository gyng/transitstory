//! S8a — the war machine's separate army SoA. Legions are AI-launched (funded by tribute), ride the
//! rail network, and march deterministically in their OWN SoA — the binding condition: `dispatch`'s
//! `v.clear()` (every `SetHeadway`) rebuilds the shared `VehicleSoA` and would teleport a legion if it
//! lived there. These tests prove an army launches, marches deterministically, and is UNTOUCHED by a
//! `SetHeadway`. Siege/flip/targeting (S8b) layer on this.
use sim::*;

/// An arcadia world that accrues tribute (so the war machine can fund a legion) on a built route.
fn war_world() -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 0 }, // source (funds tribute)
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 0 }, // town (consumes→tribute)
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    // Station 0 is a BARRACKS (the launch origin) sitting at the supply source; station 1 is the
    // far-end town the legions march on. A barracks on a built route is the prerequisite for war.
    w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

/// Tribute funds a legion: once enough supply is consumed into tribute, the war machine fields an army
/// that then marches along the route (its arc-length position advances). Deterministic.
#[test]
fn tribute_funds_a_marching_legion() {
    let run = || {
        let mut w = war_world();
        for _ in 0..6000 {
            w.tick(50); // long enough to accrue tribute, launch, and march
        }
        w
    };
    let w = run();
    assert!(w.armies.len() >= 1, "tribute should have funded at least one legion (armies={})", w.armies.len());
    assert!(w.armies.s_mm[0] > 0, "the legion marched along its route (s_mm advanced past the start)");
    assert_eq!(run().state_hash(), run().state_hash(), "the war machine replays bit-for-bit");
}

/// THE binding-condition gate-blind test (fantasy-build-plan.md #2): a `SetHeadway` rebuilds the shared
/// `VehicleSoA` (`dispatch::v.clear()`). A marching legion lives in a SEPARATE SoA, so its `s_mm` must
/// be UNTOUCHED by that rebuild — proving armies aren't a `kind` byte in the vehicle SoA.
#[test]
fn legion_position_survives_a_set_headway() {
    let mut w = war_world();
    // March a legion out onto the route.
    for _ in 0..6000 {
        w.tick(50);
    }
    assert!(w.armies.len() >= 1, "a legion is marching");
    let before = w.armies.s_mm[0];
    assert!(before > 0, "the legion is mid-route");
    // A SetHeadway forces a full vehicle re-dispatch (v.clear() + rebuild). If the army shared that
    // SoA, this would teleport it; in its own SoA it is untouched.
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 90_000 });
    assert_eq!(w.armies.s_mm[0], before, "a SetHeadway's v.clear() must NOT move a legion (separate SoA)");
}

/// S8b — the conquest loop: legions march to the far-end town, besiege it, and FLIP it. Over a long
/// run MANY legions launch (tribute keeps funding them) and all target that one town, so this also
/// exercises the gate-blind EXACTLY-ONCE property: a town is captured ONCE, not re-counted each tick a
/// later legion garrisons it.
#[test]
fn war_machine_captures_town_exactly_once() {
    let run = || {
        let mut w = war_world();
        for _ in 0..12000 {
            w.tick(50); // long enough to launch many legions, march, besiege, and flip the town
        }
        w
    };
    let w = run();
    assert!(w.armies.len() > 1, "many legions launched (so the exactly-once guard is actually exercised)");
    assert_eq!(w.town_value[1], 0, "the far-end town's resistance was ground to 0 (it fell)");
    assert_eq!(
        w.towns_captured, 1,
        "the town is captured EXACTLY ONCE despite multiple besieging legions (got {})",
        w.towns_captured
    );
    assert_eq!(run().state_hash(), run().state_hash(), "the conquest loop replays bit-for-bit");
}

/// The Majesty STEERING lever (S8): a bounty posted on a MID-route town pulls a legion to besiege it —
/// something that never happens by default (legions march to the route's FAR end). So `town_value[mid]
/// == 0` proves the bounty redirected the AI. The player baits armies; they don't command them.
#[test]
fn a_bounty_steers_a_legion_to_a_mid_route_town() {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 9,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: 0 }, // barracks + source
                DemandCell { x_mm: 1_000_000, y_mm: 0, origin_w: 2.0, dest_w: 40.0, commodity: 0 }, // MID town (bountied)
                DemandCell { x_mm: 2_000_000, y_mm: 0, origin_w: 2.0, dest_w: 40.0, commodity: 0 }, // FAR town (default target)
            ],
        },
        ..Default::default()
    };
    let build = || {
        let mut w = World::new(9, city.clone());
        w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None }); // st0
        w.apply(&Command::PlaceStation { x_mm: 1_000_000, y_mm: 0, name: None }); // st1 (mid)
        w.apply(&Command::PlaceStation { x_mm: 2_000_000, y_mm: 0, name: None }); // st2 (far)
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 4 });
        w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
        // Bait legions to the MID town — without this they'd march past it to st2.
        w.apply(&Command::PostBounty { station: StationId(1), amount: 1000 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..12000 {
            w.tick(50);
        }
        w
    };
    let w = build();
    assert!(w.armies.len() >= 1, "tribute fielded a legion");
    assert_eq!(w.town_value[1], 0, "the BOUNTIED mid town was besieged + captured (the bounty steered the AI)");
    assert_eq!(build().state_hash(), build().state_hash(), "bounty-steered war replays bit-for-bit");
}

/// The cross-mode teeth extend to `PostBounty` too — transit rejects it (no mutation).
#[test]
fn transit_rejects_post_bounty() {
    let mut w = World::new(0, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    let ev = w.apply(&Command::PostBounty { station: StationId(0), amount: 500 });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "transit must reject PostBounty");
    assert!(w.bounty.is_empty(), "a rejected PostBounty sets no bounty");
}

/// Player AGENCY: without a barracks, no legion is ever fielded — even though tribute accrues. War is
/// gated on the player building a barracks (the design's "you don't command armies directly").
#[test]
fn no_barracks_no_legion() {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 7,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 0 },
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 0 },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    // Plain stations, NO barracks.
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..8000 {
        w.tick(50);
    }
    assert!(w.tribute > 0, "tribute accrued (the supply loop ran)");
    assert!(w.armies.is_empty(), "no barracks ⇒ no legion ever fielded (player agency gate)");
}

/// The disjoint-save guard's first real cross-mode teeth (S8): the transit ruleset REJECTS the
/// fantasy-only `PlaceBarracks` — no station created, no mutation, a `Rejected` event — so a transit
/// save can never contain a command that would replay against the wrong `apply`.
#[test]
fn transit_rejects_place_barracks() {
    let mut w = World::new(0, CityData::default()); // transit ruleset
    let ev = w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None });
    assert!(matches!(ev.as_slice(), [Event::Rejected { .. }]), "transit must reject PlaceBarracks");
    assert!(w.stations.is_empty(), "a rejected PlaceBarracks creates no station");
    assert!(w.is_barracks.is_empty(), "a rejected PlaceBarracks sets no barracks flag");
}

/// Transit never runs `war_step` (it inherits the no-op), so the army SoA stays empty — the war
/// machine is genuinely fantasy-only, and transit's only change is the one-time golden re-pin from the
/// (empty) army fields appearing in `Canonical`.
#[test]
fn transit_fields_no_armies() {
    let mut w = World::new(7, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..1000 {
        w.tick(50);
    }
    assert!(w.armies.is_empty(), "transit never fields legions (war is fantasy-only)");
}
