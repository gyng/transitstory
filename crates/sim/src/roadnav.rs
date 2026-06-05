//! Class-following navigation: grid A* over the buildability raster, used to route a line's
//! inter-stop span ALONG cells of a preferred class instead of straight track — BUSES follow
//! `class::ROAD`, FERRIES follow `class::WATER`. Pure integer cost + index-ordered expansion →
//! deterministic; the result is fed as pass-through points to the Catmull-Rom smoother (like
//! player waypoints, but auto-derived).
use crate::city::class;
use crate::geo_local::PointMm;
use rustc_hash::FxHashMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const MAX_CELLS: usize = 60_000; // bound the search; beyond this a bus just goes straight
const ENTER_PREFERRED: i64 = 10; // cost to step into a cell of the preferred class (cheap — follow it)
const ENTER_OFF: i64 = 100; // cost to step into anything else (leaving the corridor)

#[inline]
fn cell_of(p: PointMm, cell_mm: i64) -> (i64, i64) {
    (p.x_mm.div_euclid(cell_mm), p.y_mm.div_euclid(cell_mm))
}

/// Route the span `from`→`to` along cells of class `prefer`, returning the intermediate points
/// (simplified cell centres) the line should thread to follow that corridor — or EMPTY (⇒ straight
/// track) when neither end touches `prefer`, the search box is too big, or the path is straight.
pub(crate) fn class_route(
    lookup: &FxHashMap<(i32, i32), u8>,
    cell_mm: i64,
    prefer: u8,
    from: PointMm,
    to: PointMm,
) -> Vec<PointMm> {
    if cell_mm <= 0 {
        return Vec::new();
    }
    let cls = |cx: i64, cy: i64| lookup.get(&(cx as i32, cy as i32)).copied().unwrap_or(class::OPEN);
    let (fx, fy) = cell_of(from, cell_mm);
    let (tx, ty) = cell_of(to, cell_mm);
    // Only route when at least one end is on the preferred class; else the line just goes straight.
    if cls(fx, fy) != prefer && cls(tx, ty) != prefer {
        return Vec::new();
    }

    // Pad the box by ~the stop separation so a road can DETOUR off the straight line (a small
    // fixed margin would clip any real detour). Long spans blow past MAX_CELLS → straight.
    let margin = (fx - tx).abs().max((fy - ty).abs()).max(6);
    let (minx, maxx) = (fx.min(tx) - margin, fx.max(tx) + margin);
    let (miny, maxy) = (fy.min(ty) - margin, fy.max(ty) + margin);
    let w = (maxx - minx + 1) as usize;
    let h = (maxy - miny + 1) as usize;
    if w == 0 || h == 0 || w.saturating_mul(h) > MAX_CELLS {
        return Vec::new();
    }

    let idx = |lx: i64, ly: i64| (ly * w as i64 + lx) as usize;
    let start = idx(fx - minx, fy - miny);
    let goal = idx(tx - minx, ty - miny);
    let (glx, gly) = (tx - minx, ty - miny);
    let heur = |lx: i64, ly: i64| (lx - glx).abs().max((ly - gly).abs()) * ENTER_PREFERRED;

    let mut dist = vec![i64::MAX; w * h];
    let mut came: Vec<i32> = vec![-1; w * h];
    let mut done = vec![false; w * h];
    dist[start] = 0;
    let mut heap: BinaryHeap<Reverse<(i64, u32)>> = BinaryHeap::new();
    heap.push(Reverse((heur(fx - minx, fy - miny), start as u32)));

    // 8-neighbour steps (dx, dy, factor): orthogonal ×10, diagonal ×14 (≈ √2) — integer.
    const NB: [(i64, i64, i64); 8] =
        [(1, 0, 10), (-1, 0, 10), (0, 1, 10), (0, -1, 10), (1, 1, 14), (1, -1, 14), (-1, 1, 14), (-1, -1, 14)];
    while let Some(Reverse((_f, node))) = heap.pop() {
        let node = node as usize;
        if done[node] {
            continue; // stale heap entry (a better path was already settled)
        }
        done[node] = true;
        if node == goal {
            break;
        }
        let (lx, ly) = ((node % w) as i64, (node / w) as i64);
        let g = dist[node];
        for &(dx, dy, fac) in &NB {
            let (nx, ny) = (lx + dx, ly + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let ncell = idx(nx, ny);
            if done[ncell] {
                continue;
            }
            let enter = if cls(minx + nx, miny + ny) == prefer { ENTER_PREFERRED } else { ENTER_OFF };
            let ng = g.saturating_add(enter * fac / 10);
            if ng < dist[ncell] {
                dist[ncell] = ng;
                came[ncell] = node as i32;
                heap.push(Reverse((ng.saturating_add(heur(nx, ny)), ncell as u32)));
            }
        }
    }
    if dist[goal] == i64::MAX {
        return Vec::new(); // unreachable on an open grid (shouldn't happen) — go straight
    }

    // Reconstruct goal→start, to cell-centre points.
    let center = |node: usize| -> PointMm {
        let (lx, ly) = ((node % w) as i64, (node / w) as i64);
        PointMm::new((minx + lx) * cell_mm + cell_mm / 2, (miny + ly) * cell_mm + cell_mm / 2)
    };
    let mut nodes = Vec::new();
    let (mut cur, mut guard) = (goal as i32, 0usize);
    while cur != -1 && guard <= w * h {
        nodes.push(cur as usize);
        if cur as usize == start {
            break;
        }
        cur = came[cur as usize];
        guard += 1;
    }
    nodes.reverse();
    let pts: Vec<PointMm> = nodes.iter().map(|&n| center(n)).collect();

    // Keep only direction-change vertices (drop collinear runs) and the two endpoints (handled by
    // the stops themselves), so the smoother gets a compact set of road bends.
    let mut out = Vec::new();
    for i in 1..pts.len().saturating_sub(1) {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let cross = (b.x_mm - a.x_mm) * (c.y_mm - b.y_mm) - (b.y_mm - a.y_mm) * (c.x_mm - b.x_mm);
        if cross != 0 {
            out.push(b);
        }
    }
    out
}
