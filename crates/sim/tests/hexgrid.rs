//! The load-bearing invariants of the hex lattice (fantasy-build-plan.md S5), pinned BEFORE the
//! primitives are wired into `grid_walk`/`node_of` — so the determinism-critical float-round math is
//! proven in isolation, not inside the cross-line mutex where a bug is gate-blind. These are
//! STRUCTURAL assertions (symmetry / round-trip / adjacency / exactly-once), not `run()==run()`.
use sim::geo_local::PointMm;
use sim::hexgrid::{axial_of, center_of, distance, line, line_costed, Axial};

const SIZE: i64 = 120_000; // a representative grid_cell_mm (matches the square-grid default order)

/// THE float-round hazard the build plan flags: a cell's own centre must map back to that cell, for
/// EVERY cell in a wide range. If this ever fails, two lines meeting at a shared cell could be
/// classified into different cells ⇒ the cross-line mutex silently disengages.
#[test]
fn center_axial_roundtrip_is_exact() {
    for q in -40..=40 {
        for r in -40..=40 {
            let a: Axial = (q, r);
            let back = axial_of(center_of(a, SIZE), SIZE);
            assert_eq!(back, a, "centre of {a:?} must round-trip to {a:?}, got {back:?}");
        }
    }
}

/// `line(a,b)` must be the EXACT reverse of `line(b,a)` — the same unordered edge set, in opposite
/// order. The single property the cross-line mutex's correctness rests on (opposing traversals of a
/// shared corridor must reserve identical edges). Replicates `grid_walk`'s canonical-walk guarantee.
#[test]
fn line_is_canonical_reverse() {
    let cases: &[(Axial, Axial)] = &[
        ((0, 0), (5, 0)),
        ((0, 0), (3, 4)),
        ((-2, 1), (4, -3)),
        ((7, -2), (-5, 6)),
        ((0, 0), (0, 9)),
        ((10, 10), (-10, -10)),
    ];
    for &(a, b) in cases {
        let ab = line(a, b);
        let mut ba = line(b, a);
        ba.reverse();
        assert_eq!(ab, ba, "line({a:?},{b:?}) must equal reverse of line({b:?},{a:?})");
    }
}

/// A line's consecutive cells are always hex-NEIGHBOURS (distance 1) — no skips, no duplicates. The
/// epsilon nudge exists to guarantee this (a boundary-straddling sample could otherwise jump or stall).
#[test]
fn line_steps_are_adjacent_and_unique() {
    let cases: &[(Axial, Axial)] = &[((0, 0), (8, -3)), ((-4, 7), (5, 5)), ((0, 0), (12, 0)), ((3, 3), (3, -9))];
    for &(a, b) in cases {
        let l = line(a, b);
        assert_eq!(l.first().copied(), Some(a), "line starts at a");
        assert_eq!(l.last().copied(), Some(b), "line ends at b");
        assert_eq!(l.len() as i64, distance(a, b) + 1, "line length = distance + 1 (no skips/dupes)");
        for w in l.windows(2) {
            assert_eq!(distance(w[0], w[1]), 1, "consecutive cells {:?}->{:?} must be neighbours", w[0], w[1]);
        }
    }
}

/// Distance is a metric on the lattice: zero to self, symmetric, and 1 to each of the 6 neighbours.
#[test]
fn distance_is_a_hex_metric() {
    assert_eq!(distance((3, -2), (3, -2)), 0, "distance to self is 0");
    assert_eq!(distance((0, 0), (9, -4)), distance((9, -4), (0, 0)), "distance is symmetric");
    // The 6 pointy-top axial neighbours.
    let nbrs: [Axial; 6] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)];
    for d in nbrs {
        assert_eq!(distance((0, 0), d), 1, "{d:?} is a unit neighbour of the origin");
    }
}

/// The build is deterministic: identical inputs give identical lines and identical cell centres
/// (the float path must not vary run-to-run). The in-module analog of the global replay gate.
#[test]
fn hex_build_is_deterministic() {
    let a: Axial = (-7, 3);
    let b: Axial = (6, -8);
    assert_eq!(line(a, b), line(a, b), "line is a pure function");
    assert_eq!(center_of(a, SIZE), center_of(a, SIZE), "centre is a pure function");
    // distinct cells get distinct centres (no collisions that would merge edges).
    let c1 = center_of((0, 0), SIZE);
    let c2 = center_of((1, 0), SIZE);
    let c3 = center_of((0, 1), SIZE);
    assert!(c1 != c2 && c1 != c3 && c2 != c3, "distinct cells have distinct centres");
}

