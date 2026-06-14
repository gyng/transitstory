//! S9 — decadence, the lose condition. The spreading corruption overruns a realm that doesn't fight
//! back, and conquest pushes it back — the urgency behind the supply→conquest flywheel. These tests
//! assert the core tension: an idle realm FALLS; a conquering realm HOLDS. Deterministic.
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

/// A realm that runs its supply but NEVER fights (no barracks ⇒ no conquest) is overrun: decadence
/// grows unchecked past the capital threshold. The pressure has teeth.
#[test]
fn decadence_overruns_an_idle_realm() {
    let mut w = World::new(
        11,
        arcadia(vec![
            DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 0 },
            DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 0 },
        ]),
    );
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..10000 {
        w.tick(50);
    }
    assert!(w.decadence > 0, "decadence spread while running");
    assert!(sim::decadence::is_lost(&w), "an idle realm (no conquest) is overrun by the corruption");
}

/// Conquest is the BRAKE on decadence: a captured town drives the net rate negative. The robust proof
/// (timing-independent) is a contrast — the SAME realm with a barracks (so it conquers) ends with
/// strictly LESS decadence than without one (idle). The source↔sink are kept farther apart than the
/// ~500 m catchment so the supply chain actually flows (a short route would merge their catchments and
/// stall production — the lesson from the first cut). Deterministic.
fn run_realm(with_barracks: bool, ticks: usize) -> World {
    let mut w = World::new(
        11,
        arcadia(vec![
            DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 0 }, // source (+ barracks)
            DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 80.0, commodity: 0 }, // town
        ]),
    );
    if with_barracks {
        w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None });
    } else {
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    }
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
fn conquest_pushes_decadence_back() {
    let idle = run_realm(false, 15000); // no barracks ⇒ no conquest ⇒ unchecked corruption
    let conquering = run_realm(true, 15000); // barracks ⇒ a town falls ⇒ pushback
    assert!(conquering.towns_captured >= 1, "the conquering realm took a town (the brake engaged)");
    assert!(
        conquering.decadence < idle.decadence,
        "conquest pushes decadence below the idle (unchecked) level: {} vs {}",
        conquering.decadence,
        idle.decadence
    );
    assert_eq!(run_realm(true, 15000).state_hash(), run_realm(true, 15000).state_hash(), "the decadence race replays bit-for-bit");
}

/// S4 bake seam: a baked world's `initial_decadence` seeds the STARTING corruption — a more-corrupt
/// continent begins further up the lose meter, so an idle realm is overrun SOONER. The `Default` city
/// leaves it 0 (a clean start), which is why every existing city + the golden fixtures are byte-identical.
#[test]
fn initial_decadence_seeds_the_starting_corruption() {
    let mk = || vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 1.0, dest_w: 1.0, commodity: 0 }];

    // default (unseeded) ⇒ the realm starts clean
    assert_eq!(World::new(11, arcadia(mk())).decadence, 0, "an unseeded realm starts clean");

    // baked-corrupt ⇒ starts at the seeded floor
    let mut corrupt = arcadia(mk());
    corrupt.initial_decadence = 5000;
    assert_eq!(World::new(11, corrupt).decadence, 5000, "the bake seeds world.decadence");

    // a negative bake can't bank surplus (clamped ≥ 0)
    let mut neg = arcadia(mk());
    neg.initial_decadence = -1234;
    assert_eq!(World::new(11, neg).decadence, 0, "a negative seed clamps to a clean start");

    // urgency: a more-corrupt continent (higher seed) is overrun in FEWER ticks than a clean one
    let ticks_to_fall = |seed_dec: i64| {
        let mut c = arcadia(mk());
        c.initial_decadence = seed_dec;
        let mut w = World::new(11, c);
        w.apply(&Command::SetRunning { running: true });
        let mut n = 0;
        while !sim::decadence::is_lost(&w) && n < 1_000_000 {
            w.tick(50);
            n += 1;
        }
        n
    };
    assert!(ticks_to_fall(10_000) < ticks_to_fall(0), "a more-corrupt continent falls sooner (more urgency)");
}

/// Transit never runs `war_step` ⇒ no decadence, never lost — the lose condition is fantasy-only.
#[test]
fn transit_has_no_decadence() {
    let mut w = World::new(0, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..2000 {
        w.tick(50);
    }
    assert_eq!(w.decadence, 0, "transit has no decadence");
    assert!(!sim::decadence::is_lost(&w), "transit can never be lost to corruption");
}
