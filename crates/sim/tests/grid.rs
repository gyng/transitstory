//! GRID geometry mode (fantasy-fork.md §10 / shared-rail.md), now on a HEX lattice (S5). When
//! `CityData.grid_cell_mm > 0`, track is built on pointy-top axial hex cells (`hexgrid`,
//! integer-quantised) so two lines over the same corridor produce BYTE-IDENTICAL vertices — the
//! foundation for the cross-line `edge_key` mutex. Tested through Commands + the public
//! `Path.polyline`. Parity (a non-grid city is byte-identical) is covered by the rest of the suite
//! staying green (and the determinism golden pin, which uses continuous geometry).
use sim::hexgrid;
use sim::*;

fn grid_world(cell_mm: i64, seed: u64) -> World {
    World::new(seed, CityData { grid_cell_mm: cell_mm, ..Default::default() })
}

fn place(w: &mut World, x: i64, y: i64) -> StationId {
    let id = StationId(w.stations.len() as u32);
    w.apply(&Command::PlaceStation { x_mm: x, y_mm: y, name: None });
    id
}

fn make_line(w: &mut World, stops: &[StationId]) -> LineId {
    let li = LineId(w.lines.len() as u32);
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for &s in stops {
        w.apply(&Command::AddStop { line: li, station: s, after: None });
    }
    li
}

fn cell_of(p: &PointMm, cell: i64) -> (i64, i64) {
    hexgrid::axial_of(*p, cell)
}

#[test]
fn grid_vertices_land_on_the_lattice() {
    let cell = 100_000i64;
    let mut w = grid_world(cell, 1);
    // Stops placed at arbitrary sub-cell positions — they must SNAP to hex cell centres.
    let a = place(&mut w, 314_159, 271_828);
    let b = place(&mut w, 600_000, 900_001);
    let c = place(&mut w, 1_050_000, 250_000);
    make_line(&mut w, &[a, b, c]);
    let poly = &w.lines[0].paths[0].polyline;
    assert!(poly.len() >= 3, "a grid polyline is a dense lattice walk");
    // Every vertex is a HEX cell CENTRE: it round-trips through axial_of → center_of back to itself.
    for p in poly {
        let c = hexgrid::axial_of(*p, cell);
        assert_eq!(hexgrid::center_of(c, cell), *p, "vertex {p:?} is not a hex cell centre (cell {c:?})");
    }
    // Consecutive vertices are adjacent hexes (a unit lattice step) ⇒ unit tile-edges.
    for w2 in poly.windows(2) {
        let (ca, cb) = (cell_of(&w2[0], cell), cell_of(&w2[1], cell));
        assert_eq!(hexgrid::distance(ca, cb), 1, "non-unit hex step {ca:?}->{cb:?}");
    }
}

#[test]
fn two_lines_share_byte_identical_edges_on_a_common_corridor() {
    // THE cross-line foundation (the LITE narrow guarantee): two distinct lines that both run A -> B
    // with the SAME consecutive stop-cells produce IDENTICAL polyline vertices on that section — so
    // Phase 2's per-edge mutex (keyed on the cell pair) actually engages. This is the shared-station
    // trunk pattern. (A corridor shared BETWEEN stops is the FULL-model seam, pinned #[ignore]d above.)
    // RED on continuous geometry: two Catmull-Rom curves never share exact vertices.
    let cell = 100_000i64;
    let mut w = grid_world(cell, 2);
    let a_pos = PointMm::new(3 * cell + 10_000, 4_000);
    let b_pos = PointMm::new(6 * cell + 20_000, 3 * cell + 30_000);
    let a = place(&mut w, a_pos.x_mm, a_pos.y_mm);
    let b = place(&mut w, b_pos.x_mm, b_pos.y_mm);
    // Two lines sharing the A->B section, approaching/leaving from DIFFERENT ends.
    let x = place(&mut w, 0, 0);
    let y = place(&mut w, 9 * cell, 9 * cell);
    let z = place(&mut w, 0, 5 * cell);
    let q = place(&mut w, 9 * cell, 0);
    make_line(&mut w, &[x, a, b, y]);
    make_line(&mut w, &[z, a, b, q]);

    // The hex cells stops A and B snap to (where the shared A->B section begins/ends).
    let ca = hexgrid::axial_of(a_pos, cell);
    let cb = hexgrid::axial_of(b_pos, cell);
    let section = |li: usize| -> Vec<PointMm> {
        let poly = &w.lines[li].paths[0].polyline;
        let ia = poly.iter().position(|p| cell_of(p, cell) == ca).expect("A vertex");
        let ib = poly.iter().position(|p| cell_of(p, cell) == cb).expect("B vertex");
        assert!(ia < ib);
        poly[ia..=ib].to_vec()
    };
    let s0 = section(0);
    let s1 = section(1);
    assert!(s0.len() >= 4, "the A->B section spans several lattice edges");
    assert_eq!(s0, s1, "two lines over the same corridor must emit byte-identical vertices");
}

