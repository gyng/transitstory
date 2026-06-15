//! Hex-lattice primitives (fantasy-fork.md §10, fantasy-build-plan.md S5): pointy-top axial `(q, r)`
//! cells over the same `i64`-mm planar space `geo_local` uses. This is the geometry the fantasy 4X
//! map is built on; the transit game's default continuous Catmull-Rom path never calls it. Wired into
//! `line.rs::grid_walk` (the polyline) and `dispatch.rs::node_of` (the cross-line block key) at S5;
//! kept HERE, in ONE module both import, so the two callers cannot disagree on cell identity (a
//! disagreement would silently disengage the cross-line mutex — a gate-blind head-on collision).
//!
//! **Float discipline.** Hex math needs `√3`, so this module uses `f64` — EXACTLY as `line.rs`'s
//! shipped Catmull-Rom (`smooth_centripetal`) already does for the continuous path. It is confined to
//! the geometry-build step, every result is immediately quantised to `i64` (cell centres to mm,
//! fractional axials to integer `(q, r)` via `cube_round`), and the operations are a fixed sequence
//! ⇒ bit-identical across runs (IEEE-754). The determinism gate (`run()==run()` + the golden pin)
//! and the structural invariants below (`tests/hexgrid.rs`) pin it. No hashed value is read from a
//! float; floats only PRODUCE quantised integers that then enter state.
use crate::geo_local::PointMm;

/// Axial hex coordinate `(q, r)` — integer, the cell identity the cross-line mutex keys on.
pub type Axial = (i64, i64);

/// `√3` as a fixed `f64` literal (not a non-`const` `.sqrt()` call) — a single deterministic constant
/// for both the forward and inverse transforms, so they round-trip consistently.
const SQRT3: f64 = 1.732_050_807_568_877_2;

/// Cube round: snap a fractional cube coord to the nearest integer hex, repairing the coordinate with
/// the largest rounding delta so the cube constraint `x + y + z == 0` is preserved (Red Blob Games).
#[inline]
fn cube_round(fx: f64, fy: f64, fz: f64) -> (i64, i64, i64) {
    let (mut rx, mut ry, mut rz) = (fx.round(), fy.round(), fz.round());
    let (dx, dy, dz) = ((rx - fx).abs(), (ry - fy).abs(), (rz - fz).abs());
    if dx > dy && dx > dz {
        rx = -ry - rz;
    } else if dy > dz {
        ry = -rx - rz;
    } else {
        rz = -rx - ry;
    }
    (rx as i64, ry as i64, rz as i64)
}

/// Pixel (mm) → axial cell — the `node_of` primitive. `size_mm` is the hex centre-to-corner size
/// (= `grid_cell_mm`). The one float-then-`round`-to-`i64` site the build plan flags; pinned by the
/// centre round-trip test (a cell's own centre must map back to that cell, for ALL cells in range).
#[inline]
pub fn axial_of(p: PointMm, size_mm: i64) -> Axial {
    let s = size_mm as f64;
    let (px, py) = (p.x_mm as f64, p.y_mm as f64);
    let fq = (SQRT3 / 3.0 * px - py / 3.0) / s;
    let fr = (2.0 / 3.0 * py) / s;
    // axial → cube (x=q, z=r, y=-x-z), round, return axial (q, r).
    let (rx, _ry, rz) = cube_round(fq, -fq - fr, fr);
    (rx, rz)
}

/// Axial cell → its centre in mm (the polyline vertex). Inverse of `axial_of` on cell centres.
#[inline]
pub fn center_of(a: Axial, size_mm: i64) -> PointMm {
    let s = size_mm as f64;
    let (q, r) = (a.0 as f64, a.1 as f64);
    let x = s * (SQRT3 * q + SQRT3 / 2.0 * r);
    let y = s * (1.5 * r);
    PointMm::new(x.round() as i64, y.round() as i64)
}

/// Hex (cube) distance between two axial cells — the number of steps on the lattice.
#[inline]
pub fn distance(a: Axial, b: Axial) -> i64 {
    let (ax, az) = a;
    let (bx, bz) = b;
    let (ay, by) = (-ax - az, -bx - bz);
    ((ax - bx).abs() + (ay - by).abs() + (az - bz).abs()) / 2
}

