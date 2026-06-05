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
/// Comfortable lateral acceleration (mm/s^2) -> curve speed cap = sqrt(a * radius).
const LAT_ACCEL_MM_S2: f64 = 800.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub color: u32,
    pub name: String,
    /// Transport mode: 0=rail, 1=bus, 2=ferry, 3=air (trainset::tmode). Picks the vehicle
    /// preset + the placement-gate rules.
    pub mode: u8,
    /// Circular line: the path closes (last stop -> first) and trains loop forward instead
    /// of reversing at an end.
    pub loop_line: bool,
    pub stops: Vec<StationId>,
    /// Freeform control points that BEND the track between stops. `waypoints[i]` shapes the span
    /// after stop `i` (between stop i and i+1; for a loop the last entry is the closing span back
    /// to stop 0). These are pure geometry (mm) — pass-through shaping points, NOT halts — so the
    /// curve threads stop0, wp[0]…, stop1, wp[1]…, and tight bends just slow trains via the
    /// existing per-vertex speed cap. Empty = straight-through (the original behaviour).
    #[serde(default)]
    pub waypoints: Vec<Vec<PointMm>>,
    pub headway_ms: i64,
    pub trainset: Option<TrainsetAssignment>,
    /// Dense smoothed curve vertices (mm). Recomputed from stop positions on change.
    pub polyline: Vec<PointMm>,
    /// Cumulative arc-length (mm) at each polyline vertex; `arclen_mm[0] == 0`.
    pub arclen_mm: Vec<i64>,
    /// Arc-length (mm) at each STOP along the smoothed polyline (stations to halt at).
    pub stop_arclen_mm: Vec<i64>,
    /// Curve speed cap (mm/s) at each polyline vertex (i64::MAX where straight).
    pub speed_cap_mm_s: Vec<i64>,
    /// Tightest curve radius on the line (mm); i64::MAX if effectively straight.
    pub min_radius_mm: i64,
    /// Build mode per inter-stop span: 0=Surface, 1=Elevated, 2=Tunnel.
    pub span_mode: Vec<u8>,
    /// Total surface-rail disruption units (lower is better; 0 when elevated/tunnel/ROW).
    pub disruption_units: i64,
    /// True if any Surface span crosses water (the UI's one hard gate).
    pub crosses_water_surface: bool,
    /// Capital cost to build this line (dollars): track by mode + land-taking + trains.
    pub capital_cost: i64,
    /// Tombstone: a bulldozed line keeps its slot (ids are indices) but is skipped by the
    /// dispatcher, routing, cost/opex sums, and views. Determinism-safe (in state_hash).
    #[serde(default)]
    pub removed: bool,
}

/// Build modes for a track span.
pub mod mode {
    pub const SURFACE: u8 = 0;
    pub const ELEVATED: u8 = 1;
    pub const TUNNEL: u8 = 2;
}

impl Line {
    pub fn new(color: u32, default_headway_ms: i64) -> Self {
        Self {
            color,
            name: String::new(),
            mode: 0,
            loop_line: false,
            stops: Vec::new(),
            waypoints: Vec::new(),
            headway_ms: default_headway_ms,
            trainset: None,
            polyline: Vec::new(),
            arclen_mm: Vec::new(),
            stop_arclen_mm: Vec::new(),
            speed_cap_mm_s: Vec::new(),
            min_radius_mm: i64::MAX,
            span_mode: Vec::new(),
            disruption_units: 0,
            crosses_water_surface: false,
            capital_cost: 0,
            removed: false,
        }
    }

    /// Span index (inter-stop segment) containing forward arc-length `s_mm`.
    pub fn span_of(&self, s_mm: i64) -> usize {
        if self.stop_arclen_mm.len() < 2 {
            return 0;
        }
        for j in 1..self.stop_arclen_mm.len() {
            if s_mm < self.stop_arclen_mm[j] {
                return j - 1;
            }
        }
        self.stop_arclen_mm.len() - 2
    }