/// ADVERSARIAL: exhaustively sweep ~28k ordered axial pairs and assert the load-bearing properties on
/// EVERY one — `line(a,b)` is the exact reverse of `line(b,a)`, every step is a unit hex neighbour, and
/// the endpoints/length are exact. A single counterexample (one pair where the canonical reverse
/// breaks) would silently disengage the cross-line mutex, so the hand-picked cases above aren't
/// enough — this enumerates the whole neighbourhood. Pure integer/quantised math ⇒ a clean sweep is a
/// proof over the tested domain, no sampling.
#[test]
fn line_canonical_reverse_holds_exhaustively() {
    let mut checked = 0u64;
    for aq in -6..=6 {
        for ar in -6..=6 {
            for bq in -6..=6 {
                for br in -6..=6 {
                    let a: Axial = (aq, ar);
                    let b: Axial = (bq, br);
                    let ab = line(a, b);
                    let mut ba = line(b, a);
                    ba.reverse();
                    assert_eq!(ab, ba, "canonical-reverse broke at {a:?}<->{b:?}");
                    assert_eq!(ab.first().copied(), Some(a));
                    assert_eq!(ab.last().copied(), Some(b));
                    assert_eq!(ab.len() as i64, distance(a, b) + 1, "skip/dupe at {a:?}->{b:?}");
                    for w in ab.windows(2) {
                        assert_eq!(distance(w[0], w[1]), 1, "non-unit step at {a:?}->{b:?}: {:?}", w);
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 13 * 13 * 13 * 13, "swept the full pair neighbourhood");
}

/// THE one-bend property (the readability win): a minimal lattice line turns at most ONCE — a single
/// run along one direction, then a single run along another. Swept exhaustively (the OLD cube-lerp
/// staircase would fail this with many turns). Fewer turns ⇒ the clean TTD-style track the design wants.
#[test]
fn line_has_at_most_one_bend() {
    for aq in -6..=6 {
        for ar in -6..=6 {
            for bq in -6..=6 {
                for br in -6..=6 {
                    let l = line((aq, ar), (bq, br));
                    let mut turns = 0;
                    for w in l.windows(3) {
                        let s1 = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                        let s2 = (w[2].0 - w[1].0, w[2].1 - w[1].1);
                        if s1 != s2 {
                            turns += 1;
                        }
                    }
                    assert!(turns <= 1, "({aq},{ar})->({bq},{br}) bends {turns}× (want ≤1): {l:?}");
                }
            }
        }
    }
}

/// COST-AWARENESS: of the two same-length one-bend corners, the router takes the cheaper. With a cost
/// that penalises the q==0 side, the (0,0)->(2,2) line swings the OTHER way (right-then-up); flip the
/// penalty and it swings back (up-then-right). So track routes around dear terrain (water/mountains).
#[test]
fn line_costed_takes_the_cheaper_corner() {
    let a: Axial = (0, 0);
    let b: Axial = (2, 2);
    let up_then_right = vec![(0, 0), (0, 1), (0, 2), (1, 2), (2, 2)];
    let right_then_up = vec![(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)];
    // Penalise the q==0 cells ⇒ avoid the up-first corner ⇒ take right-then-up.
    let avoid_left = line_costed(a, b, &|c: Axial| if c.0 == 0 { 1000 } else { 0 });
    assert_eq!(avoid_left, right_then_up, "should swing away from the penalised q==0 side");
    // Penalise the q==2 cells ⇒ avoid the right-first corner ⇒ take up-then-right.
    let avoid_right = line_costed(a, b, &|c: Axial| if c.0 == 2 { 1000 } else { 0 });
    assert_eq!(avoid_right, up_then_right, "should swing away from the penalised q==2 side");
    // Symmetry holds with cost too (b->a is the exact reverse).
    let mut rev = line_costed(b, a, &|c: Axial| if c.0 == 0 { 1000 } else { 0 });
    rev.reverse();
    assert_eq!(rev, avoid_left, "cost-aware line stays canonical/symmetric");
}

/// A point near a cell's centre (within the inner radius) classifies to that cell — the snap a build
/// gesture relies on. Uses small offsets well inside the hex so we don't probe boundary ties here.
#[test]
fn near_center_snaps_to_cell() {
    for q in -6..=6 {
        for r in -6..=6 {
            let c = center_of((q, r), SIZE);
            // nudge a few mm off-centre; the inner radius is ~size*√3/2 ≫ a few mm.
            let probe = PointMm::new(c.x_mm + 1_000, c.y_mm - 1_000);
            assert_eq!(axial_of(probe, SIZE), (q, r), "a point near centre of ({q},{r}) snaps there");
        }
    }
}
