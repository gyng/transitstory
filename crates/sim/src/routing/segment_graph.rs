//! TTD L4b — deterministic least-cost search over the derived segment graph (docs/ttd-l4-plan.md).
//!
//! A PURE primitive: given a [`TrackGraph`] (with its L4a CSR adjacency) and a source/destination NODE,
//! `route_segments` returns the cheapest ordered chain of `(TrackSegmentId, reverse)` edges from `src` to
//! `dst`, or `None` if unreachable. Edge cost is the segment's integer `length_mm()`; `reverse` is the
//! chosen traversal direction vs the segment's canonical `cells[0] → cells[last]` orientation.
//!
//! **Golden- AND fingerprint-neutral by construction:** no caller is wired (L4c/L4h will consume it), it
//! reads only the DERIVED, non-`Canonical` `TrackGraph`, and it touches no vehicle/world state.
//!
//! **Determinism:** integer Dijkstra with `Vec`-indexed `dist`/`prev` over node indices (no HashMap
//! iteration, no float, no wall-clock). The frontier is a min-heap of `Reverse<(cost_mm, seg_tiebreak,
//! node)>` — so among equal-cost relaxations the lower incoming seg_id wins, and among equal `(cost, seg)`
//! the lower node index wins. Every comparison is a total integer order, so two runs are bit-identical.
use crate::ids::TrackSegmentId;
use crate::track_graph::TrackGraph;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Least-cost segment chain from `src_node` to `dst_node` over `graph` (edge cost = `length_mm()`, ties
/// broken by the lower incoming seg_id, then the lower node index). Returns the ordered
/// `(TrackSegmentId, reverse)` edges to traverse — `reverse == true` means the segment is walked against
/// its canonical `cells[0] → cells[last]` orientation (i.e. entered at endpoint `b`). `None` if the
/// destination is unreachable, or if either node index is out of range. `src == dst` ⇒ `Some(vec![])`.
pub fn route_segments(graph: &TrackGraph, src_node: u32, dst_node: u32) -> Option<Vec<(TrackSegmentId, bool)>> {
    let n = graph.nodes.len();
    let (src, dst) = (src_node as usize, dst_node as usize);
    if src >= n || dst >= n {
        return None;
    }
    if src == dst {
        return Some(Vec::new());
    }

    // Vec-indexed labels (no HashMap iteration). `dist[v]` = best known cost to v (`i64::MAX` = ∞); `prev[v]`
    // = the edge we arrived by, `(predecessor node, seg_id, reverse)`.
    let mut dist: Vec<i64> = vec![i64::MAX; n];
    let mut prev: Vec<Option<(u32, u32, bool)>> = vec![None; n];
    let mut done: Vec<bool> = vec![false; n];

    // Frontier: Reverse → min-heap. Key (cost, seg_tiebreak, node) is a total integer order; `seg_tiebreak`
    // is the seg_id of the edge taken into `node` (u32::MAX for the source's phantom self-entry), so equal
    // costs deterministically prefer the lower incoming seg_id, then the lower node index.
    let mut heap: BinaryHeap<Reverse<(i64, u32, u32)>> = BinaryHeap::new();
    dist[src] = 0;
    heap.push(Reverse((0, u32::MAX, src_node)));

    while let Some(Reverse((cost, _seg_tb, u))) = heap.pop() {
        let ui = u as usize;
        if done[ui] {
            continue; // a stale, higher-cost entry for an already-finalised node
        }
        done[ui] = true;
        if ui == dst {
            break; // finalised the destination — its predecessor chain is fixed
        }

        // Incident seg_ids are already SORTED ascending (L4a), so relaxations are offered in canonical
        // order — the heap key still decides ties, but iterating sorted keeps the whole walk reproducible.
        for &seg_id in graph.incident(u) {
            let seg = &graph.segments[seg_id as usize];
            // The far endpoint of this segment from u. A segment with a == b can't relax to a new node.
            let (v, reverse) = if seg.a == u {
                (seg.b, false) // walked a→b = canonical orientation
            } else {
                (seg.a, true) // entered at b, walked b→a = reversed
            };
            let vi = v as usize;
            if done[vi] {
                continue;
            }
            let w = seg.length_mm();
            let nd = cost.saturating_add(w);
            if nd < dist[vi] {
                dist[vi] = nd;
                prev[vi] = Some((u, seg_id, reverse));
                heap.push(Reverse((nd, seg_id, v)));
            }
        }
    }

    if dist[dst] == i64::MAX {
        return None; // unreachable
    }

    // Walk predecessors dst → src, then reverse into traversal order.
    let mut chain: Vec<(TrackSegmentId, bool)> = Vec::new();
    let mut cur = dst;
    while cur != src {
        let (p, seg_id, reverse) = prev[cur].expect("reachable node has a predecessor edge");
        chain.push((TrackSegmentId(seg_id), reverse));
        cur = p as usize;
    }
    chain.reverse();
    Some(chain)
}
