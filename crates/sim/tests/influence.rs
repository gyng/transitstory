//! Fantasy (arcadia) #infrastructure — the CONNECTED-RAIL build gate. The realm's network must be ONE
//! graph rooted at the capital: rail extends only from a station already wired to your seat (or to a
//! town conquest has flipped into a holding). A fresh line must START at a holding; once anchored it
//! reaches ANY distance — connectivity, not a radius. Conquest mints a new rail root. The gate is a pure
//! `apply`-time read (no hashed state), so `influence_hops == 0` (transit + the golden fixture) is
//! byte-identical — proven by the determinism/arcadia golden tests, not re-asserted here.
use sim::*;

/// A small arcadia world: a capital seat at the origin (station 0), plus a NEAR (station 1, 500 km) and
/// a FAR (station 2, 3 000 km) station — neither a holding, so both start OFF the network. `realm()`
/// leaves an empty Line 0 ready to rail.
fn realm() -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000,
        capital_x_mm: 0,
        capital_y_mm: 0,
        influence_hops: 5, // any > 0 ARMS the gate; the value no longer sets a build radius
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![DemandCell { x_mm: 0, y_mm: 0, origin_w: 80.0, dest_w: 2.0, commodity: 1 }],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // 0 — at the capital (a root)
    w.apply(&Command::PlaceStation { x_mm: 500_000, y_mm: 0, name: None }); // 1 — near, off-network
    w.apply(&Command::PlaceStation { x_mm: 3_000_000, y_mm: 0, name: None }); // 2 — far, off-network
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w
}

fn rejected(evs: &[Event]) -> bool {
    evs.iter().any(|e| matches!(e, Event::Rejected { .. }))
}

fn add(w: &mut World, line: u32, station: u32) -> Vec<Event> {
    w.apply(&Command::AddStop { line: LineId(line), station: StationId(station), after: None })
}

#[test]
fn fresh_line_must_start_at_a_holding() {
    let mut w = realm();
    // Station 1 (near) is NOT a holding and no line touches it yet — a fresh line cannot start there,
    // however close it sits to the capital. Connectivity, not a radius.
    let evs = add(&mut w, 0, 1);
    assert!(rejected(&evs), "an isolated first stop must be gated: {evs:?}");
    assert_eq!(w.lines[0].stops.len(), 0, "the rejected stop must not attach");
    // But the capital station (a root) is always a valid seed.
    assert!(!rejected(&add(&mut w, 0, 0)), "the capital seat must always be buildable");
    assert_eq!(w.lines[0].stops.len(), 1);
}

#[test]
fn an_anchored_line_extends_to_any_distance() {
    let mut w = realm();
    // Anchor at the capital, then extend to the near AND the far station — distance is irrelevant once
    // the line is wired to the seat (the OLD influence disc would have gated the 3 000 km far stop).
    assert!(!rejected(&add(&mut w, 0, 0)));
    assert!(!rejected(&add(&mut w, 0, 1)));
    let evs = add(&mut w, 0, 2);
    assert!(!rejected(&evs), "a connected line extends to any distance: {evs:?}");
    assert_eq!(w.lines[0].stops.len(), 3, "all three stops attach");
}

#[test]
fn a_new_line_may_branch_off_the_existing_network() {
    let mut w = realm();
    // Build Line 0: capital → near. Station 1 is now ON the network.
    add(&mut w, 0, 0);
    add(&mut w, 0, 1);
    // A SECOND line may seed at station 1 (an interchange) even though it isn't a holding — it's
    // reachable via Line 0.
    w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 0, literal: false });
    assert!(!rejected(&add(&mut w, 1, 1)), "a new line may seed on a networked station");
    // …and extend onward to the far station.
    assert!(!rejected(&add(&mut w, 1, 2)), "the branch line extends from the network");
    assert_eq!(w.lines[1].stops.len(), 2);
}

#[test]
fn conquest_mints_a_new_rail_root() {
    let mut w = realm();
    // The far station (station 2) is unreachable: not a holding, no line touches it.
    assert!(rejected(&add(&mut w, 0, 2)), "the far station starts off-network");
    // Conquer the far town — siege grinds its garrison to 0, flipping `town_value` (the exact signal
    // `compute_rail_reachable` reads as a root). It need NOT be wired to the capital by rail.
    while w.town_value.len() <= 2 {
        w.town_value.push(crate_resistance());
    }
    w.town_value[2] = 0; // station 2 is now a captured holding
    // A fresh line may now START at the captured town — an island far from the capital network.
    w.apply(&Command::CreateLine { color: 3, name: None, loop_line: false, mode: 0, literal: false });
    let evs = add(&mut w, 1, 2);
    assert!(!rejected(&evs), "a captured town must be a valid rail root: {evs:?}");
    assert_eq!(w.lines[1].stops.len(), 1);
}

/// A non-zero garrison value for the still-neutral filler stations the test pushes (any `> 0` reads as
/// uncaptured). The exact figure is irrelevant — only `== 0` flips a town to a holding.
fn crate_resistance() -> i64 {
    1_000
}
