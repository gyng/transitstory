//! Struct-of-Arrays vehicle store + the 1-D arc-length motion integrator (trapezoidal
//! accel/cruise/brake + fixed dwell, out-and-back). Positions are integer mm. Holds
//! previous-tick AND current-tick positions so the frontend interpolates at 60fps.
use crate::ids::LineId;
use crate::world::World;

#[derive(Default)]
pub struct VehicleSoA {
    pub line: Vec<LineId>,
    /// Which service PATH of the line this vehicle runs (`line.paths[path]`): 0 = trunk, 1.. = a
    /// branch (P3). Trains are assigned round-robin across a branched line's paths.
    pub path: Vec<u8>,
    /// Arc-length position along the line polyline (mm), current and previous tick.
    pub s_mm: Vec<i64>,
    pub prev_s_mm: Vec<i64>,
    /// Travel direction: +1 forward along stops, -1 returning (out-and-back).
    pub dir: Vec<i8>,
    /// Cartesian position (mm) derived from `s_mm`, current and previous tick.
    pub x_mm: Vec<i64>,
    pub y_mm: Vec<i64>,
    pub prev_x_mm: Vec<i64>,
    pub prev_y_mm: Vec<i64>,
    /// Heading in radians (for sprite rotation).
    pub angle: Vec<f32>,
    /// Current speed (mm/s).
    pub v_mm_s: Vec<i64>,
    /// Dwell timer: vehicle is stopped boarding/alighting until this clock time.
    pub dwell_until_ms: Vec<i64>,
    /// Onboard passenger count (= onboard_pax.len(); kept for the hash/render).
    pub onboard: Vec<u16>,
    /// Onboard passengers with their multi-leg routes (capacity-capped board/alight).
    pub onboard_pax: Vec<Vec<crate::pax::Pax>>,
    /// Station id this vehicle arrived at THIS tick (-1 otherwise); consumed by board/alight.
    pub at_station: Vec<i32>,
    /// RENDER-ONLY (TTD L2, **excluded from `Canonical`**): the platform berth index this consist holds
    /// (or is pulling into) at a multi-platform station this tick, else -1. Re-derived every tick from the
    /// dwell order; drives the lateral render offset so parallel-berthed trains don't draw on top of each
    /// other. Never hashed — like `x_mm`/`angle`, a derived display value, not state.
    pub berth_idx: Vec<i32>,
}

impl VehicleSoA {
    #[inline]
    pub fn len(&self) -> usize {
        self.line.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.line.is_empty()
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.path.clear();
        self.s_mm.clear();
        self.prev_s_mm.clear();
        self.dir.clear();
        self.x_mm.clear();
        self.y_mm.clear();
        self.prev_x_mm.clear();
        self.prev_y_mm.clear();
        self.angle.clear();
        self.v_mm_s.clear();
        self.dwell_until_ms.clear();
        self.onboard.clear();
        self.onboard_pax.clear();
        self.at_station.clear();
        self.berth_idx.clear();
    }
}

/// Index of the next stop in the travel direction (the end index if past the last stop,
/// which triggers a reversal in `advance`).
fn next_stop_index(arc: &[i64], s: i64, dir: i64) -> usize {
    if dir > 0 {
        for i in 0..arc.len() {
            if arc[i] > s + 1 {
                return i;
            }
        }
        arc.len().saturating_sub(1)
    } else {
        for i in (0..arc.len()).rev() {
            if arc[i] < s - 1 {
                return i;
            }
        }
        0
    }
}

/// Advance every vehicle one fixed step along its line (trapezoidal speed + dwell + reverse
/// at ends). Integer mm/ms throughout; deterministic. Records prev positions for interpolation.
/// Street-running surface track through built-up land is slow (tram-like) — a real downside
/// of NOT grade-separating in the dense core (~43 km/h on the CLOCK; ×CLOCK_SCALE frame).
const STREET_SPEED_MM_S: i64 = 360_000;
/// A bus OFF the road network crawls (no road to run on) — ~25 km/h clock. On a `class::ROAD`
/// cell it runs at its full spec speed (subject to congestion). The bus's road-bound identity.
const OFF_ROAD_BUS_MM_S: i64 = 210_000;
/// A ferry forced OFF the water (over land) barely moves — its identity is water-bound, so the
/// geometry keeps it on `class::WATER`; this is the penalty for any leg that strays onto land.
const OFF_WATER_FERRY_MM_S: i64 = 90_000;

/// Monotone round-trip arc-length `p` for a vehicle — the follow coordinate in which every train on
/// a line advances in increasing `p` (wrapping at the circuit length), so "the train ahead" is just
/// the next-larger `p`. Loop and out-and-back unify here. Pure integer (`s`, `dir`, `total`).
#[inline]
fn loop_p(s: i64, dir: i8, total: i64, loop_line: bool) -> i64 {
    if !loop_line && dir < 0 {
        2 * total - s
    } else {
        s
    }
}

// --- P2 single-track meet helpers (docs/capacity-roadmap.md) ---------------------------------
// Occupancy is per-tick SCRATCH: re-derived from the SoA each tick (never persisted, never hashed),
// kept as a sorted `Vec<(key, dir)>` looked up by binary search — no HashMap iteration, so the meet
// protocol stays bit-for-bit deterministic. Determinism is inherited from the already-hashed
// s_mm/dir/line/path + track_type. The ONE hashed addition for P2 is `Path.track_type`.

/// Packed total-order key for a single span: `(line, path, span)`.
#[inline]
fn seg_key(line: u32, path: u8, span: u32) -> u64 {
    ((line as u64) << 40) | ((path as u64) << 32) | (span as u64)
}

