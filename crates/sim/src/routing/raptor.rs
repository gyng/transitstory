//! Time-dependent (frequency-based) RAPTOR — the upgrade from min-transfer `BfsRouter`. Instead
//! of counting legs, it minimises **expected travel time**: each boarded leg costs an expected
//! wait of ~headway/2 plus the in-vehicle time to ride it (arc-length ÷ a mode-effective speed,
//! plus per-stop dwell). Rounds = trips, so the `max_legs` bound is the RAPTOR round count.
//!
//! It is a strict drop-in: same `Router::plan` port, same `Vec<Leg>` output (so `Pax`/board_alight
//! are untouched), and fully deterministic — integer ms throughout, index-ordered iteration, no
//! HashMap iteration. No real timetable exists (vehicles are headway-dispatched, positions
//! emergent), so the honest model is frequency-based: wait = headway/2, ride = mode speed.
use super::{Footpaths, Leg, Router};
use crate::ids::{LineId, StationId};
use crate::line::Line;

/// Unreachable sentinel (kept well below i64::MAX so `+ wait` can't overflow).
const INF: i64 = i64::MAX / 4;
/// Effective cruise speed as a fraction of a mode's top speed (accounts for accel/decel/curves),
/// expressed as a rational so the ride-time estimate stays pure integer: time = dist·NUM/(v·DEN).
/// NUM folds in the mm/s → ms factor (×1000): 0.75·v_max ⇒ dist·1000/(0.75 v) = dist·4000/(3 v).
const RIDE_NUM: i64 = 4000;
const RIDE_DEN: i64 = 3;

/// Frequency-aware RAPTOR. Zero-sized; swaps in for `BfsRouter` behind `trait Router`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RaptorRouter;

impl Router for RaptorRouter {
    fn plan(
        &self,
        lines: &[Line],
        serving: &[Vec<LineId>],
        footpaths: &Footpaths,
        origin: StationId,
        dest: StationId,
        max_legs: usize,
    ) -> Option<Vec<Leg>> {
        plan_raptor(lines, serving, footpaths, origin, dest, max_legs)
    }

    fn reachable(&self, lines: &[Line], serving: &[Vec<LineId>], footpaths: &Footpaths, origin: StationId, max_legs: usize) -> Vec<i64> {
        reachable_raptor(lines, serving, footpaths, origin, max_legs)
    }
}

/// One directed traversal of a line: the ordered stops a vehicle visits and the cumulative
/// in-vehicle time (ms) to reach each, so arrival from boarding at index `a` to alighting at `b`
/// is `board_time + cum_ms[b] - cum_ms[a]`. Out-and-back lines yield two routes (each direction);
/// loops yield one forward route, doubled so any board point can reach any stop within a cycle.
struct DirRoute {
    line: LineId,
    wait_ms: i64,
    stops: Vec<StationId>,
    cum_ms: Vec<i64>,
}

#[inline]
fn ride_ms(dist_mm: i64, v_mm_s: i64) -> i64 {
    dist_mm.max(0).saturating_mul(RIDE_NUM) / (RIDE_DEN * v_mm_s.max(1))
}

/// Build the directed routes for every operational line (the RAPTOR "route" set).
fn build_routes(lines: &[Line]) -> Vec<DirRoute> {
    let mut routes = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        if line.removed || line.trainset.is_none() {
            continue;
        }
        let line_id = LineId(li as u32);
        let spec = line.vehicle_spec();
        let v = spec.v_max_mm_s;
        let dwell = spec.dwell_ms.max(0);
        // Trains split round-robin across the line's service paths (P3), so each path runs at
        // ~1/npaths the line frequency — the rider's expected wait scales accordingly. npaths==1
        // (a non-branched line) reduces to the old headway/2, so existing routing is unchanged.
        let npaths = line.paths.len().max(1) as i64;
        let wait = line.headway_ms.max(0) * npaths / 2;

        // One directed route set per service path (trunk + each branch). Each path is linear.
        for path in &line.paths {
            let n = path.stops.len();
            if n < 2 {
                continue;
            }
            let arclen = &path.stop_arclen_mm;
            if arclen.len() < n {
                continue; // geometry not built yet — skip defensively
            }
            // Forward inter-stop spans (mm): seg[i] between stop i and i+1.
            let seg = |i: usize| -> i64 { arclen[i + 1] - arclen[i] };

            if path.loop_line && arclen.len() > n {
                // Cyclic forward route, doubled so any boarding reaches all stops within one loop.
                let span_cyc = |i: usize| -> i64 { arclen[i + 1] - arclen[i] };
                let mut stops = Vec::with_capacity(2 * n);
                let mut cum = Vec::with_capacity(2 * n);
                cum.push(0);
                stops.push(path.stops[0]);
                for p in 1..2 * n {
                    let s = span_cyc((p - 1) % n);
                    cum.push(cum[p - 1] + ride_ms(s, v) + dwell);
                    stops.push(path.stops[p % n]);
                }
                routes.push(DirRoute { line: line_id, wait_ms: wait, stops, cum_ms: cum });
            } else {
                // Out-and-back: a forward route and a backward route.
                let mut fwd_stops = Vec::with_capacity(n);
                let mut fwd_cum = Vec::with_capacity(n);
                fwd_cum.push(0);
                fwd_stops.push(path.stops[0]);
                for j in 1..n {
                    fwd_cum.push(fwd_cum[j - 1] + ride_ms(seg(j - 1), v) + dwell);
                    fwd_stops.push(path.stops[j]);
                }
                let mut bwd_stops = Vec::with_capacity(n);
                let mut bwd_cum = Vec::with_capacity(n);
                bwd_cum.push(0);
                bwd_stops.push(path.stops[n - 1]);
                for j in 1..n {
                    bwd_cum.push(bwd_cum[j - 1] + ride_ms(seg(n - 1 - j), v) + dwell);
                    bwd_stops.push(path.stops[n - 1 - j]);
                }
                routes.push(DirRoute { line: line_id, wait_ms: wait, stops: fwd_stops, cum_ms: fwd_cum });
                routes.push(DirRoute { line: line_id, wait_ms: wait, stops: bwd_stops, cum_ms: bwd_cum });
            }
        }
    }
    routes
}

