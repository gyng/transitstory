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

/// The CANONICAL hex line between two cells, INCLUSIVE — the hex analog of `line.rs::grid_walk`'s
/// octilinear walk. Drawn from the lexicographically SMALLER endpoint, then reversed, so `line(a,b)`
/// is the EXACT reverse of `line(b,a)` (the same unordered edge set). This is load-bearing for the
/// cross-line mutex: an out-and-back train and an opposing train on another line cross the shared
/// section in opposite directions and must reserve the SAME edges. A fixed epsilon nudge keeps every
/// interpolated point off a hex boundary, so consecutive cells are always adjacent (no skips/dupes).
pub fn line(a: Axial, b: Axial) -> Vec<Axial> {
    let (lo, hi, rev) = if a <= b { (a, b, false) } else { (b, a, true) };
    let n = distance(lo, hi);
    if n == 0 {
        return vec![lo];
    }
    let (ax, az) = lo;
    let (bx, bz) = hi;
    let (acx, acy, acz) = (ax as f64, (-ax - az) as f64, az as f64);
    let (bcx, bcy, bcz) = (bx as f64, (-bx - bz) as f64, bz as f64);
    let mut v: Vec<Axial> = Vec::with_capacity((n + 1) as usize);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        // Lerp in cube space + a fixed nudge (sum stays ~0) so no sample lands on a boundary.
        let lx = acx + (bcx - acx) * t + 1e-6;
        let ly = acy + (bcy - acy) * t + 1e-6;
        let lz = acz + (bcz - acz) * t - 2e-6;
        let (rx, _ry, rz) = cube_round(lx, ly, lz);
        v.push((rx, rz));
    }
    if rev {
        v.reverse();
    }
    v
}
