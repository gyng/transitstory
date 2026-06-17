//! A transit line: an ordered trunk of station stops (out-and-back or loop), color, headway,
//! trainset — plus an optional tree of BRANCHES off the trunk (P3, docs/capacity-roadmap.md). The
//! engine materialises one **service `Path`** per route through the tree (the trunk, and each
//! trunk-prefix continued onto a branch); every `Path` is a linear smoothed polyline exactly like
//! the old single-polyline line, so vehicle motion / routing / rendering run per-path unchanged.
//! A non-branched line has exactly one path (`paths[0]`, the trunk), and behaves as before.
use crate::geo_local::PointMm;
use crate::ids::StationId;
use crate::trainset::TrainsetAssignment;
use serde::{Deserialize, Serialize};

/// Curve samples per inter-stop span (more = smoother + denser polyline).
const SAMPLES_PER_SPAN: usize = 10;
/// Samples per segment for LITERAL (imported real-geometry) lines — a light pass that just rounds
/// the raw OSM corners (the vertices are already dense, so 2 keeps the alignment + caps the size).
const LITERAL_SAMPLES: usize = 2;
/// Comfortable lateral acceleration (mm/s²) -> curve speed cap = sqrt(a * radius). CLOCK-FRAME:
/// ×CLOCK_SCALE² (like the trainset accels), so the cap over a given REAL radius scales with the
/// ×CLOCK_SCALE speeds and curves bind exactly as tightly as before the unification.
const LAT_ACCEL_MM_S2: f64 = 720_000.0;

/// Build modes for a track span.
pub mod mode {
    pub const SURFACE: u8 = 0;
    pub const ELEVATED: u8 = 1;
    pub const TUNNEL: u8 = 2;
}

/// Track type per span (P2, docs/capacity-roadmap.md). DOUBLE is the default everywhere so P1 replay
/// stays byte-identical until a `SetSegmentTrack` lands. On a SINGLE span, opposing-direction trains
/// cannot both be inside — they MEET at the bounding stations (passing places); single track is
/// cheaper to build but lower capacity.
pub mod track {
    pub const DOUBLE: u8 = 0;
    pub const SINGLE: u8 = 1;
}

/// One materialised service path of a line: a root-to-leaf stop sequence (the trunk, or a trunk
/// prefix continued onto a branch) with its own smoothed geometry. All vehicle motion / routing /
/// rendering runs on a `Path`, so a path behaves exactly like the old single-polyline line.
///
/// `Serialize` is HAND-WRITTEN (TTD L3 C1, see the impl) so the determinism hash reflects the
/// geometry-ownership flip: a path BOUND to track segments (grid) omits its derived `polyline`/`arclen`/
/// track tables from the hash — they live authoritatively in the hashed segment slab — while an UNBOUND
/// (continuous / non-grid) path still hashes its own geometry exactly as before. `Deserialize` stays
/// derived (it is never used to reconstruct hashed state — saves are command logs — but `Line`'s derive
/// needs it to compile).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Path {
    /// Ordered stops along this path.
    pub stops: Vec<StationId>,
    /// Circular path: closes (last -> first) and runs forward instead of reversing at an end.
    pub loop_line: bool,
    /// Dense smoothed curve vertices (mm). Recomputed from stop positions on change.
    pub polyline: Vec<PointMm>,
    /// Cumulative arc-length (mm) at each polyline vertex; `arclen_mm[0] == 0`.
    pub arclen_mm: Vec<i64>,
    /// Arc-length (mm) at each STOP along the smoothed polyline (stations to halt at).
    pub stop_arclen_mm: Vec<i64>,
    /// Curve speed cap (mm/s) at each polyline vertex (i64::MAX where straight).
    pub speed_cap_mm_s: Vec<i64>,
    /// Tightest curve radius on the path (mm); i64::MAX if effectively straight.
    pub min_radius_mm: i64,
    /// Build mode per inter-stop span: 0=Surface, 1=Elevated, 2=Tunnel.
    pub span_mode: Vec<u8>,
    /// Track type per inter-stop span: 0=Double (default), 1=Single (P2). Parallel to `span_mode`.
    /// Default-empty deserializes as all-Double; sized/zero-filled (Double) in `rebuild`.
    #[serde(default)]
    pub track_type: Vec<u8>,
    /// Literal geometry: connect the stop + waypoint vertices DIRECTLY (no Catmull-Rom smoothing /
    /// subdivision). Set for real-world imported lines so they follow the actual OSM track alignment
    /// rather than an idealised synthesised curve — and so dense imported geometry doesn't 10×-bloat
    /// the polyline. Player-drawn lines stay smoothed (literal = false).
    #[serde(default)]
    pub literal: bool,
    /// TTD L3 C1 (HASHED via the hand-written `Serialize`): the ordered list of `TrackGraph` segments this
    /// path covers, each with a `bool` = traversed in REVERSE (cells[last] → cells[0]). Bound in the
    /// apply/dispatch write-path by `dispatch::bind_path_segments` right after `derive_track_graph` (a
    /// segment boundary is a junction whose degree depends on OTHER lines, so a path can't decompose itself
    /// without the whole graph). For a BOUND (grid) path this binding IS the path's geometry in the hash —
    /// the polyline/arclen/track tables are reconstructed from the (hashed) segment slab. EMPTY for
    /// continuous / non-grid networks (no graph), where the path hashes its own self-authored geometry.
    /// (`Deserialize`-skipped: never deserialized for hashed state — rebuilt from the command log.)
    #[serde(skip_deserializing)]
    pub segments: Vec<(crate::ids::TrackSegmentId, bool)>,
}