    /// Curve speed cap (mm/s) at forward arc-length `s_mm` (the tighter of the bracketing
    /// vertices). i64::MAX where the track is straight.
    pub fn speed_cap_at(&self, s_mm: i64) -> i64 {
        if self.speed_cap_mm_s.len() < 2 {
            return i64::MAX;
        }
        let s = s_mm.clamp(0, self.length_mm());
        for i in 1..self.arclen_mm.len() {
            if s <= self.arclen_mm[i] {
                return self.speed_cap_mm_s[i - 1].min(self.speed_cap_mm_s[i]);
            }
        }
        *self.speed_cap_mm_s.last().unwrap_or(&i64::MAX)
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

    /// Station id for a stop index (wraps for loops, where index == stops.len() is the start).
    pub fn station_for_stop_index(&self, i: usize) -> StationId {
        if self.stops.is_empty() {
            return StationId(0);
        }
        self.stops[i % self.stops.len()]
    }

    /// Rebuild the smoothed polyline + arc-length tables from the ordered stop positions.
    /// For a loop the path is closed (first stop appended) so trains run a full circuit.
    pub fn rebuild_from_points(&mut self, stop_pts: &[PointMm]) {
        let wps = self.waypoints.clone();
        self.rebuild_with_span_points(stop_pts, &wps);
    }

    /// As `rebuild_from_points`, but the per-span shaping points are supplied EXTERNALLY rather
    /// than read from `self.waypoints` — so a bus line can be threaded along an auto-computed road
    /// route while a rail line uses the player's waypoints. `span_points[i]` shapes the span after
    /// stop i (pass-through, not halts).
    pub fn rebuild_with_span_points(&mut self, stop_pts: &[PointMm], span_points: &[Vec<PointMm>]) {
        // Interleave each stop with its span's control points so the curve threads
        // stop0, wp[0]…, stop1, wp[1]…, stop2, …. Only stops are halts (recorded in stop_arclen_mm).
        let n = stop_pts.len();
        let mut pts: Vec<PointMm> = Vec::with_capacity(n);
        let mut is_stop: Vec<bool> = Vec::with_capacity(n);
        for i in 0..n {
            pts.push(stop_pts[i]);
            is_stop.push(true);
            // Shaping points for the span AFTER stop i (skip on the open end of a non-loop line).
            if i + 1 < n || self.loop_line {
                if let Some(span_wps) = span_points.get(i) {
                    for &wp in span_wps {
                        pts.push(wp);
                        is_stop.push(false);
                    }
                }
            }
        }
        if self.loop_line && n >= 3 {
            pts.push(stop_pts[0]); // close the loop back to the first stop
            is_stop.push(true);
        }
        let (poly, stop_idx) = smooth_centripetal(&pts);
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
        // Arc-length at each STOP vertex only (waypoints are pass-through, not halts).
        self.stop_arclen_mm = stop_idx
            .iter()
            .zip(&is_stop)
            .filter(|(_, &stop)| stop)
            .map(|(&i, _)| self.arclen_mm.get(i).copied().unwrap_or(0))
            .collect();

        // Per-vertex curve speed cap (from local circumradius) + tightest radius.
        let n = self.polyline.len();
        self.speed_cap_mm_s = vec![i64::MAX; n];
        let mut minr = f64::INFINITY;
        for i in 1..n.saturating_sub(1) {
            let a = (self.polyline[i - 1].x_mm as f64, self.polyline[i - 1].y_mm as f64);
            let b = (self.polyline[i].x_mm as f64, self.polyline[i].y_mm as f64);
            let c = (self.polyline[i + 1].x_mm as f64, self.polyline[i + 1].y_mm as f64);
            let r = circumradius(a, b, c);
            self.speed_cap_mm_s[i] = cap_from_radius(r);
            if r < minr {
                minr = r;
            }
        }
        self.min_radius_mm = if minr.is_finite() { minr as i64 } else { i64::MAX };
    }
}

/// Circumradius (mm) of the triangle through three points; +inf when ~collinear (straight).
fn circumradius(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    let d = |p: (f64, f64), q: (f64, f64)| ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
    let ab = d(a, b);
    let bc = d(b, c);
    let ca = d(c, a);
    let area2 = ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs(); // 2 * area
    if area2 < 1.0 {
        return f64::INFINITY;
    }
    ab * bc * ca / (2.0 * area2)
}

/// Curve speed cap (mm/s) for a radius (mm): v = sqrt(lateral_accel * radius).
fn cap_from_radius(r_mm: f64) -> i64 {
    if !r_mm.is_finite() {
        return i64::MAX;
    }
    (LAT_ACCEL_MM_S2 * r_mm).sqrt() as i64
}

/// Centripetal Catmull-Rom through `pts`. Returns the dense polyline and, for each input
/// stop, its index in that polyline (so stations remain exact vertices on the curve).
fn smooth_centripetal(pts: &[PointMm]) -> (Vec<PointMm>, Vec<usize>) {
    let n = pts.len();
    if n < 2 {
        return (pts.to_vec(), (0..n).collect());
    }
    // n == 2 still runs the loop below (clamped neighbours => ~linear) so straight spans get
    // intermediate vertices too — needed for per-segment buildability sampling.
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