fn plan_raptor(
    lines: &[Line],
    serving: &[Vec<LineId>],
    footpaths: &Footpaths,
    origin: StationId,
    dest: StationId,
    max_legs: usize,
) -> Option<Vec<Leg>> {
    let n = serving.len();
    let (oi, di) = (origin.index(), dest.index());
    if oi >= n || di >= n || oi == di || max_legs == 0 {
        return None;
    }
    let (best, parent, walk_src) = raptor_labels(lines, serving, footpaths, oi, max_legs);
    if best[di] >= INF {
        return None;
    }
    reconstruct(&parent, &walk_src, oi, di, max_legs)
}

/// One-to-all earliest-arrival labels (ms) from `origin` to every station, plus the parent
/// pointers for path reconstruction. This is the shared RAPTOR core: `plan` reconstructs a path
/// from it, `reachable` exposes the label vector for the demand model's accessibility weighting.
fn raptor_labels(
    lines: &[Line],
    serving: &[Vec<LineId>],
    footpaths: &Footpaths,
    oi: usize,
    max_legs: usize,
) -> (Vec<i64>, Vec<Option<(StationId, LineId)>>, Vec<Option<StationId>>) {
    let n = serving.len();
    let routes = build_routes(lines);
    let nlines = lines.len();

    let mut best = vec![INF; n]; // earliest arrival at each station (min over rounds)
    let mut prev = vec![INF; n]; // arrival snapshot at the START of the current round (for boarding)
    let mut parent: Vec<Option<(StationId, LineId)>> = vec![None; n];
    // For a station reached by WALKING this round: the station walked FROM (which was reached by a
    // ride). reconstruct uses it to make consecutive ride legs straddle a footpath gap.
    let mut walk_src: Vec<Option<StationId>> = vec![None; n];
    best[oi] = 0;
    prev[oi] = 0;

    let mut marked = vec![oi]; // stations improved last round → board candidates this round
    let mut in_improved = vec![false; n];
    let mut active_line = vec![false; nlines];

    for _round in 0..max_legs {
        // Mark every line serving a station improved last round (RAPTOR route-marking).
        for v in active_line.iter_mut() {
            *v = false;
        }
        for &s in &marked {
            for &l in &serving[s] {
                if l.index() < nlines {
                    active_line[l.index()] = true;
                }
            }
        }

        let mut improved: Vec<usize> = Vec::new();
        for r in &routes {
            if !active_line[r.line.index()] {
                continue;
            }
            // Scan the directed route once: carry the earliest boarding, relaxing downstream stops.
            let mut board_arr = INF; // arrival time we were aboard at the boarding stop
            let mut cum_at_board = 0i64;
            let mut board_station: Option<StationId> = None;
            for pos in 0..r.stops.len() {
                let st = r.stops[pos];
                let sidx = st.index();
                if sidx >= n {
                    continue;
                }
                if let Some(bs) = board_station {
                    let arr = board_arr.saturating_add(r.cum_ms[pos] - cum_at_board);
                    if arr < best[sidx] {
                        best[sidx] = arr;
                        parent[sidx] = Some((bs, r.line));
                        if !in_improved[sidx] {
                            in_improved[sidx] = true;
                            improved.push(sidx);
                        }
                    }
                }
                // Consider (re)boarding here using the previous round's arrival at this stop.
                let pa = prev[sidx];
                if pa < INF {
                    // Auto-timetable (no editor): the ORIGIN boarding pays the unbiased ~headway/2
                    // (a cold arrival), but a TRANSFER boards a line that's somewhat coordinated with
                    // the feeder, so it waits less — a deterministic 3/8·headway (= 3/4 of wait_ms,
                    // since wait_ms is headway/2). Integer, headway-only → hash-neutral, no clock/tod.
                    let wait = if sidx == oi { r.wait_ms } else { r.wait_ms * 3 / 4 };
                    let board_here = pa + wait;
                    let staying = match board_station {
                        Some(_) => board_arr.saturating_add(r.cum_ms[pos] - cum_at_board),
                        None => INF,
                    };
                    if board_here < staying {
                        board_arr = board_here;
                        cum_at_board = r.cum_ms[pos];
                        board_station = Some(st);
                    }
                }
            }
        }

        // Footpath relaxation: from each station improved by TRANSIT this round (it has a transit
        // parent), relax its walkable neighbours. Only transit-reached stations propagate on foot,
        // so a journey never starts or ends with a walk — the first leg boards at the origin and
        // board_alight's transfer stays the only walk site. A walked-to station is marked so the
        // NEXT round can board a line there; the walk is recorded in walk_src (parent stays a ride).
        let transit_count = improved.len();
        let mut k = 0;
        while k < transit_count {
            let s = improved[k];
            k += 1;
            if parent[s].is_none() || s >= footpaths.len() {
                continue; // origin / walk-reached → don't propagate footpaths (one hop, ride→walk→ride)
            }
            let arr_s = best[s];
            for &(w, wms) in &footpaths[s] {
                let wi = w as usize;
                if wi >= n {
                    continue;
                }
                let cand = arr_s.saturating_add(wms);
                if cand < best[wi] {
                    best[wi] = cand;
                    walk_src[wi] = Some(StationId(s as u32));
                    parent[wi] = None; // reached on foot, not by a ride
                    if !in_improved[wi] {
                        in_improved[wi] = true;
                        improved.push(wi);
                    }
                }
            }
        }

        if improved.is_empty() {
            break; // no station got closer → fixpoint, further rounds add nothing
        }
        // Roll the round forward: this round's results become next round's boarding snapshot.
        prev.copy_from_slice(&best);
        for &s in &improved {
            in_improved[s] = false;
        }
        marked = improved;
    }

    (best, parent, walk_src)
}