// TTD L3 C1 — hand-written canonical `Serialize` for the determinism hash (the geometry-ownership flip).
// Field order matches the struct declaration so an UNBOUND path is byte-identical to the old derive PLUS the
// (now-hashed) `segments` binding appended last — i.e. the continuous transit golden's re-pin is a clean
// empty-slice shift. A BOUND (grid) path serializes ONLY `stops`/`loop_line`/`literal`/`segments`: its
// geometry (polyline/arclen/track_type/span_mode/min_radius/speed_cap) is OMITTED here because it lives
// authoritatively in the hashed segment slab (`Canonical.track_segments`) — so geometry genuinely LEAVES
// `Path` in the hash. The unbound branch reproduces the EXACT prior field set (all 10 derived fields) so the
// only transit-hash delta is the appended (empty) `segments` binding — a clean empty-slice re-pin.
impl serde::Serialize for Path {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        if self.segments.is_empty() {
            // UNBOUND (continuous / non-grid / nothing to bind): hash geometry exactly as the old derive,
            // then the `segments` binding (empty) appended last. (postcard ignores struct name/len/field
            // names — only the field byte sequence matters.)
            let mut st = s.serialize_struct("Path", 11)?;
            st.serialize_field("stops", &self.stops)?;
            st.serialize_field("loop_line", &self.loop_line)?;
            st.serialize_field("polyline", &self.polyline)?;
            st.serialize_field("arclen_mm", &self.arclen_mm)?;
            st.serialize_field("stop_arclen_mm", &self.stop_arclen_mm)?;
            st.serialize_field("speed_cap_mm_s", &self.speed_cap_mm_s)?;
            st.serialize_field("min_radius_mm", &self.min_radius_mm)?;
            st.serialize_field("span_mode", &self.span_mode)?;
            st.serialize_field("track_type", &self.track_type)?;
            st.serialize_field("literal", &self.literal)?;
            st.serialize_field("segments", &self.segments)?;
            st.end()
        } else {
            // BOUND (grid): geometry lives in the segment slab — hash only the service identity + binding.
            let mut st = s.serialize_struct("Path", 4)?;
            st.serialize_field("stops", &self.stops)?;
            st.serialize_field("loop_line", &self.loop_line)?;
            st.serialize_field("literal", &self.literal)?;
            st.serialize_field("segments", &self.segments)?;
            st.end()
        }
    }
}