// Single-track working is BLOCK working keyed by TRAIN IDENTITY (vehicle index), not direction: a
// single span holds exactly ONE train at a time. A train at a TERMINUS (a dead-end, not a passing
// place) reserves the adjacent single span through its whole turnaround, so opposing trains hold at
// the upstream passing place. This guarantees the head-on (mutual-exclusion) invariant. LIVENESS
// (no deadlock) is guaranteed UPSTREAM, by the dispatch single-track capacity cap (dispatch.rs):
// over-provisioning a single-track line is self-limiting, because the meet protocol alone cannot
// untangle a P1×P2 cycle once trains outnumber the line's passing capacity. The reservation is
// re-derived from positions each tick (start-of-tick occupants get the lowest index, since A.1 scans
// in index order), so it is bit-for-bit deterministic — never persisted, never hashed.

/// Record a single-span occupant; first writer (lowest index) wins. One sorted row per key.
fn occ_claim(occ: &mut Vec<(u64, u32)>, key: u64, owner: u32) {
    if let Err(pos) = occ.binary_search_by_key(&key, |&(k, _)| k) {
        occ.insert(pos, (key, owner));
    }
}

/// The vehicle index holding a single span at start-of-tick, if any.
fn occ_owner(occ: &[(u64, u32)], key: u64) -> Option<u32> {
    occ.binary_search_by_key(&key, |&(k, _)| k).ok().map(|p| occ[p].1)
}

/// Claim an EMPTY single span this tick: first claimant (lowest index, since the resolve pass scans
/// in ascending index order) wins; a later contender on the same span is denied. Deterministic.
fn try_claim(claimed: &mut Vec<(u64, u32)>, key: u64, owner: u32) -> bool {
    match claimed.binary_search_by_key(&key, |&(k, _)| k) {
        Ok(p) => claimed[p].1 == owner,
        Err(pos) => {
            claimed.insert(pos, (key, owner));
            true
        }
    }
}

// --- P4 junction-group mutex helpers (docs/capacity-roadmap.md) -------------------------------
// A switch cluster is a mutex over a per-path arc-length SPAN [lo, hi]: a consist occupies it while
// its [tail, head] segment overlaps the span. Reuses the P2 occ_claim/occ_owner/try_claim machinery
// keyed on the group id (`junc_key`) instead of a single span — re-derived each tick, never hashed.

/// Packed total-order key for a junction GROUP: `(line, key_station)`. A distinct key space from
/// `seg_key` (a grouped-point mutex, not a span mutex); injective in its two 32-bit fields.
#[inline]
fn junc_key(line: u32, key_station: u32) -> u64 {
    ((line as u64) << 32) | (key_station as u64)
}

/// Packed total-order key for a station PLATFORM BERTH (TTD L2): `(station, berth)`. Line-INDEPENDENT (a
/// berth is shared infrastructure — two lines at one station contend for its berths), in its own per-tick
/// `berth_occ` Vec (so the `<<8` key space never aliases `seg_key`/`junc_key`). `berth < MAX_PLATFORMS`.
#[inline]
fn berth_key(station: u32, berth: u32) -> u64 {
    ((station as u64) << 8) | (berth as u64)
}

/// Does consist segment `[tail, head]` OVERLAP the group span `[lo, hi]`? Half-open at BOTH gates
/// (`head > lo` and `tail < hi`) — a train resting with its head exactly on the near gate `lo` does
/// NOT occupy (it sits at the passing place), matching P2's strict `strictly_inside`. The ONE shared
/// predicate used in BOTH Phase A.1.5 (occupancy) and Phase B.4 (owner early-out), so a head-on-gate
/// train can never occupy-in-one-pass-but-not-the-other (a deterministic-but-wrong blessed state).
#[inline]
fn group_overlap(tail: i64, head: i64, lo: i64, hi: i64) -> bool {
    let (a, b) = if tail <= head { (tail, head) } else { (head, tail) };
    b > lo && a < hi
}

/// Is single span `[span_lo, span_hi]` on `(line, path)` covered by a junction BLOCK (a P4 switch
/// cluster or an S2 single-span block)? When so, that block is the SOLE meet authority on the span and
/// P2's per-`(line,path,span)` meet (Phase A.1 / B / B.5) must SKIP it: otherwise a train resting
/// inside the block and a train P2-gating the same physical span form a P2×junction wait-for cycle
/// (the deadlock the S2 review found). A block's window contains the whole span (`lo<=span_lo &&
/// hi>=span_hi`). Inert when `junctions` is empty (the non-branched / fully-double-trunk case).
#[inline]
fn span_block_covered(junctions: &[crate::world::Junction], line: usize, path: u8, span_lo: i64, span_hi: i64) -> bool {
    junctions.iter().any(|j| {
        j.line.index() == line
            && j.span_by_path.iter().any(|&(p, lo, hi)| p == path && lo <= span_lo && hi >= span_hi)
    })
}

/// Is single span `[span_lo, span_hi]` on `(line, path)` covered by a CROSS-LINE shared-rail block
/// (Phase 2)? When so, that block is the SOLE meet authority on the shared physical edge — the per-line
/// P2/P4 mutex must SKIP it, or a train held by the cross-line block while gated by its own line's
/// per-line meet forms a wait-for cycle (the S2 skip-guard lesson, generalised cross-line). Inert when
/// `cross_blocks` is empty (continuous / non-grid / non-shared networks).
#[inline]
fn cross_span_covered(cross_blocks: &[crate::world::CrossBlock], line: usize, path: u8, span_lo: i64, span_hi: i64) -> bool {
    cross_blocks.iter().any(|b| {
        b.by_lane.iter().any(|&(l, p, lo, hi)| l as usize == line && p == path && lo <= span_lo && hi >= span_hi)
    })
}

