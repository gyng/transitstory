//! A transit line: ordered station stops (out-and-back), color, headway, trainset. The
//! drawn path is a CURVED polyline (centripetal Catmull-Rom through the stops) so track
//! follows smooth curves rather than straight dog-legs (a soft minimum-radius: centripetal
//! parameterization avoids cusps/loops). The dense polyline drives both rendering and the
//! sim's arc-length motion; `stop_arclen_mm` marks where the actual stations sit on it.
use crate::geo_local::PointMm;
use crate::ids::StationId;
use crate::trainset::TrainsetAssignment;
use serde::{Deserialize, Serialize};

/// Curve samples per inter-stop span (more = smoother + denser polyline).
const SAMPLES_PER_SPAN: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub color: u32,
    pub stops: Vec<StationId>,
    pub headway_ms: i64,
    pub trainset: Option<TrainsetAssignment>,
    /// Dense smoothed curve vertices (mm). Recomputed from stop positions on change.
    pub polyline: Vec<PointMm>,
    /// Cumulative arc-length (mm) at each polyline vertex; `arclen_mm[0] == 0`.
    pub arclen_mm: Vec<i64>,
    /// Arc-length (mm) at each STOP along the smoothed polyline (stations to halt at).
    pub stop_arclen_mm: Vec<i64>,
}

impl Line {
    pub fn new(color: u32, default_headway_ms: i64) -> Self {
        Self {
            color,
            stops: Vec::new(),
            headway_ms: default_headway_ms,
            trainset: None,
            polyline: Vec::new(),
            arclen_mm: Vec::new(),
            stop_arclen_mm: Vec::new(),
        }
    }

    /// One-way length of the smoothed path in mm.
    pub fn length_mm(&self) -> i64 {
        self.arclen_mm.last().copied().unwrap_or(0)
    }

    /// Cartesian point (mm) at forward arc-length `s_mm` along the smoothed polyline.
    pub fn point_at(&self, s_mm: i64) -> (i64, i64) {
        match self.polyline.len() {
            0 => return (0, 0),
            1 => return (self.polyline[0].x_mm, self.polyline[0].y_mm),
            _ => {}
        }
        let s = s_mm.clamp(0, self.length_mm());
        for i in 1..self.arclen_mm.len() {
            if s <= self.arclen_mm[i] {
                let seg_start = self.arclen_mm[i - 1];
                let seg_len = self.arclen_mm[i] - seg_start;
                let a = self.polyline[i - 1];
                let b = self.polyline[i];
                if seg_len <= 0 {
                    return (a.x_mm, a.y_mm);
                }
                let t = s - seg_start;
                return (
                    a.x_mm + (b.x_mm - a.x_mm) * t / seg_len,
                    a.y_mm + (b.y_mm - a.y_mm) * t / seg_len,
                );
            }
        }
        let last = self.polyline[self.polyline.len() - 1];
        (last.x_mm, last.y_mm)
    }

    /// Heading (radians, render-only float) of the segment at forward arc-length `s_mm`.
    pub fn heading_at(&self, s_mm: i64) -> f32 {
        if self.polyline.len() < 2 {
            return 0.0;
        }
        let s = s_mm.clamp(0, self.length_mm());
        for i in 1..self.arclen_mm.len() {
            if s <= self.arclen_mm[i] {
                let a = self.polyline[i - 1];
                let b = self.polyline[i];
                return ((b.y_mm - a.y_mm) as f32).atan2((b.x_mm - a.x_mm) as f32);
            }
        }
        0.0
    }

    /// Rebuild the smoothed polyline + arc-length tables from the ordered stop positions.
    pub fn rebuild_from_points(&mut self, stop_pts: &[PointMm]) {
        let (poly, stop_idx) = smooth_centripetal(stop_pts);
        self.polyline = poly;
        // Cumulative arc-length along the dense polyline.
        self.arclen_mm.clear();
        let mut acc = 0i64;
        for i in 0..self.polyline.len() {
            if i == 0 {
                self.arclen_mm.push(0);
            } else {
                acc += self.polyline[i - 1].dist_mm(&self.polyline[i]);
                self.arclen_mm.push(acc);
            }
        }
        // Arc-length at each stop vertex.
        self.stop_arclen_mm = stop_idx
            .iter()
            .map(|&i| self.arclen_mm.get(i).copied().unwrap_or(0))
            .collect();
    }
}

/// Centripetal Catmull-Rom through `pts`. Returns the dense polyline and, for each input
/// stop, its index in that polyline (so stations remain exact vertices on the curve).
fn smooth_centripetal(pts: &[PointMm]) -> (Vec<PointMm>, Vec<usize>) {
    let n = pts.len();
    if n < 3 {
        // 0–2 stops: nothing to curve.
        return (pts.to_vec(), (0..n).collect());
    }
    let p: Vec<(f64, f64)> = pts.iter().map(|q| (q.x_mm as f64, q.y_mm as f64)).collect();
    let mut out: Vec<PointMm> = Vec::with_capacity(n * SAMPLES_PER_SPAN + 1);
    let mut stop_idx: Vec<usize> = Vec::with_capacity(n);

    let at = |i: isize| -> (f64, f64) {
        let c = i.clamp(0, (n - 1) as isize) as usize;
        p[c]
    };

    for i in 0..n - 1 {
        let p0 = at(i as isize - 1);
        let p1 = at(i as isize);
        let p2 = at(i as isize + 1);
        let p3 = at(i as isize + 2);
        stop_idx.push(out.len()); // span starts exactly at stop i
        for k in 0..SAMPLES_PER_SPAN {
            let t = k as f64 / SAMPLES_PER_SPAN as f64;
            let (x, y) = catmull(p0, p1, p2, p3, t);
            out.push(PointMm::new(x.round() as i64, y.round() as i64));
        }
    }
    stop_idx.push(out.len());
    out.push(pts[n - 1]); // final stop exactly

    (out, stop_idx)
}

/// Centripetal Catmull-Rom (alpha = 0.5) interpolation at local parameter `t` in [0,1]
/// between p1 and p2, using neighbors p0,p3. Falls back to linear on degenerate spacing.
fn catmull(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let d = |a: (f64, f64), b: (f64, f64)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().sqrt();
    let t0 = 0.0;
    let t1 = t0 + d(p0, p1).max(1e-6);
    let t2 = t1 + d(p1, p2).max(1e-6);
    let t3 = t2 + d(p2, p3).max(1e-6);
    let tt = t1 + t * (t2 - t1);
    let lerp = |a: (f64, f64), b: (f64, f64), u: f64| (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u);
    let a1 = lerp(p0, p1, (tt - t0) / (t1 - t0));
    let a2 = lerp(p1, p2, (tt - t1) / (t2 - t1));
    let a3 = lerp(p2, p3, (tt - t2) / (t3 - t2));
    let b1 = lerp(a1, a2, (tt - t0) / (t2 - t0));
    let b2 = lerp(a2, a3, (tt - t1) / (t3 - t1));
    lerp(b1, b2, (tt - t1) / (t2 - t1))
}