impl Path {
    pub fn new(stops: Vec<StationId>, loop_line: bool) -> Self {
        Self { stops, loop_line, min_radius_mm: i64::MAX, ..Default::default() }
    }

    /// The span `s_mm` is STRICTLY INSIDE (between its two bounding stops, on neither gate), or
    /// `None` when on a station gate (a passing place owning no single-span reservation). P2's
    /// single-track meet protocol keys off this; shared so the "inside vs at a gate" test can't drift.
    pub fn strictly_inside(&self, s_mm: i64) -> Option<usize> {
        let sp = self.span_of(s_mm);
        let lo = self.stop_arclen_mm.get(sp).copied().unwrap_or(i64::MIN);
        let hi = self.stop_arclen_mm.get(sp + 1).copied().unwrap_or(i64::MAX);
        if s_mm > lo && s_mm < hi {
            Some(sp)
        } else {
            None
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

    /// Number of inter-stop spans (the closing span included for a loop).
    pub fn nspans(&self) -> usize {
        if self.loop_line {
            self.stops.len()
        } else {
            self.stops.len().saturating_sub(1)
        }
    }

    /// Rebuild the smoothed polyline + arc-length tables from this path's ordered stop positions.
    /// `span_points[i]` shapes the span after stop i (pass-through bends, not halts). Existing
    /// `span_mode` values are preserved where the span count is unchanged; new spans default Surface.
    pub fn rebuild(&mut self, stop_pts: &[PointMm], span_points: &[Vec<PointMm>], grid_cell_mm: i64, cost: &dyn Fn(crate::hexgrid::Axial) -> i64) {
        let n = stop_pts.len();
        // A stop's geometric vertex. For LITERAL (imported) lines this is the stop's ON-TRACK
        // position — the adjacent real waypoint — NOT the supplied point: same-name interchanges are
        // merged to ONE point (a single station id, for transfers), but each line runs through its
        // OWN platform, so forcing the merged point into the polyline spikes the track to whichever
        // line's platform was imported first. Anchoring to the real track removes that snap; the
        // station id stays shared. Smoothed (player-drawn) lines just use the supplied stop point.
        let stop_vertex = |i: usize| -> PointMm {
            if self.literal {
                span_points
                    .get(i)
                    .and_then(|w| w.first())
                    .copied()
                    .or_else(|| i.checked_sub(1).and_then(|p| span_points.get(p)).and_then(|w| w.last()).copied())
                    .unwrap_or(stop_pts[i])
            } else {
                stop_pts[i]
            }
        };
        let mut pts: Vec<PointMm> = Vec::with_capacity(n);
        let mut is_stop: Vec<bool> = Vec::with_capacity(n);
        for i in 0..n {
            pts.push(stop_vertex(i));
            is_stop.push(true);
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
            pts.push(stop_vertex(0)); // close the loop back to the first stop's on-track vertex
            is_stop.push(true);
        }
        // Literal (imported) geometry follows the real OSM vertices with only a VERY MINOR
        // centripetal pass (few samples) — just enough to round the raw corners, not the big
        // sweeping curve full smoothing would invent (and without the 10× polyline bloat). Player
        // geometry gets the full smooth. Curve speed caps come from the actual vertices either way,
        // so a real-world line's tight curves still slow trains correctly.
        // GRID mode (grid_cell_mm > 0): a crisp octilinear lattice walk (integer-exact, byte-identical
        // for two lines over the same cells — the cross-line edge_key foundation). Otherwise the
        // continuous Catmull-Rom curve (literal = a light pass; player = full smooth). Parity: a city
        // with grid_cell_mm == 0 takes the EXACT existing path ⇒ byte-identical geometry, zero re-pins.
        let (poly, stop_idx) = if grid_cell_mm > 0 {
            grid_walk(&pts, grid_cell_mm, cost)
        } else {
            let samples = if self.literal { LITERAL_SAMPLES } else { SAMPLES_PER_SPAN };
            smooth_centripetal(&pts, samples)
        };
        self.polyline = poly;
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
        self.stop_arclen_mm = stop_idx
            .iter()
            .zip(&is_stop)
            .filter(|(_, &stop)| stop)
            .map(|(&i, _)| self.arclen_mm.get(i).copied().unwrap_or(0))
            .collect();

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
        // Preserve existing per-span build modes; resize to the new span count (new spans Surface).
        let nspans = self.nspans();
        if self.span_mode.len() != nspans {
            self.span_mode.resize(nspans, mode::SURFACE);
        }
        // Same for track type (new spans default Double).
        if self.track_type.len() != nspans {
            self.track_type.resize(nspans, track::DOUBLE);
        }
    }

    /// TTD L3 C1: recompute the arc-length + curve tables from the CURRENT `self.polyline` (which the
    /// segment-concatenation just authored). `stop_arclen_mm` is unchanged — the concatenated polyline is
    /// byte-identical to the grid-walk one, so each stop sits at the same arc-length. Re-accumulates
    /// `arclen_mm` and re-derives `speed_cap_mm_s`/`min_radius_mm` over the polyline exactly as `rebuild`
    /// does (the circumradius float pattern is the pre-existing accepted one), so the runtime geometry the
    /// integrator reads is identical whether built by `rebuild` or derived from the segments.
    pub fn recompute_tables_from_polyline(&mut self) {
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

/// A branch off the trunk: it leaves the trunk at trunk stop `diverge_at` and continues through
/// `stops`. Multiple branches may share a `diverge_at` (a 3-way junction). A tree, never a cycle.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Branch {
    pub diverge_at: u16,
    pub stops: Vec<StationId>,
    /// Per-span shaping points for the branch's OWN spans (junction→stop0, stop0→stop1, …) — the
    /// real OSM alignment of the spur, for a literal imported line. The shared trunk-prefix reuses
    /// the trunk's waypoints, so the spur matches the trunk exactly up to the divergence. Empty =
    /// straight spur.
    #[serde(default)]
    pub waypoints: Vec<Vec<PointMm>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub color: u32,
    pub name: String,
    /// Transport mode: 0=rail, 1=bus, 2=ferry, 3=air (trainset::tmode). Picks the vehicle
    /// preset + the placement-gate rules.
    pub mode: u8,
    /// Circular line: the trunk path closes (last stop -> first) and trains loop forward instead
    /// of reversing at an end.
    pub loop_line: bool,
    /// The trunk: the primary ordered stop sequence (`paths[0]`).
    pub stops: Vec<StationId>,
    /// Literal geometry (real-world imports follow the OSM alignment via dense waypoints, not a
    /// synthesised curve). Propagated to every path on rebuild. See `Path::literal`.
    #[serde(default)]
    pub literal: bool,
    /// Branches off the trunk (P3). Empty for a simple linear/loop line.
    #[serde(default)]
    pub branches: Vec<Branch>,
    /// Freeform control points that BEND the trunk track between stops. `waypoints[i]` shapes the
    /// span after trunk stop `i`. Pure geometry (mm) — pass-through, NOT halts. Empty = straight.
    #[serde(default)]
    pub waypoints: Vec<Vec<PointMm>>,
    pub headway_ms: i64,
    pub trainset: Option<TrainsetAssignment>,
    /// Materialised service paths: `paths[0]` is the trunk; `paths[1..]` is one per branch (the
    /// trunk prefix up to the divergence, continued onto the branch's stops). Derived on rebuild.
    #[serde(default)]
    pub paths: Vec<Path>,
    /// Total surface-rail disruption units across ALL paths (lower is better; 0 when elevated/ROW).
    pub disruption_units: i64,
    /// True if any Surface span on ANY path crosses water (the UI's one hard gate).
    pub crosses_water_surface: bool,
    /// Capital cost to build this line (dollars): track by mode + land-taking + trains.
    pub capital_cost: i64,
    /// Tombstone: a bulldozed line keeps its slot (ids are indices) but is skipped by the
    /// dispatcher, routing, cost/opex sums, and views. Determinism-safe (in state_hash).
    #[serde(default)]
    pub removed: bool,
}

impl Line {
    /// The concrete vehicle spec for this line: its assigned aircraft/trainset within the mode
    /// roster (`trainset.spec`), or the mode default when unassigned. Spec id 0 is always the mode
    /// default, so an unassigned or default-assigned line behaves exactly as before (determinism).
    #[inline]
    pub fn vehicle_spec(&self) -> crate::trainset::TrainsetSpec {
        crate::trainset::spec_for(self.mode, self.trainset.map(|t| t.spec).unwrap_or(0))
    }

    pub fn new(color: u32, default_headway_ms: i64) -> Self {
        Self {
            color,
            name: String::new(),
            mode: 0,
            loop_line: false,
            stops: Vec::new(),
            literal: false,
            branches: Vec::new(),
            waypoints: Vec::new(),
            headway_ms: default_headway_ms,
            trainset: None,
            paths: vec![Path::new(Vec::new(), false)],
            disruption_units: 0,
            crosses_water_surface: false,
            capital_cost: 0,
            removed: false,
        }
    }

    /// The trunk path (`paths[0]`) — what line-level queries (and every non-branched line) mean.
    #[inline]
    pub fn trunk(&self) -> &Path {
        &self.paths[0]
    }

    /// The root-to-leaf stop sequence for each service path: the trunk, then each branch as the
    /// trunk prefix `[0..=diverge_at]` continued onto the branch's stops. Used by geometry rebuild,
    /// dispatch (trains run these round-robin), and routing. Each is linear; branch paths are
    /// out-and-back even on a loop trunk.
    pub fn path_specs(&self) -> Vec<(Vec<StationId>, bool)> {
        let mut out = vec![(self.stops.clone(), self.loop_line)];
        for b in &self.branches {
            let d = (b.diverge_at as usize).min(self.stops.len().saturating_sub(1));
            let mut s: Vec<StationId> = self.stops[..=d].to_vec();
            s.extend_from_slice(&b.stops);
            out.push((s, false));
        }
        out
    }

    // --- trunk-delegating geometry accessors (line-level callers + every non-branched path) ---
    #[inline]
    pub fn length_mm(&self) -> i64 {
        self.paths.first().map(|p| p.length_mm()).unwrap_or(0)
    }
    #[inline]
    pub fn point_at(&self, s_mm: i64) -> (i64, i64) {
        self.paths.first().map(|p| p.point_at(s_mm)).unwrap_or((0, 0))
    }
    #[inline]
    pub fn heading_at(&self, s_mm: i64) -> f32 {
        self.paths.first().map(|p| p.heading_at(s_mm)).unwrap_or(0.0)
    }
    #[inline]
    pub fn speed_cap_at(&self, s_mm: i64) -> i64 {
        self.paths.first().map(|p| p.speed_cap_at(s_mm)).unwrap_or(i64::MAX)
    }
    #[inline]
    pub fn span_of(&self, s_mm: i64) -> usize {
        self.paths.first().map(|p| p.span_of(s_mm)).unwrap_or(0)
    }
    #[inline]
    pub fn station_for_stop_index(&self, i: usize) -> StationId {
        self.paths.first().map(|p| p.station_for_stop_index(i)).unwrap_or(StationId(0))
    }
    #[inline]
    pub fn min_radius_mm(&self) -> i64 {
        self.paths.first().map(|p| p.min_radius_mm).unwrap_or(i64::MAX)
    }

    /// Rebuild ONLY the trunk path geometry from explicit stop points (no branches) — the cost
    /// preview path. Uses the line's waypoints for shaping; leaves branches untouched.
    pub fn rebuild_from_points(&mut self, stop_pts: &[PointMm], grid_cell_mm: i64) {
        let wps = self.waypoints.clone();
        let mut p = Path::new(self.stops.clone(), self.loop_line);
        p.literal = self.literal;
        p.rebuild(stop_pts, &wps, grid_cell_mm, &|_| 0); // helper: flat cost (no terrain context here)
        self.paths = vec![p];
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

/// Crisp GRID geometry (fantasy-fork.md §10): snap each input point to its `cell_mm` lattice cell and
/// connect consecutive points by a dense OCTILINEAR walk — a vertex at every cell CENTRE along the way,
/// stepping diagonally where both axes differ then straight. No Catmull-Rom, no float in vertex math.
/// Returns the polyline + each input point's vertex index.
///
/// SHARING GUARANTEE (the cross-line `edge_key` foundation, shared-rail.md): the walk between a cell
/// PAIR is a deterministic pure-integer function of just that (canonical, lexicographically-ordered)
/// pair, so two lines whose stops snap to the **same consecutive stop-cells** emit byte-identical
/// vertices ⇒ identical edges, and `a→b` is the exact reverse of `b→a`. This is the realistic
/// shared-TRUNK pattern (two lines sharing a central section with shared STATIONS). **It does NOT
/// cover a corridor shared BETWEEN stops** — an express `A→B` and a local `A→M→B` over the same rail
/// split the walk at `M` and emit different cells unless `M` lies exactly on the canonical `A→B` walk
/// (the express/local false-negative the grid review found). That case needs explicit laid track
/// lines reference (the FULL track-objects model), and is out of LITE scope — Phase 2's cross-line
/// mutex contract is "shared consecutive stop-cells", and `grid_express_local_*` (#[ignore]d) pins it.
fn grid_walk(pts: &[PointMm], cell_mm: i64, cost: &dyn Fn(crate::hexgrid::Axial) -> i64) -> (Vec<PointMm>, Vec<usize>) {
    use crate::hexgrid;
    // Each stop snaps to its HEX cell (`axial_of`); consecutive stops connect by the CANONICAL hex
    // line (`hexgrid::line` — drawn from the lexicographically smaller endpoint then reversed, so
    // `a -> b` is the EXACT reverse of `b -> a`, the same edge set). Vertices are hex cell CENTRES, so
    // two lines whose stops land in the same cells emit byte-identical edges — the foundation the
    // cross-line mutex rests on (`dispatch::node_of` recovers the same cell from a shared centre).
    let mut poly: Vec<PointMm> = Vec::new();
    let mut stop_idx: Vec<usize> = Vec::with_capacity(pts.len());
    let mut prev: Option<hexgrid::Axial> = None;
    for &p in pts {
        let c = hexgrid::axial_of(p, cell_mm);
        match prev {
            None => {
                poly.push(hexgrid::center_of(c, cell_mm));
                stop_idx.push(0);
            }
            Some(pc) => {
                // `line(pc, c)` is pc..=c inclusive; pc is already in `poly`, so push the rest.
                for &cc in &hexgrid::line_costed(pc, c, cost)[1..] {
                    poly.push(hexgrid::center_of(cc, cell_mm));
                }
                // `c` is the last vertex pushed (or, if c == pc, the previous vertex ⇒ zero-length span).
                stop_idx.push(poly.len().saturating_sub(1));
            }
        }
        prev = Some(c);
    }
    (poly, stop_idx)
}

/// Centripetal Catmull-Rom through `pts`. Returns the dense polyline and, for each input
/// stop, its index in that polyline (so stations remain exact vertices on the curve).
fn smooth_centripetal(pts: &[PointMm], samples: usize) -> (Vec<PointMm>, Vec<usize>) {
    let n = pts.len();
    let samples = samples.max(1);
    if n < 2 {
        return (pts.to_vec(), (0..n).collect());
    }
    let p: Vec<(f64, f64)> = pts.iter().map(|q| (q.x_mm as f64, q.y_mm as f64)).collect();
    let mut out: Vec<PointMm> = Vec::with_capacity(n * samples + 1);
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
        for k in 0..samples {
            let t = k as f64 / samples as f64;
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
