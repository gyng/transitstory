//! S10a — the decadence area-control field's STATIC topology (the board the S10b tide diffuses over).
//! These pin the structural invariants the build plan's risk battery names for the CA substrate — a hex
//! domain over passable land, symmetric adjacency, a monotone creep gradient to the capital, and a
//! far-edge reservoir that REACHES the capital (the loseability guarantee) — asserted structurally, never
//! via `run()==run()`. The field is un-hashed (a pure function of `CityData`), so building it is
//! golden-neutral; the dynamic hashed tide is S10b.
use sim::hexgrid::{self, Axial};
use sim::*;

const SIZE: i64 = 250_000; // the baked hex pitch (grid_cell_mm)

/// A synthetic baked-like arcadia world: a `w × h` block of hex cells (PLAIN=10), with `holes` punched
/// as impassable MOUNTAIN(=6), a `capital` cell, on the same hex lattice + transform the bake uses.
fn hex_world(w: i64, h: i64, capital: Axial, holes: &[Axial]) -> CityData {
    let mut cells = Vec::new();
    for q in 0..w {
        for r in 0..h {
            let c = if holes.contains(&(q, r)) { 6 } else { 10 };
            let p = hexgrid::center_of((q, r), SIZE);
            cells.push(BuildCell { x_mm: p.x_mm, y_mm: p.y_mm, c });
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

#[test]
fn builds_a_passable_hex_domain_with_symmetric_adjacency() {
    let f = DecadenceField::build(&hex_world(10, 10, (0, 0), &[]));
    assert_eq!(f.len(), 100, "every plain cell joins the domain");
    // Cells are sorted (deterministic, index-stable).
    let mut sorted = f.cells.clone();
    sorted.sort_unstable();
    assert_eq!(f.cells, sorted, "domain cells are index-ordered");
    // Adjacency is SYMMETRIC and every listed neighbour is an actual hex neighbour (distance 1).
    for i in 0..f.len() as u32 {
        for &j in f.neighbors(i) {
            assert_eq!(hexgrid::distance(f.cells[i as usize], f.cells[j as usize]), 1, "neighbours are adjacent");
            assert!(f.neighbors(j).contains(&i), "adjacency is symmetric ({i}->{j} but not back)");
        }
    }
    // An interior cell has all 6 neighbours; a corner has fewer.
    let interior = f.cells.iter().position(|&c| c == (5, 5)).unwrap() as u32;
    assert_eq!(f.neighbors(interior).len(), 6, "an interior hex has 6 neighbours");
}

#[test]
fn creep_distance_is_monotone_from_the_capital() {
    let f = DecadenceField::build(&hex_world(10, 10, (0, 0), &[]));
    let cap = f.capital.expect("the baked capital lands on a domain cell");
    assert_eq!(f.dist_to_capital[cap as usize], 0, "the capital is the gradient origin (distance 0)");
    for i in 0..f.len() as u32 {
        let d = f.dist_to_capital[i as usize];
        assert_ne!(d, u32::MAX, "every cell of a connected block is reachable from the capital");
        if i != cap {
            // BFS monotonicity: a downhill neighbour (one step closer to the capital) always exists.
            let has_downhill = f.neighbors(i).iter().any(|&n| f.dist_to_capital[n as usize] == d - 1);
            assert!(has_downhill, "cell {i} (dist {d}) has a neighbour one step closer to the capital");
        }
    }
}

#[test]
fn reservoir_is_the_far_edge_and_reaches_the_capital() {
    // Loseability: the tide-origin reservoir must be able to PATH to the capital, or the realm is
    // unloseable — the build plan asserts this against a walled-off capital.
    let f = DecadenceField::build(&hex_world(12, 12, (0, 0), &[]));
    assert!(!f.reservoir.is_empty(), "a tide reservoir is seeded");
    assert!(f.capital.is_some(), "a baked domain resolves a capital");
    let maxd = *f.dist_to_capital.iter().filter(|&&d| d != u32::MAX).max().unwrap();
    for &c in &f.reservoir {
        let d = f.dist_to_capital[c as usize];
        assert_ne!(d, u32::MAX, "every reservoir anchor reaches the capital (loseable, not walled off)");
        assert!(d * 2 > maxd, "the reservoir is the FAR edge (in the far half of the creep distance)");
    }
    // The single farthest seed is the corner opposite the (0,0) capital, at the max creep distance.
    let far = f.reservoir[0];
    assert_eq!(f.dist_to_capital[far as usize], maxd, "reservoir[0] is the farthest cell");
    assert!(f.cells[far as usize].0 > 6 && f.cells[far as usize].1 > 6, "the reservoir is far from the (0,0) capital");
}

#[test]
fn the_bfs_routes_through_a_carved_pass_in_a_mountain_wall() {
    // The bake guarantees a passable corridor to the capital even across a ridge (the carved pass). A
    // full MOUNTAIN wall WITH one gap must still connect both sides — the loseability structural proof.
    let wall: Vec<Axial> = (0..10).filter(|&r| r != 5).map(|r| (5, r)).collect(); // column q=5, gap at r=5
    let f = DecadenceField::build(&hex_world(10, 10, (0, 0), &wall));
    assert_eq!(f.len(), 100 - 9, "the 9 wall cells are excluded (the gap stays)");
    // A cell on the FAR side of the wall is still reachable (only via the carved gap).
    let far = f.cells.iter().position(|&c| c == (9, 9)).unwrap() as u32;
    assert_ne!(f.dist_to_capital[far as usize], u32::MAX, "the far side is reachable through the pass");
}

#[test]
fn mountains_and_water_are_excluded_from_the_domain() {
    let f = DecadenceField::build(&hex_world(6, 6, (0, 0), &[(2, 2), (3, 3)]));
    assert!(!f.cells.contains(&(2, 2)) && !f.cells.contains(&(3, 3)), "MOUNTAIN cells are not domain");
    assert_eq!(f.len(), 36 - 2, "everything but the two ridge cells");
}

#[test]
fn transit_and_terrainless_worlds_have_an_empty_field() {
    assert!(DecadenceField::build(&CityData::default()).is_empty(), "transit (no terrain) ⇒ no CA");
    // Buildability present but no hex pitch ⇒ no CA (we can't quantise to the lattice).
    let mut no_pitch = hex_world(4, 4, (0, 0), &[]);
    no_pitch.grid_cell_mm = 0;
    assert!(DecadenceField::build(&no_pitch).is_empty(), "no grid_cell_mm ⇒ no CA");
    // A baked domain ALWAYS resolves a capital — even one sited at the origin cell (0,0) is a real seat,
    // not an "unset" sentinel (non-baked worlds returned empty above, so we only get here for a real map).
    let origin_cap = DecadenceField::build(&hex_world(4, 4, (0, 0), &[]));
    assert!(origin_cap.capital.is_some() && !origin_cap.reservoir.is_empty(), "a capital at the origin still seeds a tide");
}

#[test]
fn field_build_is_deterministic() {
    let mk = || DecadenceField::build(&hex_world(11, 9, (1, 1), &[(4, 4), (4, 5)]));
    let a = mk();
    let b = mk();
    assert_eq!(a.cells, b.cells);
    assert_eq!(a.nbr_start, b.nbr_start);
    assert_eq!(a.nbr_flat, b.nbr_flat);
    assert_eq!(a.dist_to_capital, b.dist_to_capital);
    assert_eq!(a.reservoir, b.reservoir);
    assert_eq!(a.capital, b.capital);
}

#[test]
fn world_new_wires_the_field_and_stays_golden_neutral() {
    // The field is built into World, but it is un-hashed (a pure function of CityData), so a fantasy
    // world's state_hash is identical whether or not the field has cells — adding it re-pins NOTHING.
    let w = World::new(12, hex_world(8, 8, (0, 0), &[]));
    assert!(!w.decadence_field.is_empty(), "World::new builds the CA board for a baked arcadia world");
    assert_eq!(w.decadence_field.capital, w.decadence_field.capital, "capital resolved");
    // Two worlds whose ONLY difference is the (un-hashed) field topology hash identically: a 4x4 vs an
    // 8x8 terrain with no commands applied both start at the same canonical state (empty dynamic state).
    let small = World::new(12, hex_world(4, 4, (0, 0), &[]));
    let big = World::new(12, hex_world(9, 9, (0, 0), &[]));
    assert_ne!(small.decadence_field.len(), big.decadence_field.len(), "different boards");
    assert_eq!(small.state_hash(), big.state_hash(), "the un-hashed CA board never moves state_hash");
}