/// FULL-track-model seam (`#[ignore]`d, un-ignore when lines reference explicit laid track). An
/// EXPRESS line A→B and a LOCAL line A→M→B running the SAME physical rail (M placed exactly on the
/// A→B line) emit DIFFERENT edges today, because grid_walk splits the hex line at M. The narrow
/// LITE guarantee (shared consecutive stop-cells) does not cover a corridor shared BETWEEN stops — that
/// needs explicit laid track both lines reference (the FULL track-objects model). Captured by the grid
/// review so Phase 2's "shared consecutive stop-cells" contract is honest and the seam is tracked.
#[test]
#[ignore = "express/local corridor-between-stops sharing needs the FULL laid-track model; see grid_walk doc"]
fn grid_express_local_corridor_shares_edges_is_full_model() {
    let cell = 100_000i64;
    let mut w = grid_world(cell, 5);
    let a = place(&mut w, 12_345, 9_999); // cell (0,0)
    let m = place(&mut w, 3 * cell + 50_000, 2 * cell + 50_000); // cell (3,2), exactly on A→B
    let b = place(&mut w, 6 * cell + 40_000, 4 * cell + 40_000); // cell (6,4)
    make_line(&mut w, &[a, b]); // express
    make_line(&mut w, &[a, m, b]); // local
    let edge_set = |li: usize| -> std::collections::BTreeSet<((i64, i64), (i64, i64))> {
        w.lines[li].paths[0]
            .polyline
            .windows(2)
            .map(|w2| {
                let (p, q) = (cell_of(&w2[0], cell), cell_of(&w2[1], cell));
                if p <= q { (p, q) } else { (q, p) }
            })
            .collect()
    };
    assert_eq!(edge_set(0), edge_set(1), "express + local on one rail must share all edges (FULL model)");
}

#[test]
fn grid_walk_is_symmetric_and_canonical() {
    // A -> B and B -> A produce the reverse of the SAME vertex list (so an out-and-back train and an
    // opposing train share the edge), and the walk depends only on the cell pair (canonical).
    let cell = 100_000i64;
    let mut w = grid_world(cell, 3);
    let a = place(&mut w, 2 * cell + 5_000, 1 * cell + 5_000); // (2,1)
    let b = place(&mut w, 7 * cell + 5_000, 5 * cell + 5_000); // (7,5)
    make_line(&mut w, &[a, b]);
    make_line(&mut w, &[b, a]);
    let mut p0 = w.lines[0].paths[0].polyline.clone();
    let p1 = w.lines[1].paths[0].polyline.clone();
    p0.reverse();
    assert_eq!(p0, p1, "B->A must be the exact reverse of A->B (canonical, symmetric)");
}

#[test]
fn grid_city_replays_bit_for_bit() {
    let build = || {
        let mut w = grid_world(100_000, 9);
        let a = place(&mut w, 0, 0);
        let b = place(&mut w, 550_000, 320_000);
        let c = place(&mut w, 1_100_000, 80_000);
        let li = make_line(&mut w, &[a, b, c]);
        w.apply(&Command::AssignTrainset { line: li, spec: 0, count: 3 });
        w.apply(&Command::SetRunning { running: true });
        w
    };
    let mut a = build();
    let mut b = build();
    for _ in 0..1500 {
        a.tick(50);
    }
    for _ in 0..1500 {
        b.tick(50);
    }
    assert_eq!(a.state_hash(), b.state_hash(), "grid geometry replays deterministically");
}

#[test]
fn grid_handles_adjacent_and_same_cell_stops() {
    // Degenerate spacing must not panic: stops in adjacent cells (a single edge) and two stops in the
    // SAME cell (a zero-length span). Trains still dispatch + move.
    let cell = 100_000i64;
    let mut w = grid_world(cell, 4);
    let a = place(&mut w, 2 * cell + 1_000, 0); // (2,0)
    let b = place(&mut w, 3 * cell + 1_000, 0); // (3,0) adjacent
    let c = place(&mut w, 3 * cell + 9_000, 5_000); // (3,0) SAME cell as b
    let d = place(&mut w, 6 * cell, 0); // (6,0)
    let li = make_line(&mut w, &[a, b, c, d]);
    w.apply(&Command::AssignTrainset { line: li, spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    let total = w.lines[0].length_mm();
    assert!(total > 0, "the line has positive length despite a zero-length span");
    let mut moved = false;
    let start = { w.tick(50); w.vehicles.s_mm.clone() };
    for _ in 0..2000 {
        w.tick(50);
    }
    for i in 0..w.vehicles.len() {
        if w.vehicles.s_mm[i] != start[i] {
            moved = true;
        }
    }
    assert!(moved, "trains move on a grid line with degenerate spacing");
}