pub(crate) fn advance(world: &mut World, dt_ms: i64) {
    let clock = world.clock_ms;
    let lines = &world.lines;
    let stations = &world.stations; // TTD L2: platform_count per station (disjoint field co-borrow)
    let disabled = &world.line_disabled_until_ms; // #war: RAIDED lines — their consists freeze in place
    let junctions = &world.junctions; // P4: immutable co-borrow (disjoint field) alongside &mut vehicles
    let cross_blocks = &world.cross_blocks; // Phase 2: cross-line shared-rail blocks (shared-rail.md)
    let build_lookup = &world.build_lookup;
    let build_cell_mm = world.build_cell_mm;
    let v = &mut world.vehicles;
    // TTD signals (render-only scratch): rebuilt fresh each tick from the meet occupancy below. Disjoint
    // field borrow alongside &lines / &mut vehicles; never read back into state, so it can't perturb the hash.
    let sig = &mut world.signal_occupancy;
    sig.clear();

    // Self-induced congestion: count BUSES per road cell at the START of the tick (the player's own
    // service is a road user). Transient (not hashed); built by iterating the ordered vehicle Vec
    // and read only via get() — no HashMap iteration, so it stays deterministic.
    let mut bus_load: rustc_hash::FxHashMap<(i32, i32), u16> = rustc_hash::FxHashMap::default();
    for i in 0..v.len() {
        if lines[v.line[i].index()].mode == crate::trainset::tmode::BUS {
            let key = (v.x_mm[i].div_euclid(build_cell_mm) as i32, v.y_mm[i].div_euclid(build_cell_mm) as i32);
            *bus_load.entry(key).or_insert(0) += 1;
        }
    }

    // Block-following pre-pass (P1, docs/capacity-roadmap.md). Snapshot every vehicle's start-of-tick
    // loop coordinate `p_start`, and the index of the train AHEAD on its line. Vehicles of a line are
    // a contiguous, increasing-`p` run in the SoA (dispatch order) and the follow clamp preserves that
    // order, so the leader of run-position j is (j+1) cyclic. Using start-of-tick leader positions
    // makes the clamp order-independent (deterministic). A lone train's leader is itself ⇒ its gap is
    // ~the full circuit, so the clamp never binds without a special case.
    let n = v.len();
    // P4: per-vehicle consist length (the junction-clearing footprint). A train holds a switch
    // cluster while its [head − dir·len, head] segment overlaps it, so a longer consist clears it
    // slower — for free, no new state (`length_mm` already drives P1 spacing).
    let len_of: Vec<i64> = (0..n).map(|i| lines[v.line[i].index()].vehicle_spec().length_mm).collect();
    let mut p_start = vec![0i64; n];
    let mut leader = vec![0usize; n];
    {
        // Group by (line, PATH): trains on different service paths of a branched line diverge, so
        // each path is its own follow stream. Vehicles of a (line, path) are a contiguous,
        // increasing-`p` run in the SoA (dispatch order); the clamp preserves that order, so the
        // leader of run-position j is (j+1) cyclic. (Cross-path conflict on the shared trunk is the
        // deferred junction phase P4 — here paths follow independently.)
        for i in 0..n {
            let line = &lines[v.line[i].index()];
            let (total, loop_line) = line
                .paths
                .get(v.path[i] as usize)
                .map(|p| (p.length_mm(), p.loop_line))
                .unwrap_or((0, false));
            p_start[i] = loop_p(v.s_mm[i], v.dir[i], total, loop_line);
        }
        let mut i = 0usize;
        while i < n {
            let (li, pa) = (v.line[i], v.path[i]);
            let mut j = i;
            while j < n && v.line[j] == li && v.path[j] == pa {
                j += 1;
            }
            for k in i..j {
                leader[k] = if k + 1 < j { k + 1 } else { i };
            }
            i = j;
        }
    }

    // The move integrator runs in three index-ordered passes (P2, docs/capacity-roadmap.md). P1's
    // pre-pass above feeds Phase A. Splitting derive from commit is behaviour-IDENTICAL for
    // double-track lines: P1 already reads start-of-tick snapshots (`p_start`/`bus_load`), not
    // committed positions, so no compute reads another train's new position — the only hash change is
    // the new `track_type` field. The occupancy Vecs are per-tick scratch (re-derived, never hashed).
    let mut desired_ds = vec![0i64; n];
    let mut desired_nv = vec![0i64; n];
    let mut eff_dir = vec![0i64; n];
    let mut c_s = vec![0i64; n];
    let mut c_stop_idx = vec![0usize; n];
    let mut c_next_arc = vec![0i64; n];
    let mut c_has_path = vec![false; n]; // valid path (gets prev-capture, maybe no move)
    let mut c_move = vec![false; n]; // participates in the move (path ok, total>0, not dwelling)
    let mut c_dwell = vec![false; n];
    // TTD L2 multi-platform scratch: the P1-UNCLAMPED advance (so Phase A.3 can restore it when relaxing a
    // follower into a free berth), and which station each consist is DWELLING at this tick (-1 = none).
    let mut unclamped_ds = vec![0i64; n];
    let mut unclamped_nv = vec![0i64; n];
    let mut dwell_station = vec![-1i32; n];

    // TTD L2 — berth occupancy (derived, never hashed). Reset the render-only berth index, find each
    // consist's dwelling station (parked on a stop's arclen with its dwell timer live), and SEED the berth
    // mutex: every dwelling consist claims the lowest free berth at its station (index order ⇒ deterministic,
    // first-claimant-wins like the P4 mutex). `berth_occ` lives one tick. At K=1 a station's single dwelling
    // train fills its only berth (free = 0), so Phase A.3's relaxation never fires ⇒ byte-identical.
    let mut berth_occ: Vec<(u64, u32)> = Vec::new();
    for i in 0..n {
        v.berth_idx[i] = -1;
        if clock >= v.dwell_until_ms[i] {
            continue;
        }
        let line = &lines[v.line[i].index()];
        let Some(path) = line.paths.get(v.path[i] as usize) else { continue };
        let s = v.s_mm[i];
        if let Some(stop_idx) = path.stop_arclen_mm.iter().position(|&a| a == s) {
            dwell_station[i] = path.station_for_stop_index(stop_idx).0 as i32;
        }
    }
    for i in 0..n {
        let st = dwell_station[i];
        if st < 0 {
            continue;
        }
        let k = stations.get(st as usize).map(|s| s.platform_count).unwrap_or(1).max(1) as u32;
        for b in 0..k {
            if occ_owner(&berth_occ, berth_key(st as u32, b)).is_none() {
                occ_claim(&mut berth_occ, berth_key(st as u32, b), i as u32);
                v.berth_idx[i] = b as i32;
                break;
            }
        }
    }

    // Phase A.1 — start-of-tick single-span occupancy, scanned in index order into a sorted Vec.
    // A train occupies a SINGLE span if it is (a) strictly inside it, or (b) sitting at a TERMINUS
    // gate adjacent to it — the terminus is a dead-end, not a passing place, so its single span is
    // reserved through the whole turnaround (this is the deadlock fix).
    let mut occ: Vec<(u64, u32)> = Vec::new();
    for i in 0..n {
        let line = &lines[v.line[i].index()];
        let path = match line.paths.get(v.path[i] as usize) {
            Some(p) => p,
            None => continue,
        };
        let nsp = path.stop_arclen_mm.len();
        if nsp < 2 || path.loop_line {
            continue; // a loop runs one-way (no opposing direction) ⇒ no meets ⇒ P2 never binds
        }
        let s = v.s_mm[i];
        let span = path.strictly_inside(s).or_else(|| {
            if path.loop_line {
                None // a loop has no terminus
            } else if s == path.stop_arclen_mm[0] {
                Some(0) // first stop ⇒ span 0
            } else if s == path.stop_arclen_mm[nsp - 1] {
                Some(nsp - 2) // last stop ⇒ last span
            } else {
                None // a through-station gate is a passing place — owns nothing
            }
        });
        if let Some(sp) = span {
            if path.track_type.get(sp).copied().unwrap_or(0) == crate::line::track::SINGLE {
                let lo = path.stop_arclen_mm[sp];
                let hi = path.stop_arclen_mm.get(sp + 1).copied().unwrap_or(lo);
                // S2/Phase 2: a junction block OR a cross-line block owns shared single spans — P2 must
                // not also claim them (the sole-authority skip-guard, else a P2×block wait-for cycle).
                if !span_block_covered(junctions, v.line[i].index(), v.path[i], lo, hi)
                    && !cross_span_covered(cross_blocks, v.line[i].index(), v.path[i], lo, hi)
                {
                    occ_claim(&mut occ, seg_key(v.line[i].index() as u32, v.path[i], sp as u32), i as u32);
                }
            }
        }
    }

    // TTD signals: a marker on EVERY single span — GREEN (clear, status 0) or RED (occupied, status 1) —
    // so the whole block network reads at a glance, like TTD's signal aspects. Path 0 (the trunk) per line
    // (branch spurs deferred); block-covered spans (junction/cross) are owned by their own mutex, skipped.
    // Render-only scratch (golden-neutral); WAITING amber is layered on at the Phase B meet gate below.
    for (li, line) in lines.iter().enumerate() {
        let path = match line.paths.first() {
            Some(p) if !p.loop_line => p,
            _ => continue, // a loop runs one-way (no meets) ⇒ no single-track signals
        };
        for sp in 0..path.track_type.len() {
            if path.track_type[sp] != crate::line::track::SINGLE {
                continue;
            }
            let lo = path.stop_arclen_mm.get(sp).copied().unwrap_or(0);
            let hi = path.stop_arclen_mm.get(sp + 1).copied().unwrap_or(lo);
            if span_block_covered(junctions, li, 0, lo, hi) || cross_span_covered(cross_blocks, li, 0, lo, hi) {
                continue;
            }
            let occupied = occ_owner(&occ, seg_key(li as u32, 0, sp as u32)).is_some();
            sig.push(crate::world::SignalOccupancy {
                line: li as u32,
                path: 0,
                span: sp as u32,
                mid_mm: (lo + hi) / 2,
                status: if occupied { 1 } else { 0 },
            });
        }
    }

    // Phase A.1.5 — start-of-tick junction-GROUP occupancy (P4). A consist occupies a switch cluster
    // when its [tail, head] segment overlaps the cluster's span on its OWN path; first writer (lowest
    // index) wins — the same deterministic tiebreak as P2's A.1. Inert (no scan) when there are no
    // junctions, so a non-branched network is byte-identical to pre-P4.
    let mut junc_occ: Vec<(u64, u32)> = Vec::new();
    if !junctions.is_empty() {
        for i in 0..n {
            let li = v.line[i].index();
            let head = v.s_mm[i];
            let tail = head - (v.dir[i] as i64) * len_of[i];
            for j in junctions {
                if j.line.index() != li {
                    continue;
                }
                let (lo, hi) = match j.span_by_path.iter().find(|&&(p, _, _)| p == v.path[i]) {
                    Some(&(_, lo, hi)) => (lo, hi),
                    None => continue, // this path doesn't pass the cluster
                };
                if group_overlap(tail, head, lo, hi) {
                    occ_claim(&mut junc_occ, junc_key(li as u32, j.key_station.index() as u32), i as u32);
                }
            }
        }
    }

    // Phase A.1.7 — start-of-tick CROSS-LINE block occupancy (Phase 2, shared-rail.md). A consist
    // occupies a shared-rail block when its [tail, head] overlaps its lane window — keyed on the
    // line-INDEPENDENT `block_id`, so two DISTINCT lines on one physical rail contend for ONE row
    // (unlike junc_key/seg_key). Inert (no scan) when there are no cross blocks ⇒ continuous / non-grid
    // / non-shared networks are byte-identical. Owner = lowest index this tick (fairness in B.6/cap).
    let mut cross_occ: Vec<(u64, u32)> = Vec::new();
    if !cross_blocks.is_empty() {
        for i in 0..n {
            let li = v.line[i].index() as u32;
            let pi = v.path[i];
            let head = v.s_mm[i];
            let tail = head - (v.dir[i] as i64) * len_of[i];
            for blk in cross_blocks {
                if blk
                    .by_lane
                    .iter()
                    .any(|&(l, p, lo, hi)| l == li && p == pi && group_overlap(tail, head, lo, hi))
                {
                    occ_claim(&mut cross_occ, blk.block_id, i as u32);
                }
            }
        }
    }

    // Phase A.2 — per-train P1-clamped desired advance (writes scratch only, commits nothing).
    for i in 0..n {
        let line = &lines[v.line[i].index()];
        let path = match line.paths.get(v.path[i] as usize) {
            Some(p) => p,
            None => continue,
        };
        c_has_path[i] = true;
        let total = path.length_mm();
        if total <= 0 || path.arclen_mm.len() < 2 {
            continue;
        }
        // Rail-attack (#war): a RAIDED line is frozen — its consist holds in place (c_move stays false ⇒
        // no advance, no boarding, no delivery) until the disable timer lapses, then it resumes. The Vec is
        // lazily grown, so absent ⇒ 0 ⇒ never frozen (transit + goldens take this path for every train).
        if disabled.get(v.line[i].index()).copied().unwrap_or(0) > clock {
            continue;
        }
        if clock < v.dwell_until_ms[i] {
            c_dwell[i] = true;
            continue;
        }

        let spec = line.vehicle_spec();
        // Loops always run forward (+1); out-and-back uses the stored direction.
        let dir = if path.loop_line { 1 } else { v.dir[i] as i64 };
        let s = v.s_mm[i];
        // Stops sit at specific arc-lengths along the smoothed polyline.
        let stop_idx = next_stop_index(&path.stop_arclen_mm, s, dir);
        let next_arc = path.stop_arclen_mm[stop_idx];
        let dist_to_stop = (next_arc - s).abs();

        let accel_step = spec.accel_mm_s2 * dt_ms / 1000;
        let decel_step = spec.decel_mm_s2 * dt_ms / 1000;
        let vcur = v.v_mm_s[i];
        // Effective top speed = min(trainset vmax, local curve speed cap, street-running cap).
        let mut vmax_eff = spec.v_max_mm_s.min(path.speed_cap_at(s));
        // Surface speed depends on the ground class (the buildability raster). Buses are road-bound;
        // rail/heavy are tram-capped only through dense built-up land.
        let span = path.span_of(s);
        if path.span_mode.get(span).copied().unwrap_or(0) == crate::line::mode::SURFACE {
            let (cx, cy) = path.point_at(s);
            let key = (cx.div_euclid(build_cell_mm) as i32, cy.div_euclid(build_cell_mm) as i32);
            let cell = build_lookup.get(&key).copied().unwrap_or(crate::city::class::OPEN);
            if line.mode == crate::trainset::tmode::BUS {
                if cell != crate::city::class::ROAD {
                    vmax_eff = vmax_eff.min(OFF_ROAD_BUS_MM_S); // off-road: crawl, no road
                } else {
                    // On a road, share it with traffic. Congestion = time-of-day × LOCAL built-up
                    // density (BUILT cells in the 3×3 around this road cell — heavier traffic
                    // downtown). Integer over the clock + raster → hash-safe.
                    let mut built = 0i64;
                    for ddx in -1..=1 {
                        for ddy in -1..=1 {
                            if build_lookup.get(&(key.0 + ddx, key.1 + ddy)).copied().unwrap_or(0)
                                == crate::city::class::BUILT
                            {
                                built += 1;
                            }
                        }
                    }
                    let occr = bus_load.get(&key).copied().unwrap_or(0) as i64;
                    vmax_eff = vmax_eff * crate::tod::congestion_at(clock, built, occr) / 100;
                }
            } else if line.mode == crate::trainset::tmode::FERRY {
                // Ferries are water-bound: full speed on open WATER, barely moving on land.
                if cell != crate::city::class::WATER {
                    vmax_eff = vmax_eff.min(OFF_WATER_FERRY_MM_S);
                }
            } else if cell == crate::city::class::BUILT {
                vmax_eff = vmax_eff.min(STREET_SPEED_MM_S);
            }
        }
        let brake_dist =
            (vcur as i128 * vcur as i128 / (2 * spec.decel_mm_s2.max(1) as i128)) as i64;

        let mut nv = if dist_to_stop <= brake_dist {
            (vcur - decel_step).max(0)
        } else {
            (vcur + accel_step).min(vmax_eff)
        };
        if nv == 0 && dist_to_stop > 0 {
            nv = accel_step.max(1); // crawl so we always reach the stop (no stall)
        }
        nv = nv.min(vmax_eff); // hold the curve cap even mid-brake

        let ds = nv * dt_ms / 1000;
        // TTD L2: remember the P1-UNCLAMPED advance (still braked to halt at `next_arc`, so it never
        // overruns the stop) — Phase A.3 restores it when relaxing a follower into a free berth.
        unclamped_ds[i] = ds;
        unclamped_nv[i] = nv;
        // Block following (P1): cap this tick's advance so the head holds a braking-distance + standoff
        // gap behind the LEADER'S TAIL, in the loop coordinate `p`. Homogeneous lines run untouched.
        let round = if path.loop_line { total } else { 2 * total };
        let len_lead = spec.length_mm; // leader shares this line ⇒ same consist length
        let gap_ht = (p_start[leader[i]] - len_lead - p_start[i]).rem_euclid(round.max(1));
        let room = (gap_ht - crate::trainset::block_gap_mm(vcur, spec.decel_mm_s2)).max(0);
        let ds = ds.min(room);
        if ds < nv * dt_ms / 1000 {
            nv = ds * 1000 / dt_ms.max(1); // braking for the block ahead
        }

        c_move[i] = true;
        desired_ds[i] = ds;
        desired_nv[i] = nv;
        eff_dir[i] = dir;
        c_s[i] = s;
        c_stop_idx[i] = stop_idx;
        c_next_arc[i] = next_arc;
    }

    // Phase A.3 — TTD L2 MULTI-PLATFORM relaxation (docs/ttd-track-model.md). The single-cell jam is the
    // P1 follow-clamp: a follower brakes a block-gap behind a DWELLING leader and the whole chain idles. If
    // the follower's next stop has a FREE berth AND the train it's clamped behind is dwelling AT that stop,
    // relax the clamp — let it pull up to the platform (the unclamped advance still halts at the stop, so it
    // never overruns) and CLAIM a berth, so it dwells in PARALLEL instead of queueing. Index order; the
    // claim reserves the berth so the next follower sees it taken ⇒ at most K consists pull in. This only
    // un-does the SOFT P1 clamp — the HARD track mutexes (Phase B single-track / B.4 junction / B.6
    // cross-line) still run after this and have the final say on `desired_ds`, so a relaxed follower can
    // never violate track occupancy; the berth itself is a real mutex. INERT at K=1 (free==0 ⇒ never fires
    // ⇒ byte-identical), so the goldens are unchanged beyond L2a's platform_count byte.
    for i in 0..n {
        if !c_move[i] {
            continue;
        }
        let ld = leader[i];
        if ld == i || clock >= v.dwell_until_ms[ld] {
            continue; // lone train, or the leader isn't dwelling — the normal P1 clamp stands
        }
        let line = &lines[v.line[i].index()];
        let Some(path) = line.paths.get(v.path[i] as usize) else { continue };
        let station = path.station_for_stop_index(c_stop_idx[i]).0;
        // the leader must be dwelling at OUR next stop (the jam we're relieving), not some other station.
        if dwell_station[ld] != station as i32 {
            continue;
        }
        let k = stations.get(station as usize).map(|s| s.platform_count).unwrap_or(1).max(1) as u32;
        // claim the lowest free berth, if any; reserving it gates the next follower this same tick.
        let mut got = None;
        for b in 0..k {
            if occ_owner(&berth_occ, berth_key(station, b)).is_none() {
                occ_claim(&mut berth_occ, berth_key(station, b), i as u32);
                got = Some(b);
                break;
            }
        }
        if let Some(b) = got {
            v.berth_idx[i] = b as i32;
            desired_ds[i] = unclamped_ds[i]; // pull up to the platform (still braked to halt at the stop)
            desired_nv[i] = unclamped_nv[i];
        }
    }

    // Phase B — single-track MEET authority: gate entry into a SINGLE span (further min() on `ds`).
    let mut claimed: Vec<(u64, u32)> = Vec::new();
    for i in 0..n {
        if !c_move[i] || desired_ds[i] == 0 {
            continue;
        }
        let line = &lines[v.line[i].index()];
        let path = match line.paths.get(v.path[i] as usize) {
            Some(p) => p,
            None => continue,
        };
        if path.loop_line {
            continue; // loop: one-way, no meets (P2 no-op; only the build-cost discount applies)
        }
        let dir = eff_dir[i];
        let s = c_s[i];
        // The single span the train moves THROUGH toward its far gate (next_arc) — keyed off the stop
        // index, NOT span_of(s+ds): correct for sub-tick spans and for a train departing an entry gate.
        let trav_span = if dir > 0 { c_stop_idx[i].saturating_sub(1) } else { c_stop_idx[i] };
        // S2 skip-guard: a shared-trunk single span owned by a junction block is gated ONLY by that
        // block (Phase B.4); P2's per-path gate here would form a P2×junction wait-for cycle.
        {
            let blo = path.stop_arclen_mm.get(trav_span).copied().unwrap_or(0);
            let bhi = path.stop_arclen_mm.get(trav_span + 1).copied().unwrap_or(blo);
            if span_block_covered(junctions, v.line[i].index(), v.path[i], blo, bhi)
                || cross_span_covered(cross_blocks, v.line[i].index(), v.path[i], blo, bhi)
            {
                continue;
            }
        }
        if path.track_type.get(trav_span).copied().unwrap_or(0) != crate::line::track::SINGLE {
            continue;
        }
        if path.strictly_inside(s) == Some(trav_span) {
            continue; // already the occupant — P1 governs spacing, P2 does not re-gate
        }
        let key = seg_key(v.line[i].index() as u32, v.path[i], trav_span as u32);
        let admit = match occ_owner(&occ, key) {
            Some(o) if o == i as u32 => true, // my OWN reservation (a terminus span I'm departing into)
            Some(_) => false,                 // another train holds the block — HOLD
            None => try_claim(&mut claimed, key, i as u32), // empty: lowest-index this tick wins
        };
        if !admit {
            // Clamp to the ENTRY gate of trav_span (the passing place we wait at; room2==0 on the gate).
            let entry_arc = if dir > 0 {
                path.stop_arclen_mm[trav_span]
            } else {
                path.stop_arclen_mm[trav_span + 1]
            };
            // TTD signals: an AMBER marker at the gate this cart is held at — the block ahead is occupied.
            sig.push(crate::world::SignalOccupancy { line: v.line[i].index() as u32, path: v.path[i], span: trav_span as u32, mid_mm: entry_arc, status: 2 });
            let room2 = (dir * (entry_arc - s)).max(0);
            if desired_ds[i] > room2 {
                desired_ds[i] = room2;
                desired_nv[i] = desired_ds[i] * 1000 / dt_ms.max(1);
            }
        }
    }

    // Phase B.4 — JUNCTION-GROUP MUTEX (P4): gate a train's advance ACROSS a switch-cluster it does
    // not already own (a further min() on `ds`, after P1 block-follow and the P2 meet gate). Because
    // coupled junctions are COALESCED into one group (dispatch.rs), a consist straddles at most ONE
    // group and every contender for any member point contends for that ONE key — so the occupied-
    // junction 2-cycle (the adversaries' break) is structurally impossible: one group ⇒ one owner ⇒
    // the wait-for graph is an acyclic depth-1 forest. Re-clamping a smaller `ds` is idempotent.
    let mut junc_claimed: Vec<(u64, u32)> = Vec::new();
    if !junctions.is_empty() {
        for i in 0..n {
            if !c_move[i] || desired_ds[i] == 0 {
                continue;
            }
            let li = v.line[i].index();
            let dir = eff_dir[i];
            let s = c_s[i];
            let head0 = s;
            let tail0 = head0 - dir * len_of[i];
            for j in junctions {
                if j.line.index() != li {
                    continue;
                }
                let (lo, hi) = match j.span_by_path.iter().find(|&&(p, _, _)| p == v.path[i]) {
                    Some(&(_, lo, hi)) => (lo, hi),
                    None => continue,
                };
                // Already occupying this cluster at start-of-tick? Then P1 spacing governs my
                // clearance and the mutex must NOT re-gate me (the SAME predicate as A.1.5, so the
                // two passes can never disagree on a train sitting on the gate).
                if group_overlap(tail0, head0, lo, hi) {
                    continue;
                }
                // The near edge of the cluster in my travel direction (the gate I hold at): the low
                // edge going forward, the high edge returning.
                let gate = if dir > 0 { lo } else { hi };
                // Only gate when THIS tick's advance would bring my head ACROSS that near gate. Use
                // `s <= gate` / `s >= gate`: the train almost always DEPARTS the junction station —
                // which sits ON the near gate — so the dominant case is `s == gate` (a strict `<`/`>`
                // would miss every gate-departure and the mutex would never bind).
                let head_after = s + dir * desired_ds[i];
                let crossing = (dir > 0 && s <= gate && head_after > gate)
                    || (dir < 0 && s >= gate && head_after < gate);
                if !crossing {
                    continue;
                }
                let key = junc_key(li as u32, j.key_station.index() as u32);
                let admit = match occ_owner(&junc_occ, key) {
                    Some(o) if o == i as u32 => true, // (can't happen given the occupancy early-out)
                    Some(_) => false,                 // another train owns the cluster — HOLD
                    None => try_claim(&mut junc_claimed, key, i as u32), // empty: lowest index wins
                };
                if !admit {
                    // Clamp the head to the near gate (rest AT the passing place, owning nothing).
                    let room = (dir * (gate - s)).max(0);
                    if desired_ds[i] > room {
                        desired_ds[i] = room;
                        desired_nv[i] = desired_ds[i] * 1000 / dt_ms.max(1);
                    }
                }
            }
        }
    }

    // Phase B.6 — CROSS-LINE block mutex (Phase 2, shared-rail.md): gate a consist's advance ACROSS a
    // shared-rail block it does not already own — keyed on the line-INDEPENDENT `block_id`, so two
    // DISTINCT lines on one physical rail mutex. The atomic-whole-block reservation (one owner; others
    // clamp to the near gate owning nothing) keeps the cross-line wait-for graph an acyclic depth-1
    // forest — the single-owner-coalescing trick is unavailable cross-line, so the block IS the one
    // resource. Liveness is guaranteed UPSTREAM by the cross-line dispatch cap (+ the cyclic mutex).
    let mut cross_claimed: Vec<(u64, u32)> = Vec::new();
    if !cross_blocks.is_empty() {
        for i in 0..n {
            if !c_move[i] || desired_ds[i] == 0 {
                continue;
            }
            let li = v.line[i].index() as u32;
            let pi = v.path[i];
            let dir = eff_dir[i];
            let s = c_s[i];
            let tail0 = s - dir * len_of[i];
            let head_after = s + dir * desired_ds[i];
            for blk in cross_blocks {
                for &(l, p, lo, hi) in &blk.by_lane {
                    if l != li || p != pi {
                        continue;
                    }
                    // Already occupying this block (via this window)? Then P1 spacing governs my
                    // clearance; the mutex must NOT re-gate me (same predicate as A.1.7).
                    if group_overlap(tail0, s, lo, hi) {
                        continue;
                    }
                    let gate = if dir > 0 { lo } else { hi };
                    let crossing = (dir > 0 && s <= gate && head_after > gate)
                        || (dir < 0 && s >= gate && head_after < gate);
                    if !crossing {
                        continue;
                    }
                    let admit = match occ_owner(&cross_occ, blk.block_id) {
                        Some(o) if o == i as u32 => true,
                        Some(_) => false, // another consist (any line) owns the block — HOLD
                        None => try_claim(&mut cross_claimed, blk.block_id, i as u32),
                    };
                    if !admit {
                        let room = (dir * (gate - s)).max(0);
                        if desired_ds[i] > room {
                            desired_ds[i] = room;
                            desired_nv[i] = desired_ds[i] * 1000 / dt_ms.max(1);
                        }
                    }
                }
            }
        }
    }

    // Phase B.5 — no train comes to REST strictly inside a single span it didn't already own. This is
    // the forest-of-advancing-roots property: every blocked/stopped train rests at a gate owning
    // nothing, so the wait-for graph is an acyclic depth-1 forest (waiters → the one advancing
    // occupant) ⇒ deadlock-free.
    for i in 0..n {
        if !c_move[i] || desired_nv[i] != 0 {
            continue;
        }
        let line = &lines[v.line[i].index()];
        let path = match line.paths.get(v.path[i] as usize) {
            Some(p) => p,
            None => continue,
        };
        let dir = eff_dir[i];
        let s = c_s[i];
        let end_s = s + dir * desired_ds[i];
        if let Some(esp) = path.strictly_inside(end_s) {
            let elo = path.stop_arclen_mm.get(esp).copied().unwrap_or(0);
            let ehi = path.stop_arclen_mm.get(esp + 1).copied().unwrap_or(elo);
            if path.track_type.get(esp).copied().unwrap_or(0) == crate::line::track::SINGLE
                && path.strictly_inside(s) != Some(esp)
                // S2/Phase 2: a junction OR cross-line block's own B.4/B.6 no-rest handles its spans.
                && !span_block_covered(junctions, v.line[i].index(), v.path[i], elo, ehi)
                && !cross_span_covered(cross_blocks, v.line[i].index(), v.path[i], elo, ehi)
            {
                let entry_arc = if dir > 0 {
                    path.stop_arclen_mm[esp]
                } else {
                    path.stop_arclen_mm[esp + 1]
                };
                let room3 = (dir * (entry_arc - s)).max(0);
                if desired_ds[i] > room3 {
                    desired_ds[i] = room3;
                }
            }
        }
    }

    // Phase C — commit. prev-capture for EVERY vehicle (so 60fps interp reads the true start-of-tick
    // position), then move the active ones using the cached + resolved scratch.
    for i in 0..n {
        v.prev_s_mm[i] = v.s_mm[i];
        v.prev_x_mm[i] = v.x_mm[i];
        v.prev_y_mm[i] = v.y_mm[i];
        if !c_has_path[i] {
            continue;
        }
        if c_dwell[i] {
            v.v_mm_s[i] = 0;
            continue;
        }
        if !c_move[i] {
            continue; // total<=0 / arclen guard: prev captured, no move
        }
        let line = &lines[v.line[i].index()];
        let path = match line.paths.get(v.path[i] as usize) {
            Some(p) => p,
            None => continue,
        };
        let spec = line.vehicle_spec();
        let dir = eff_dir[i];
        let s = c_s[i];
        let ds = desired_ds[i];
        let mut nv = desired_nv[i];
        let next_arc = c_next_arc[i];
        let stop_idx = c_stop_idx[i];
        let mut new_s = s + dir * ds;
        let crossed = (dir > 0 && new_s >= next_arc) || (dir < 0 && new_s <= next_arc);
        if crossed {
            new_s = next_arc;
            nv = 0;
            v.dwell_until_ms[i] = clock + spec.dwell_ms;
            // Record arrival at this stop's station for the board/alight phase.
            v.at_station[i] = path.station_for_stop_index(stop_idx).0 as i32;
            if path.loop_line {
                // Reaching the closing vertex wraps back to the start; never reverse.
                if stop_idx + 1 >= path.stop_arclen_mm.len() {
                    new_s = 0;
                }
                v.dir[i] = 1;
            } else if stop_idx + 1 >= path.stop_arclen_mm.len() {
                v.dir[i] = -1; // forward end -> reverse
            } else if stop_idx == 0 {
                v.dir[i] = 1; // back end -> reverse
            }
        }
        v.s_mm[i] = new_s;
        v.v_mm_s[i] = nv;
        let (x, y) = path.point_at(new_s);
        v.x_mm[i] = x;
        v.y_mm[i] = y;
        let h = path.heading_at(new_s);
        v.angle[i] = if dir < 0 { h + std::f32::consts::PI } else { h };
    }
}
