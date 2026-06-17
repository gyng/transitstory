//! TTD L1 — the derived TrackGraph (docs/ttd-track-model.md). Asserts the graph is correctly derived
//! from GRID line polylines (stations/junctions/termini → nodes; degree-2 runs → segments; shared
//! corridors → one shared segment), is command-order-independent + replay-deterministic, and is INERT
//! (empty) on continuous geometry. Golden-neutrality (it never moves `state_hash`) is enforced by the
//! existing `determinism.rs`/`arcadia.rs` golden pins staying green — a derived, non-`Canonical` field
//! cannot drift the hash, and this suite would also catch a derivation panic.
use sim::hexgrid;
use sim::track_graph::{derive_track_graph, NodeKind};
use sim::*;

const CELL: i64 = 100_000;

fn grid_world(cell_mm: i64) -> World {
    World::new(7, CityData { grid_cell_mm: cell_mm, ..Default::default() })
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

/// A cell on the x-axis: `(q*CELL + CELL/2, CELL/2)` snaps to axial cell `(q, 0)` (shared_rail convention).
fn xcell(q: i64) -> (i64, i64) {
    (q * CELL + CELL / 2, CELL / 2)
}
/// A cell offset in r: `(q, r)` lattice cell, via its centre.
fn at_cell(q: i64, r: i64) -> (i64, i64) {
    let p = hexgrid::center_of((q, r), CELL);
    (p.x_mm, p.y_mm)
}

fn cell_of(w: &World, s: StationId) -> (i64, i64) {
    hexgrid::axial_of(w.stations[s.0 as usize].pos, CELL)
}

#[test]
fn empty_on_continuous_geometry() {
    // grid_cell_mm == 0 ⇒ the graph is meaningless (no exact shared vertices) ⇒ inert/empty.
    let mut w = World::new(7, CityData::default());
    let a = place(&mut w, 0, 0);
    let b = place(&mut w, 5_000_000, 0);
    make_line(&mut w, &[a, b]);
    let g = derive_track_graph(&w);
    assert!(g.nodes.is_empty() && g.segments.is_empty(), "continuous geometry yields no track graph");
}

#[test]
fn single_line_is_station_nodes_plus_contracted_segments() {
    let mut w = grid_world(CELL);
    let (x0, y0) = xcell(1);
    let s0 = place(&mut w, x0, y0);
    let (x1, y1) = xcell(4);
    let s1 = place(&mut w, x1, y1);
    let (x2, y2) = xcell(7);
    let s2 = place(&mut w, x2, y2);
    make_line(&mut w, &[s0, s1, s2]);
    let g = derive_track_graph(&w);

    // Exactly the 3 stations are nodes (the degree-2 interior cells are contracted away).
    assert_eq!(g.nodes.len(), 3, "3 stops ⇒ 3 nodes");
    assert!(g.nodes.iter().all(|n| n.kind == NodeKind::Station), "every node is a station here");
    // 2 segments, between consecutive stations, with contiguous cell chains.
    assert_eq!(g.segments.len(), 2, "3 stops in a row ⇒ 2 segments");
    for seg in &g.segments {
        assert!(seg.cells.len() >= 2);
        assert_eq!(seg.cells[0], g.nodes[seg.a as usize].cell, "cells start at endpoint a");
        assert_eq!(*seg.cells.last().unwrap(), g.nodes[seg.b as usize].cell, "cells end at endpoint b");
        // adjacency: consecutive cells are hex-neighbours (distance 1).
        for w2 in seg.cells.windows(2) {
            assert_eq!(hexgrid::distance(w2[0], w2[1]), 1, "segment steps one cell at a time");
        }
        assert!(!seg.shared, "a single line shares nothing");
    }
    // Every station's cell is present as a Station node.
    for s in [s0, s1, s2] {
        let c = cell_of(&w, s);
        let ni = g.node_at(c).expect("station is a node");
        assert_eq!(g.nodes[ni as usize].kind, NodeKind::Station);
        assert_eq!(g.nodes[ni as usize].station, Some(s.0));
    }
}

#[test]
fn termini_are_degree1_interior_stop_is_degree2() {
    let mut w = grid_world(CELL);
    let (a, b, c) = (xcell(0), xcell(3), xcell(6));
    let s0 = place(&mut w, a.0, a.1);
    let s1 = place(&mut w, b.0, b.1);
    let s2 = place(&mut w, c.0, c.1);
    make_line(&mut w, &[s0, s1, s2]);
    let g = derive_track_graph(&w);
    let deg = |s: StationId| g.nodes[g.node_at(cell_of(&w, s)).unwrap() as usize].degree;
    assert_eq!(deg(s0), 1, "endpoint is a terminus (degree 1)");
    assert_eq!(deg(s2), 1, "endpoint is a terminus (degree 1)");
    assert_eq!(deg(s1), 2, "a mid-line stop is degree 2 (still a node, because it's a station)");
}

#[test]
fn two_lines_share_one_segment() {
    // Both lines run the same A–B trunk on the x-axis (identical cells ⇒ identical edges), diverging at
    // the ends — the LITE shared-station-trunk guarantee. The A–B run must be ONE shared segment.
    let mut w = grid_world(CELL);
    let a = place(&mut w, xcell(3).0, xcell(3).1); // (3,0)
    let b = place(&mut w, xcell(6).0, xcell(6).1); // (6,0)
    let s1 = place(&mut w, xcell(0).0, xcell(0).1); // (0,0)
    let e1 = place(&mut w, xcell(9).0, xcell(9).1); // (9,0)
    let s2 = place(&mut w, at_cell(0, 3).0, at_cell(0, 3).1); // (0,3)
    let e2 = place(&mut w, at_cell(9, 3).0, at_cell(9, 3).1); // (9,3)
    make_line(&mut w, &[s1, a, b, e1]);
    make_line(&mut w, &[s2, a, b, e2]);
    let g = derive_track_graph(&w);

    let (ca, cb) = (cell_of(&w, a), cell_of(&w, b));
    // exactly one segment connects A and B, and it is shared.
    let shared_seg: Vec<_> = g
        .segments
        .iter()
        .filter(|s| {
            let (na, nb) = (g.nodes[s.a as usize].cell, g.nodes[s.b as usize].cell);
            (na == ca && nb == cb) || (na == cb && nb == ca)
        })
        .collect();
    assert_eq!(shared_seg.len(), 1, "the shared trunk collapses to ONE segment, not two ribbons");
    assert!(shared_seg[0].shared, "the A–B trunk is traversed by 2 lines ⇒ shared");
    // A and B are degree-3 junction-or-station nodes (two approaches + the trunk).
    assert!(g.nodes[g.node_at(ca).unwrap() as usize].degree >= 3);
    assert!(g.nodes[g.node_at(cb).unwrap() as usize].degree >= 3);
}

#[test]
fn command_order_independent() {
    // Build the same network two ways (stations in different orders) ⇒ identical derived graph.
    let build = |order: u8| -> sim::track_graph::TrackGraph {
        let mut w = grid_world(CELL);
        if order == 0 {
            let s0 = place(&mut w, xcell(1).0, xcell(1).1);
            let s1 = place(&mut w, xcell(4).0, xcell(4).1);
            let s2 = place(&mut w, xcell(7).0, xcell(7).1);
            make_line(&mut w, &[s0, s1, s2]);
        } else {
            // place in reverse, line drawn the other way
            let s2 = place(&mut w, xcell(7).0, xcell(7).1);
            let s1 = place(&mut w, xcell(4).0, xcell(4).1);
            let s0 = place(&mut w, xcell(1).0, xcell(1).1);
            make_line(&mut w, &[s2, s1, s0]);
        }
        derive_track_graph(&w)
    };
    let g0 = build(0);
    let g1 = build(1);
    let cells0: Vec<_> = g0.nodes.iter().map(|n| n.cell).collect();
    let cells1: Vec<_> = g1.nodes.iter().map(|n| n.cell).collect();
    assert_eq!(cells0, cells1, "node cells are canonical regardless of build order");
    let seg0: Vec<_> = g0.segments.iter().map(|s| (g0.nodes[s.a as usize].cell, g0.nodes[s.b as usize].cell, s.cells.clone())).collect();
    let seg1: Vec<_> = g1.segments.iter().map(|s| (g1.nodes[s.a as usize].cell, g1.nodes[s.b as usize].cell, s.cells.clone())).collect();
    assert_eq!(seg0, seg1, "segments (endpoints + cell chains) are canonical regardless of build order");
}

#[test]
fn derivation_is_replay_deterministic_and_hash_neutral() {
    // Same seed + same grid log, built + ticked twice ⇒ identical state_hash AND identical derived graph.
    let run = || -> (u64, usize, usize) {
        let mut w = grid_world(CELL);
        let s0 = place(&mut w, xcell(1).0, xcell(1).1);
        let s1 = place(&mut w, xcell(5).0, xcell(5).1);
        make_line(&mut w, &[s0, s1]);
        w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
        w.apply(&Command::SetRunning { running: true });
        for _ in 0..200 {
            w.tick(50);
        }
        let g = derive_track_graph(&w);
        (w.state_hash(), g.nodes.len(), g.segments.len())
    };
    let a = run();
    let b = run();
    assert_eq!(a, b, "replay is bit-for-bit identical (hash + derived graph shape)");
}

#[test]
fn canonical_orientation_invariant_holds() {
    let mut w = grid_world(CELL);
    let a = place(&mut w, xcell(3).0, xcell(3).1);
    let b = place(&mut w, xcell(6).0, xcell(6).1);
    let s1 = place(&mut w, xcell(0).0, xcell(0).1);
    let e1 = place(&mut w, xcell(9).0, xcell(9).1);
    let s2 = place(&mut w, at_cell(0, 3).0, at_cell(0, 3).1);
    let e2 = place(&mut w, at_cell(9, 3).0, at_cell(9, 3).1);
    make_line(&mut w, &[s1, a, b, e1]);
    make_line(&mut w, &[s2, a, b, e2]);
    let g = derive_track_graph(&w);
    for (i, s) in g.segments.iter().enumerate() {
        assert_eq!(s.seg_id, i as u32, "seg_id == canonical index");
        assert!(g.nodes[s.a as usize].cell <= g.nodes[s.b as usize].cell, "endpoint a is the smaller cell");
        assert_eq!(s.cells[0], g.nodes[s.a as usize].cell, "cells oriented a→b");
        assert_eq!(*s.cells.last().unwrap(), g.nodes[s.b as usize].cell);
    }
    // node cells are strictly increasing (sorted + unique).
    for w2 in g.nodes.windows(2) {
        assert!(w2[0].cell < w2[1].cell, "nodes sorted + unique by cell");
    }
}

/// TTD L3 A0: the graph structs are serde round-trippable (the wire/save contract for the future
/// authoritative segment store). A derived graph must postcard-encode → decode back equal — pinning that
/// the `Serialize`/`Deserialize` derives are coherent before L3 makes segments authoritative + hashed.
#[test]
fn track_graph_postcard_round_trips() {
    let mut w = grid_world(CELL);
    let a = place(&mut w, xcell(3).0, xcell(3).1);
    let b = place(&mut w, xcell(6).0, xcell(6).1);
    let s1 = place(&mut w, xcell(0).0, xcell(0).1);
    let e1 = place(&mut w, xcell(9).0, xcell(9).1);
    let s2 = place(&mut w, at_cell(0, 3).0, at_cell(0, 3).1);
    let e2 = place(&mut w, at_cell(9, 3).0, at_cell(9, 3).1);
    make_line(&mut w, &[s1, a, b, e1]);
    make_line(&mut w, &[s2, a, b, e2]); // a shared a→b segment, junction nodes — a non-trivial graph
    let g = derive_track_graph(&w);
    assert!(!g.segments.is_empty() && !g.nodes.is_empty(), "non-trivial graph for a meaningful round-trip");
    let bytes = postcard::to_allocvec(&g).expect("postcard encode the derived TrackGraph");
    let back: sim::track_graph::TrackGraph = postcard::from_bytes(&bytes).expect("postcard decode");
    // Structural equality (the structs aren't PartialEq): node + segment shape survives the round-trip.
    assert_eq!(back.nodes.len(), g.nodes.len());
    assert_eq!(back.segments.len(), g.segments.len());
    for (x, y) in back.nodes.iter().zip(&g.nodes) {
        assert_eq!((x.cell, x.station, x.degree), (y.cell, y.station, y.degree));
        assert_eq!(x.kind, y.kind);
    }
    for (x, y) in back.segments.iter().zip(&g.segments) {
        assert_eq!((x.seg_id, x.a, x.b, x.shared), (y.seg_id, y.a, y.b, y.shared));
        assert_eq!(x.cells, y.cells, "the ordered cell chain survives the round-trip");
    }
}

/// TTD L3 A1: each segment owns a DERIVED smoothed geometry sourced from the owning line's `Path.polyline`
/// sub-range. For a single straight line (single-line segments, no sharing) the segment's `polyline` must
/// equal the owning path's vertices for that segment's cell sub-range BIT-FOR-BIT, `arclen_mm` must be
/// cumulative + monotonic from 0 with `length_mm()` == the last arclen, and concatenating all of a path's
/// segments' polylines (deduping the shared endpoint vertex) must reproduce the full path polyline.
#[test]
fn segment_geometry_matches_owning_path_subrange() {
    let mut w = grid_world(CELL);
    // A single straight line on the x-axis through several stations ⇒ single-line segments, no sharing.
    let s0 = place(&mut w, xcell(0).0, xcell(0).1);
    let s1 = place(&mut w, xcell(3).0, xcell(3).1);
    let s2 = place(&mut w, xcell(7).0, xcell(7).1);
    let s3 = place(&mut w, xcell(12).0, xcell(12).1);
    make_line(&mut w, &[s0, s1, s2, s3]);
    let g = derive_track_graph(&w);
    assert_eq!(g.segments.len(), 3, "4 stops in a row ⇒ 3 single-line segments");

    let path = &w.lines[0].paths[0];
    // Map each path-polyline vertex to its cell (a grid-walk vertex is a cell centre ⇒ axial_of recovers it).
    let pcells: Vec<_> = path.polyline.iter().map(|&p| hexgrid::axial_of(p, CELL)).collect();

    for seg in &g.segments {
        assert!(!seg.shared, "single line shares nothing");
        assert!(seg.polyline.len() >= 2, "segment geometry has at least its two endpoints");
        assert_eq!(seg.polyline.len(), seg.cells.len(), "one vertex per cell in the chain");
        assert_eq!(seg.polyline.len(), seg.arclen_mm.len(), "arclen parallels the polyline");

        // Find this segment's cell sub-range in the owning path's polyline (forward, single line).
        let start = (0..=pcells.len() - seg.cells.len())
            .find(|&i| pcells[i..i + seg.cells.len()] == seg.cells[..])
            .expect("segment cell chain is a contiguous sub-range of the owning path polyline");
        let end = start + seg.cells.len();

        // BIT-FOR-BIT: the segment polyline equals the path's vertices for that sub-range.
        assert_eq!(&seg.polyline[..], &path.polyline[start..end], "segment polyline == owning path sub-range");

        // arclen_mm is cumulative + strictly monotonic from 0, and matches the (rebased) path sub-range.
        assert_eq!(seg.arclen_mm[0], 0, "arclen rebased to 0 at the first vertex");
        let base = path.arclen_mm[start];
        for (k, &a) in seg.arclen_mm.iter().enumerate() {
            assert_eq!(a, path.arclen_mm[start + k] - base, "arclen == rebased path arclen sub-range");
        }
        for win in seg.arclen_mm.windows(2) {
            assert!(win[1] > win[0], "arclen strictly increasing along a non-degenerate segment");
        }
        assert_eq!(seg.length_mm(), *seg.arclen_mm.last().unwrap(), "length_mm == last arclen");

        // point_at hits the endpoints exactly (integer-lerp parity with Path::point_at).
        assert_eq!(seg.point_at(0), (seg.polyline[0].x_mm, seg.polyline[0].y_mm));
        let last = seg.polyline[seg.polyline.len() - 1];
        assert_eq!(seg.point_at(seg.length_mm()), (last.x_mm, last.y_mm));
    }

    // Concatenating all segments' polylines (in path order along the cells) reproduces the full path
    // polyline, deduping the shared endpoint vertex between consecutive segments.
    let mut ordered: Vec<&_> = g.segments.iter().collect();
    // Order segments by where their cell chain starts in the path.
    ordered.sort_by_key(|seg| {
        (0..=pcells.len() - seg.cells.len())
            .find(|&i| pcells[i..i + seg.cells.len()] == seg.cells[..])
            .unwrap()
    });
    let mut concat: Vec<_> = Vec::new();
    for (si, seg) in ordered.iter().enumerate() {
        let skip = if si == 0 { 0 } else { 1 }; // dedup the shared endpoint vertex
        concat.extend_from_slice(&seg.polyline[skip..]);
    }
    assert_eq!(concat, path.polyline, "segments concatenate back to the full path polyline");
}

#[test]
fn degenerate_inputs_do_not_panic() {
    let mut w = grid_world(CELL);
    // a 1-stop line (no edges), adjacent stops (short run), a removed line.
    let s0 = place(&mut w, xcell(0).0, xcell(0).1);
    let _solo = make_line(&mut w, &[s0]); // 1 stop, filtered out (stops < 2)
    let s1 = place(&mut w, xcell(1).0, xcell(1).1); // adjacent to s0
    let l = make_line(&mut w, &[s0, s1]);
    w.apply(&Command::RemoveLine { line: l });
    let s2 = place(&mut w, xcell(4).0, xcell(4).1);
    make_line(&mut w, &[s0, s2]);
    let g = derive_track_graph(&w); // must not panic
    // the live line s0→s2 yields 2 nodes + 1 segment; the removed/solo lines contribute nothing.
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.segments.len(), 1);
}
