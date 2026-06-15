//! Fantasy (arcadia) #9 — the AREA-OF-INFLUENCE build gate. You may only lay rail to a station within
//! `influence_hops` grid-hexes of a HOLDING (the capital, or a town conquest has flipped). A still-far
//! town is unreachable until you capture a closer one; conquest expands the buildable frontier. The
//! gate is a pure `apply`-time read (no hashed state), so `influence_hops == 0` (transit + the golden
//! fixture) is byte-identical — proven by the determinism/arcadia golden tests, not re-asserted here.
use sim::*;

/// A small arcadia world with a capital at the origin, a NEAR town (within reach) and a FAR town
/// (beyond it). `influence_hops` is sized so the near town is buildable and the far one is not.
fn realm() -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000,
        // capital seat at the origin; reach = 5 hops × 100_000 × √3 ≈ 866_000 mm.
        capital_x_mm: 0,
        capital_y_mm: 0,
        influence_hops: 5,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 1 }],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    // Station 0: at the capital (always a holding). Station 1: NEAR (500 km < 866 km reach).
    // Station 2: FAR (3_000 km, far past reach).
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 500_000, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w
}

fn rejected(evs: &[Event]) -> bool {
    evs.iter().any(|e| matches!(e, Event::Rejected { .. }))
}

#[test]
fn near_station_is_buildable_far_station_is_gated() {
    let mut w = realm();
    // The capital-anchored stop (station 0) and a near stop (station 1) are within reach.
    assert!(!rejected(&w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None })));
    assert!(!rejected(&w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None })));
    // The far town (station 2) is beyond the realm's reach — gated.
    let evs = w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    assert!(rejected(&evs), "a far station must be rejected by the influence gate: {evs:?}");
    // And the line did NOT gain the far stop (no mutation on reject).
    assert_eq!(w.lines[0].stops.len(), 2, "the gated stop must not attach");
}

#[test]
fn conquest_extends_the_frontier() {
    let mut w = realm();
    // A town at 1_700 km is beyond the capital's ~866 km reach.
    w.apply(&Command::PlaceStation { x_mm: 1_700_000, y_mm: 0, name: None }); // station 3
    assert!(
        rejected(&w.apply(&Command::AddStop { line: LineId(0), station: StationId(3), after: None })),
        "station 3 (1_700 km) is beyond the capital's reach",
    );
    // Conquer a town at 1_000 km (siege grinds its garrison to 0). Capturing flips `town_value` — the
    // exact signal `buildable_at` reads to widen the realm.
    w.apply(&Command::PlaceStation { x_mm: 1_000_000, y_mm: 0, name: None }); // station 4
    while w.town_value.len() <= 4 {
        w.town_value.push(crate_resistance());
    }
    w.town_value[4] = 0; // the holding at 1_000 km is now ours
    // station 3 at 1_700 km is within ~866 km of the holding at 1_000 km (Δ = 700 km) — now buildable.
    let evs = w.apply(&Command::AddStop { line: LineId(0), station: StationId(3), after: None });
    assert!(!rejected(&evs), "a captured town must extend the frontier to reach station 3: {evs:?}");
}

/// A non-zero garrison value for the still-neutral filler stations the test pushes (any `> 0` reads as
/// uncaptured). The exact figure is irrelevant — only `== 0` flips a town to a holding.
fn crate_resistance() -> i64 {
    1_000
}
