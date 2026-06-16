//! Derived TrackGraph (TTD L1, docs/ttd-track-model.md) — the shared physical INFRASTRUCTURE abstracted
//! out of the per-line polylines. Today a `Line` owns its geometry; this promotes what
//! `dispatch::derive_cross_blocks` already computes (grid edges over the hex lattice) into a first-class
//! graph: **nodes** = stations + lattice junction cells (degree ≠ 2) + termini (degree 1); **segments** =
//! maximal runs of grid edges between consecutive nodes (through degree-2 interior cells). Two lines over
//! the same corridor collapse to ONE segment, so the network reads as infrastructure, not N stacked ribbons.
//!
//! **DERIVED & NEVER HASHED.** Like `cross_blocks`/`junctions`/`serving`, this is a pure function of the
//! already-hashed line/station topology, re-derived in `dispatch` and stored in a non-`Canonical` field —
//! so it cannot move `state_hash` (zero re-pins). Integer-only, sorted-`Vec` (no HashMap iteration); the
//! only floats are inside `hexgrid::axial_of`/`center_of`, which quantise to `Axial`/`i64` mm immediately.
//! Empty (inert) for continuous / non-grid networks (`grid_cell_mm <= 0`) — that geometry never shares
//! exact vertices, so a graph is meaningless there (p5-shared-track-roadmap.md). Later layers (L2 berths,
//! L3 authoritative geometry, L4 routing) graduate this from observational to load-bearing.
use crate::hexgrid::{self, Axial};
use crate::world::World;

/// Why a cell is a graph vertex. A station cell is always a `Station` node (even at degree 2 — a mid-line
/// stop genuinely splits a run); otherwise degree ≥ 3 is a `Junction` and degree 1 a `Terminus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Station,
    Junction,
    Terminus,
}

/// A graph node: one hex cell that is a station, a junction (degree ≥ 3), or a terminus (degree 1).
#[derive(Clone, Debug)]
pub struct TrackNode {
    /// The cell's axial identity `(q, r)` — the node's identity and sort key.
    pub cell: Axial,
    /// The cell centre in mm (`center_of`) — the render anchor. Render-only; never an ordering key.
    pub pos_mm: (i64, i64),
    pub kind: NodeKind,
    /// `StationId.0` when a station snaps to this cell, else `None` (a bare junction/terminus).
    pub station: Option<u32>,
    /// Distinct incident segments (1..=6 on a hex lattice).
    pub degree: u8,
}

/// A maximal run of grid edges between two graph nodes, passing only through degree-2 interior cells. The
/// geometry is the ordered cell chain `cells[0] == nodes[a].cell ..= cells[last] == nodes[b].cell`.
#[derive(Clone, Debug)]
pub struct TrackSegment {
    /// Index in the canonical (endpoint-cell-sorted) segment order.
    pub seg_id: u32,
    /// Endpoint node index with the SMALLER cell (canonical orientation `nodes[a].cell <= nodes[b].cell`).
    pub a: u32,
    /// Endpoint node index with the larger cell.
    pub b: u32,
    /// Ordered cell chain `a..=b` (≥ 2 cells), oriented so `cells[0] == nodes[a].cell`.
    pub cells: Vec<Axial>,
    /// Traversed by ≥ 2 distinct lines — the cross-line shared-infrastructure hint (drives the fat ribbon).
    pub shared: bool,
}

/// The derived track graph: nodes sorted by `cell`, segments sorted canonically. Empty for non-grid.
#[derive(Clone, Debug, Default)]
pub struct TrackGraph {
    pub nodes: Vec<TrackNode>,
    pub segments: Vec<TrackSegment>,
}

impl TrackGraph {
    /// Node index whose cell == `cell` (binary search over the cell-sorted `nodes`), or `None`.
    #[inline]
    pub fn node_at(&self, cell: Axial) -> Option<u32> {
        self.nodes.binary_search_by(|n| n.cell.cmp(&cell)).ok().map(|i| i as u32)
    }
}

/// The other endpoint of a canonical edge `(c0, c1)` reached from `from`.
#[inline]
fn other_end(edge: (Axial, Axial), from: Axial) -> Axial {
    if edge.0 == from {
        edge.1
    } else {
        edge.0
    }
}