/// The six pointy-top axial neighbour directions in rotational order, so consecutive entries are
/// ADJACENT — each adjacent pair brackets one 60° sector of displacement (the decomposition basis).
const DIRS: [Axial; 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

/// The cheapest one-BEND minimal-length hex line between two cells, INCLUSIVE, scored by `cost` over its
/// INTERIOR cells. A shortest hex path decomposes into a run along one of two bracketing directions then
/// the other (≤ 1 bend); the two run-ORDERS are the two "corners" of the same-length rhombus. We pick the
/// cheaper corner, so committed track swings to the side with kinder terrain (water/mountains are dear).
/// SAME LENGTH as a cube-lerp line — only the in-between cells and the turn count differ (far fewer turns).
///
/// CANONICAL + SYMMETRIC (load-bearing for the cross-line mutex): computed from the lexicographically
/// SMALLER endpoint and reversed for the larger, and the cost comparison is endpoint-order-independent,
/// so `line_costed(a,b)` is the EXACT reverse of `line_costed(b,a)` — opposing trains reserve the SAME
/// edges. Cost ties break to the first corner (deterministic). Adjacent axial dirs are unimodular, so the
/// `(k,m)` decomposition is exact integers — NO float in the cell sequence (unlike the old lerp).
pub fn line_costed(a: Axial, b: Axial, cost: &dyn Fn(Axial) -> i64) -> Vec<Axial> {
    let (lo, hi, rev) = if a <= b { (a, b, false) } else { (b, a, true) };
    let (dq, dr) = (hi.0 - lo.0, hi.1 - lo.1);
    if dq == 0 && dr == 0 {
        return vec![lo];
    }
    // Decompose hi-lo into k·d1 + m·d2 over the ONE bracketing adjacent direction pair (k,m ≥ 0).
    let (mut k, mut m, mut d1, mut d2) = (0i64, 0i64, DIRS[0], DIRS[1]);
    for i in 0..6 {
        let (e1, e2) = (DIRS[i], DIRS[(i + 1) % 6]);
        let det = e1.0 * e2.1 - e1.1 * e2.0; // ±1 for adjacent dirs (unimodular) ⇒ exact integer (k,m)
        if det == 0 {
            continue;
        }
        let (kk, mm) = ((dq * e2.1 - dr * e2.0) / det, (e1.0 * dr - e1.1 * dq) / det);
        if kk >= 0 && mm >= 0 {
            k = kk;
            m = mm;
            d1 = e1;
            d2 = e2;
            break;
        }
    }
    let build = |first: Axial, nf: i64, second: Axial, ns: i64| -> Vec<Axial> {
        let mut v = Vec::with_capacity((nf + ns + 1) as usize);
        let mut cur = lo;
        v.push(cur);
        for _ in 0..nf {
            cur = (cur.0 + first.0, cur.1 + first.1);
            v.push(cur);
        }
        for _ in 0..ns {
            cur = (cur.0 + second.0, cur.1 + second.1);
            v.push(cur);
        }
        v
    };
    let score = |v: &[Axial]| -> i64 {
        if v.len() <= 2 {
            0
        } else {
            v[1..v.len() - 1].iter().map(|&c| cost(c)).sum()
        }
    };
    let c1 = build(d1, k, d2, m); // d1-run then d2-run
    let mut v = if k > 0 && m > 0 {
        let c2 = build(d2, m, d1, k); // the other corner: d2-run then d1-run
        if score(&c2) < score(&c1) {
            c2
        } else {
            c1
        }
    } else {
        c1 // axis-aligned ⇒ a single straight run, no corner to choose
    };
    if rev {
        v.reverse();
    }
    v
}

/// The canonical hex line with NO terrain preference (flat cost ⇒ the first corner) — the structural
/// default used by the hexgrid tests and any caller without a terrain field.
pub fn line(a: Axial, b: Axial) -> Vec<Axial> {
    line_costed(a, b, &|_| 0)
}