/// One-to-all travel-time vector (ms) from `origin` to every station; `i64::MAX` = unreachable
/// within `max_legs`. The demand model weights destination attractiveness by this so that
/// well-connected places (short transit time) draw more trips than far/slow ones — the network
/// shapes the demand. RAPTOR already computes the whole label vector, so this is near-free.
fn reachable_raptor(lines: &[Line], serving: &[Vec<LineId>], footpaths: &Footpaths, origin: StationId, max_legs: usize) -> Vec<i64> {
    let n = serving.len();
    let oi = origin.index();
    if oi >= n || max_legs == 0 {
        return Vec::new();
    }
    let (best, _, _) = raptor_labels(lines, serving, footpaths, oi, max_legs);
    best.into_iter().map(|t| if t >= INF { i64::MAX } else { t }).collect()
}

/// Walk parent pointers from `dest` back to `origin` into an ordered leg list. Legs are ride-only;
/// a footpath shows up as a GAP where one leg's `alight` differs from the next leg's `board` (the
/// rider walks it). When a leg's boarding station was itself reached on foot (`walk_src`), the
/// PREVIOUS leg alighted at the station walked from — so we step `cur` back to there.
fn reconstruct(
    parent: &[Option<(StationId, LineId)>],
    walk_src: &[Option<StationId>],
    oi: usize,
    di: usize,
    max_legs: usize,
) -> Option<Vec<Leg>> {
    let mut legs = Vec::new();
    let mut cur = di;
    for _ in 0..max_legs + 1 {
        let (board, line) = parent[cur]?;
        legs.push(Leg { line, board, alight: StationId(cur as u32) });
        // Did the rider walk in to `board`? If so, the previous leg alighted at the walked-from
        // station (the footpath gap); otherwise the previous leg alighted right at `board`.
        cur = match walk_src.get(board.index()).copied().flatten() {
            Some(src) => src.index(),
            None => board.index(),
        };
        if cur == oi {
            legs.reverse();
            return Some(legs);
        }
    }
    None // chain didn't reach the origin within the bound (shouldn't happen) — fail safe
}
