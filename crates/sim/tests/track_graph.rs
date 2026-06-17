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

/// TTD L3 A2: each `Path` gains a DERIVED, NON-HASHED `segments: Vec<(TrackSegmentId, bool)>` — the
/// ordered `TrackGraph` segments it covers (bool = traversed in REVERSE). Bound post-dispatch by
/// `dispatch::bind_path_segments` (right after `derive_track_graph`), so a TICK fills it. For a single
/// straight grid line (single-line, unshared segments ⇒ the curvature representative IS this line), the
/// binding must (a) be non-empty + resolve into `world.track_graph.segments`, and (b) concatenate back
/// to the path's full `polyline` bit-for-bit (honouring the reverse flag, deduping the shared endpoint
/// vertex between consecutive segments). Shared segments are NOT asserted here (their geometry is the
/// lowest-index representative's curve — the documented A1/C1 subtlety).
#[test]
fn path_segments_bind_and_concatenate() {
    let mut w = grid_world(CELL);
    // A single straight line on the x-axis through 4 stations ⇒ 3 single-line, unshared segments.
    let s0 = place(&mut w, xcell(0).0, xcell(0).1);
    let s1 = place(&mut w, xcell(3).0, xcell(3).1);
    let s2 = place(&mut w, xcell(7).0, xcell(7).1);
    let s3 = place(&mut w, xcell(12).0, xcell(12).1);
    make_line(&mut w, &[s0, s1, s2, s3]);
    // Trigger dispatch (which derives the graph THEN binds the path segments): assign a trainset, run,
    // tick once. `dispatch` runs at the start of a tick on `dispatch_dirty`, filling `Path.segments`.
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    w.tick(50);

    let g = derive_track_graph(&w);
    assert_eq!(g.segments.len(), 3, "4 stops in a row ⇒ 3 single-line segments");

    let path = &w.lines[0].paths[0];
    // (a) non-empty + every bound seg id resolves into the derived graph's segments.
    assert!(!path.segments.is_empty(), "the path binds at least one segment after dispatch");
    assert_eq!(path.segments.len(), g.segments.len(), "the straight path covers every segment, once each");
    for &(sid, _rev) in &path.segments {
        assert!((sid.0 as usize) < g.segments.len(), "bound seg id resolves into track_graph.segments");
        assert_eq!(g.segments[sid.0 as usize].seg_id, sid.0, "seg_id == its index (canonical)");
    }

    // (b) concatenating the bound segments' polylines IN PATH ORDER (honouring the reverse flag, deduping
    // the shared boundary vertex between consecutive segments) reproduces the path's full polyline.
    let mut concat: Vec<sim::geo_local::PointMm> = Vec::new();
    for (i, &(sid, reverse)) in path.segments.iter().enumerate() {
        let seg = &g.segments[sid.0 as usize];
        // Orient the segment polyline along the direction this path traverses it.
        let mut poly = seg.polyline.clone();
        if reverse {
            poly.reverse();
        }
        let skip = if i == 0 { 0 } else { 1 }; // dedup the shared endpoint vertex
        concat.extend_from_slice(&poly[skip..]);
    }
    assert_eq!(concat, path.polyline, "bound segments concatenate back to the full path polyline bit-for-bit");
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

/// TTD L4a: the CSR node→incident-segment adjacency. On a branched grid (a junction where three corridors
/// meet) `incident(node)` must equal EXACTLY the seg_ids of segments with `a == node || b == node`, be
/// SORTED ascending by seg_id, and have `len() == node.degree` (each incident segment contributes once).
#[test]
fn incident_csr_matches_endpoints_sorted_len_eq_degree() {
    // A Y/T junction at J=(3,0): three stops hang off it on the x-axis and a branch up in r, so J is a
    // degree-3 node with three incident segments (each terminating at its own station node).
    let mut w = grid_world(CELL);
    let j = place(&mut w, xcell(3).0, xcell(3).1); // junction cell (3,0)
    let west = place(&mut w, xcell(0).0, xcell(0).1); // (0,0)
    let east = place(&mut w, xcell(6).0, xcell(6).1); // (6,0)
    let up = place(&mut w, at_cell(3, 3).0, at_cell(3, 3).1); // (3,3)
    // Two lines through J create the three corridors: west–east straight, and J–up branch.
    make_line(&mut w, &[west, j, east]);
    make_line(&mut w, &[j, up]);
    let g = derive_track_graph(&w);

    // For EVERY node, the CSR list must match the brute-force endpoint scan, sorted, len == degree.
    for (ni, node) in g.nodes.iter().enumerate() {
        let mut expected: Vec<u32> = g
            .segments
            .iter()
            .filter(|s| s.a as usize == ni || s.b as usize == ni)
            .map(|s| s.seg_id)
            .collect();
        expected.sort();
        let got = g.incident(ni as u32);
        assert_eq!(got, &expected[..], "incident(node) == segments with a==node||b==node");
        for win in got.windows(2) {
            assert!(win[0] < win[1], "incident list sorted ascending by seg_id");
        }
        assert_eq!(got.len(), node.degree as usize, "incident count == node degree");
    }

    // The junction is genuinely branched: degree 3, three incident segments.
    let ji = g.node_at(cell_of(&w, j)).unwrap();
    assert_eq!(g.nodes[ji as usize].degree, 3, "J is a degree-3 junction");
    assert_eq!(g.incident(ji).len(), 3, "J has 3 incident segments");

    // Out-of-range / unbuilt-CSR safety: total + non-panicking.
    assert!(g.incident(g.nodes.len() as u32).is_empty(), "out-of-range node ⇒ empty");
    assert!(sim::track_graph::TrackGraph::default().incident(0).is_empty(), "default graph ⇒ empty");
}

/// Total `length_mm` of a chosen `route_segments` chain over a graph (sum of each hop's segment length).
fn chain_len(g: &sim::track_graph::TrackGraph, chain: &[(sim::ids::TrackSegmentId, bool)]) -> i64 {
    chain.iter().map(|&(sid, _)| g.segments[sid.0 as usize].length_mm()).sum()
}

/// Assert a chosen chain is a contiguous walk from `src` to `dst`: hop 0 starts at src, each hop's exit node
/// is the next hop's entry node, and the last hop exits at dst. `reverse` picks the entry/exit endpoints.
fn assert_connected(g: &sim::track_graph::TrackGraph, chain: &[(sim::ids::TrackSegmentId, bool)], src: u32, dst: u32) {
    let mut cur = src;
    for &(sid, reverse) in chain {
        let seg = &g.segments[sid.0 as usize];
        let (entry, exit) = if reverse { (seg.b, seg.a) } else { (seg.a, seg.b) };
        assert_eq!(entry, cur, "each hop is entered at the current node (reverse flag chooses the endpoint)");
        cur = exit;
    }
    assert_eq!(cur, dst, "the chain ends at the destination node");
}

/// Cost from `s` to `d` via the routing primitive itself (sum of the chosen chain's segment lengths), as
/// an independent re-measurement of an alternate route's length.
fn route_cost(g: &sim::track_graph::TrackGraph, s: u32, d: u32) -> i64 {
    let chain = sim::routing::segment_graph::route_segments(g, s, d).expect("connected");
    chain_len(g, &chain)
}

/// TTD L4b: `route_segments` is a deterministic least-cost segment search.
///
/// **Test graph — a DIAMOND on the hex grid.** Two stations A, B with two stations M1, M2 each wired
/// `A–M–B` by its own line. Because every station is a graph NODE and the off-axis Ms force divergence,
/// the derivation yields a clean 4-node / 4-segment diamond: two parallel 2-segment routes A→M→B. (The
/// hex lattice is NOT mirror-symmetric in cartesian mm, so reflecting r→−r does NOT give equal lengths —
/// the two cases below were each measured to be strictly-different / exactly-equal via the empirical
/// `examples/dbg_diamond.rs` probe and pinned by cell coordinate.)
///
/// Case 1 — STRICTLY-SHORTER route wins: M1=(4,1) near-axis (short detour) vs M2=(4,5) far off-axis (long).
/// Case 2 — EXACTLY-EQUAL parallel routes (A=(0,0),B=(4,0),M1=(2,1),M2=(2,−1), both totalling 866024 mm)
///          ⇒ the canonical tiebreak takes the lower-seg_id chain, and two runs are bit-identical.
#[test]
fn route_segments_picks_shortest_then_lower_seg_id_deterministically() {
    use sim::routing::segment_graph::route_segments;

    // --- (1) Strictly-shorter route wins. ---
    let mut w = grid_world(CELL);
    let a = place(&mut w, xcell(0).0, xcell(0).1); // (0,0)
    let b = place(&mut w, xcell(8).0, xcell(8).1);
    let m1 = place(&mut w, at_cell(4, 1).0, at_cell(4, 1).1); // near-axis ⇒ short detour
    let m2 = place(&mut w, at_cell(4, 5).0, at_cell(4, 5).1); // far off-axis ⇒ long detour
    make_line(&mut w, &[a, m1, b]);
    make_line(&mut w, &[a, m2, b]);
    let g = derive_track_graph(&w);

    let na = g.node_at(cell_of(&w, a)).unwrap();
    let nb = g.node_at(cell_of(&w, b)).unwrap();
    let nm1 = g.node_at(cell_of(&w, m1)).unwrap();
    let nm2 = g.node_at(cell_of(&w, m2)).unwrap();

    let route = route_segments(&g, na, nb).expect("A and B are connected");
    assert_connected(&g, &route, na, nb);
    // The route must thread the M1 node and avoid the M2 node (both are degree-2 ⇒ a route through them
    // visits the node). Recover each hop's exit node from the reverse flag.
    let mut visited: Vec<u32> = vec![na];
    for &(sid, rev) in &route {
        let s = &g.segments[sid.0 as usize];
        visited.push(if rev { s.a } else { s.b }); // each hop's exit node
    }
    assert!(visited.contains(&nm1), "shortest route threads M1");
    assert!(!visited.contains(&nm2), "shortest route does NOT thread the long M2 detour");

    // Independently re-measure the two parallel route costs THROUGH each M node, via the primitive itself,
    // and confirm the chosen total equals the strictly-cheaper one.
    let via_m1 = route_cost(&g, na, nm1) + route_cost(&g, nm1, nb);
    let via_m2 = route_cost(&g, na, nm2) + route_cost(&g, nm2, nb);
    assert!(via_m1 < via_m2, "the M1 route is strictly shorter than the M2 route ({via_m1} < {via_m2})");
    assert_eq!(chain_len(&g, &route), via_m1, "chosen chain length == the strictly-shorter route");

    // src == dst ⇒ empty chain; out-of-range dst ⇒ None.
    assert_eq!(route_segments(&g, na, na), Some(Vec::new()), "src==dst ⇒ empty chain");
    assert_eq!(route_segments(&g, na, g.nodes.len() as u32), None, "out-of-range dst ⇒ None");

    // Unreachable destination ⇒ None: an ISOLATED disjoint line shares no cells with the diamond.
    let iso0 = place(&mut w, at_cell(0, 20).0, at_cell(0, 20).1);
    let iso1 = place(&mut w, at_cell(4, 20).0, at_cell(4, 20).1);
    make_line(&mut w, &[iso0, iso1]);
    let g_iso = derive_track_graph(&w);
    let na_i = g_iso.node_at(cell_of(&w, a)).unwrap();
    let niso = g_iso.node_at(cell_of(&w, iso0)).unwrap();
    assert_eq!(route_segments(&g_iso, na_i, niso), None, "a node in a disjoint component is unreachable ⇒ None");

    // --- (2) EXACTLY-EQUAL parallel routes ⇒ deterministic lower-seg_id chain, twice identical. ---
    // A=(0,0) B=(4,0), M1=(2,1) & M2=(2,−1): both A→M→B totals are 866024 mm (measured); A's two incident
    // segments are the two route starts, so the tiebreak must take the lower-seg_id first hop.
    let mut w2 = grid_world(CELL);
    let ea = place(&mut w2, at_cell(0, 0).0, at_cell(0, 0).1);
    let eb = place(&mut w2, at_cell(4, 0).0, at_cell(4, 0).1);
    let mt = place(&mut w2, at_cell(2, 1).0, at_cell(2, 1).1);
    let mb = place(&mut w2, at_cell(2, -1).0, at_cell(2, -1).1);
    make_line(&mut w2, &[ea, mt, eb]);
    make_line(&mut w2, &[ea, mb, eb]);
    let g2 = derive_track_graph(&w2);
    assert_eq!((g2.nodes.len(), g2.segments.len()), (4, 4), "a clean 4-node / 4-segment diamond");

    let na2 = g2.node_at(cell_of(&w2, ea)).unwrap();
    let nb2 = g2.node_at(cell_of(&w2, eb)).unwrap();
    let nmt = g2.node_at(cell_of(&w2, mt)).unwrap();
    let nmb = g2.node_at(cell_of(&w2, mb)).unwrap();

    // Confirm the two parallel routes are EXACTLY equal cost (the whole point of the tiebreak case).
    let via_t = route_cost(&g2, na2, nmt) + route_cost(&g2, nmt, nb2);
    let via_b = route_cost(&g2, na2, nmb) + route_cost(&g2, nmb, nb2);
    assert_eq!(via_t, via_b, "the two parallel routes are exactly equal-cost ({via_t} == {via_b})");

    let r1 = route_segments(&g2, na2, nb2).expect("connected");
    let r2 = route_segments(&g2, na2, nb2).expect("connected");
    assert_eq!(r1, r2, "two runs of the same query are bit-identical (deterministic)");
    assert_connected(&g2, &r1, na2, nb2);
    assert_eq!(chain_len(&g2, &r1), via_t, "the chosen equal-cost chain has the (shared) minimum cost");

    // The canonical tiebreak: among A's incident segments (the two route starts), the chosen first hop is
    // the MINIMUM seg_id.
    let first_seg = r1[0].0 .0;
    let min_inc = *g2.incident(na2).iter().min().unwrap();
    assert_eq!(first_seg, min_inc, "equal-cost tiebreak takes the lowest-seg_id first hop");
}