/// Build the derived [`TrackGraph`] from the world's GRID lines. Pure function of `lines`/`stations`/
/// `grid_cell_mm`; deterministic (every input sorted before use), command-order-independent, integer-only.
pub fn derive_track_graph(world: &World) -> TrackGraph {
    let cell = world.city.grid_cell_mm;
    if cell <= 0 {
        return TrackGraph::default();
    }

    // 1. Collect every grid edge-use across BUILT lines (track exists without trains — unlike the dispatch
    //    mutex, we do NOT require a trainset; a corridor is infrastructure the moment it's drawn). Mirrors
    //    `derive_cross_blocks` edge collection: canonical (min,max) cell pair, zero-length edges skipped.
    let mut uses: Vec<((Axial, Axial), u32)> = Vec::new(); // (canonical edge, line index)
    for (li, line) in world.lines.iter().enumerate() {
        if line.removed || line.stops.len() < 2 || line.crosses_water_surface {
            continue;
        }
        for path in &line.paths {
            let poly = &path.polyline;
            for i in 0..poly.len().saturating_sub(1) {
                let a = hexgrid::axial_of(poly[i], cell);
                let b = hexgrid::axial_of(poly[i + 1], cell);
                if a == b {
                    continue;
                }
                let edge = if a <= b { (a, b) } else { (b, a) };
                uses.push((edge, li as u32));
            }
        }
    }
    if uses.is_empty() {
        return TrackGraph::default();
    }

    // 2. Group by edge → ONE unique infrastructure edge per physical edge, `shared` = ≥2 distinct lines.
    uses.sort();
    let mut edges: Vec<((Axial, Axial), bool)> = Vec::new();
    let mut g = 0;
    while g < uses.len() {
        let edge = uses[g].0;
        let mut h = g;
        // `uses` is sorted by (edge, line) ⇒ within an edge group the line ids ascend; a distinct line is
        // one that differs from the previous (O(run) not O(run²), and shared = ≥2 distinct lines).
        let mut distinct = 0u32;
        let mut prev_line = u32::MAX;
        while h < uses.len() && uses[h].0 == edge {
            if uses[h].1 != prev_line {
                distinct += 1;
                prev_line = uses[h].1;
            }
            h += 1;
        }
        edges.push((edge, distinct >= 2));
        g = h;
    }

    // 3. Cell degree: incidence (cell, edge_idx) for both endpoints, sorted; a cell's degree = its run len
    //    (no self-loops since a==b edges were skipped, so each incident edge contributes exactly one entry).
    let mut inc: Vec<(Axial, usize)> = Vec::with_capacity(edges.len() * 2);
    for (ei, &(e, _)) in edges.iter().enumerate() {
        inc.push((e.0, ei));
        inc.push((e.1, ei));
    }
    inc.sort();
    // incident edges of a cell: the edge indices in its `inc` run (degree = run length, computed in step 5).
    let incident_of = |c: Axial| -> &[(Axial, usize)] {
        let lo = inc.partition_point(|&(cc, _)| cc < c);
        let hi = inc.partition_point(|&(cc, _)| cc <= c);
        &inc[lo..hi]
    };

    // 4. Station cells = the cells STOPPED AT by a live line (trunk + branch stops), sorted + deduped to
    //    the lowest StationId per cell. A node is a station the network actually halts at — NOT any placed
    //    station whose cell a corridor merely passes through (e.g. a stop left over from a deleted line).
    let mut sc: Vec<(Axial, u32)> = Vec::new();
    let push_stop = |sc: &mut Vec<(Axial, u32)>, s: crate::ids::StationId| {
        if let Some(st) = world.stations.get(s.0 as usize) {
            if !st.removed {
                sc.push((hexgrid::axial_of(st.pos, cell), s.0));
            }
        }
    };
    for line in &world.lines {
        if line.removed || line.stops.len() < 2 || line.crosses_water_surface {
            continue;
        }
        for &s in &line.stops {
            push_stop(&mut sc, s);
        }
        for b in &line.branches {
            for &s in &b.stops {
                push_stop(&mut sc, s);
            }
        }
    }
    sc.sort();
    sc.dedup_by_key(|x| x.0);
    let station_at = |c: Axial| -> Option<u32> { sc.binary_search_by(|x| x.0.cmp(&c)).ok().map(|i| sc[i].1) };

    // 5. Classify node cells: a cell is a NODE iff it is a station cell OR degree != 2. Walk the unique
    //    cells of `inc` in sorted order (already canonical) → cell-sorted `nodes`.
    let mut nodes: Vec<TrackNode> = Vec::new();
    let mut k = 0;
    while k < inc.len() {
        let c = inc[k].0;
        let mut kk = k + 1;
        while kk < inc.len() && inc[kk].0 == c {
            kk += 1;
        }
        let degree = kk - k;
        k = kk;
        let station = station_at(c);
        if station.is_none() && degree == 2 {
            continue; // interior through-cell, not a node
        }
        let kind = if station.is_some() {
            NodeKind::Station
        } else if degree >= 3 {
            NodeKind::Junction
        } else {
            NodeKind::Terminus
        };
        let p = hexgrid::center_of(c, cell);
        nodes.push(TrackNode { cell: c, pos_mm: (p.x_mm, p.y_mm), kind, station, degree: degree.min(255) as u8 });
    }

    // 6. Contract degree-2 runs into segments. From each node (canonical order), walk each not-yet-consumed
    //    incident edge through interior cells to the next node. `seen` over edges bounds the walk (no
    //    infinite loop ⇒ tick can't hang). `nodes` is now FINAL (immutable) — every edge's component
    //    contains a node (every line has >=2 stops = station nodes), so the node-rooted walks consume all
    //    edges; a hypothetical node-less ring (unreachable from a line) simply leaves its edges out (no
    //    panic, no mutation), since a graph segment needs two node endpoints.
    let is_node = |c: Axial| -> bool { nodes.binary_search_by(|n| n.cell.cmp(&c)).is_ok() };
    let node_idx = |c: Axial| -> Option<u32> { nodes.binary_search_by(|n| n.cell.cmp(&c)).ok().map(|i| i as u32) };
    let mut seen = vec![false; edges.len()];
    let mut raw: Vec<(u32, u32, Vec<Axial>, bool)> = Vec::new(); // (node a, node b, cells a..b, shared)
    let walk_from = |start: Axial, first_ei: usize, seen: &mut [bool]| -> Option<(u32, u32, Vec<Axial>, bool)> {
        let mut cells = vec![start];
        let mut shared = false;
        let mut cur = start;
        let mut ei = first_ei;
        let mut guard = 0usize;
        loop {
            seen[ei] = true;
            shared |= edges[ei].1;
            let nxt = other_end(edges[ei].0, cur);
            cells.push(nxt);
            cur = nxt;
            if is_node(cur) || guard > edges.len() {
                break;
            }
            // interior (degree 2): take the OTHER incident edge.
            let next_ei = incident_of(cur).iter().map(|&(_, e)| e).find(|&e| e != ei);
            match next_ei {
                Some(e) if !seen[e] => ei = e,
                _ => break,
            }
            guard += 1;
        }
        // a real segment needs both endpoints to be nodes (a non-node end ⇒ a degenerate/ring walk → drop).
        Some((node_idx(start)?, node_idx(cur)?, cells, shared))
    };
    for ni in 0..nodes.len() {
        let c = nodes[ni].cell;
        let starts: Vec<usize> = incident_of(c).iter().map(|&(_, e)| e).collect();
        for ei in starts {
            if !seen[ei] {
                if let Some(seg) = walk_from(c, ei, &mut seen) {
                    raw.push(seg);
                }
            }
        }
    }

    // 7. Canonicalise: orient each segment so cells[0] is the smaller-cell endpoint, sort by endpoint
    //    cells (then first interior cell to disambiguate parallel runs), assign seg_id.
    let mut segments: Vec<TrackSegment> = raw
        .into_iter()
        .map(|(a, b, mut cells, shared)| {
            let (na, nb) = (a as usize, b as usize);
            let (a, b) = if nodes[na].cell <= nodes[nb].cell { (a, b) } else { (b, a) };
            if cells.first() != Some(&nodes[a as usize].cell) {
                cells.reverse();
            }
            TrackSegment { seg_id: 0, a, b, cells, shared }
        })
        .collect();
    segments.sort_by(|s, t| {
        (nodes[s.a as usize].cell, nodes[s.b as usize].cell, s.cells.get(1).copied())
            .cmp(&(nodes[t.a as usize].cell, nodes[t.b as usize].cell, t.cells.get(1).copied()))
    });
    for (i, s) in segments.iter_mut().enumerate() {
        s.seg_id = i as u32;
    }

    TrackGraph { nodes, segments }
}
