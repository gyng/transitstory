//! Headway/count-based dispatcher. Places exactly `count` vehicles per served line, evenly
//! spaced around the round-trip loop (so effective headway ≈ round-trip / count). Rebuilds
//! only when `dispatch_dirty` (a line/trainset/headway/running change) — steady running does
//! no rebuild, so motion stays smooth. The timetable layer would replace this later.
use crate::ids::LineId;
use crate::world::World;

pub(crate) fn dispatch(world: &mut World) {
    if !world.dispatch_dirty {
        return;
    }
    world.dispatch_dirty = false;
    world.route_cache.clear(); // network changed; routes may differ
    world.access_cache.clear(); // …and so does destination accessibility
    world.cell_station_dirty = true; // …and the agent population's nearest-served-station map

    // Rebuild the per-station serving-lines map (operational lines only) for routing.
    // A land line with surface track over open water is NOT operational: build legality is a
    // CORE rule (AGENTS — clamps live here, the UI only previews it), so an illegal span parks
    // the whole line — no vehicles, no service — until it's elevated/tunnelled or rerouted.
    // The line stays in the world (the log is append-only) and renders as the red "fix me".
    let nstations = world.stations.len();
    let mut serving: Vec<Vec<LineId>> = vec![Vec::new(); nstations];
    for (li, line) in world.lines.iter().enumerate() {
        if !line.removed && line.trainset.is_some() && line.stops.len() >= 2 && !line.crosses_water_surface {
            // A station is served if ANY service path (trunk or branch) stops there. Dedup so a
            // trunk station (present on every path) counts the line once.
            for path in &line.paths {
                for &st in &path.stops {
                    let i = st.index();
                    if i < nstations && !serving[i].contains(&LineId(li as u32)) {
                        serving[i].push(LineId(li as u32));
                    }
                }
            }
        }
    }
    world.serving = serving;

    // --- P4: rebuild the coalesced junction set (switch-clusters, docs/capacity-roadmap.md) -------
    // A branched line's divergence/convergence points within one consist-length on the trunk are
    // COALESCED into one atomic mutex group (the load-bearing liveness fix: two switches within a
    // consist-length form a 2-cycle deadlock under a naive point-mutex). Re-derived here on the same
    // trigger as `serving`; never hashed (a pure function of the already-hashed line topology). The
    // movement clamp (vehicle.rs Phase A.1.5 + B.4) reads this set each tick.
    let mut junctions: Vec<crate::world::Junction> = Vec::new();
    for (li, line) in world.lines.iter().enumerate() {
        // Same served-line gate as dispatch — a parked/illegal/unserved line runs no vehicles.
        if line.removed || line.trainset.is_none() || line.stops.len() < 2 || line.crosses_water_surface {
            continue;
        }
        if line.branches.is_empty() {
            continue; // PARITY: a non-branched line contributes ZERO junctions (the inert case)
        }
        let len_mm = line.vehicle_spec().length_mm; // the coalescing radius = the consist footprint

        // (a) Unique divergence trunk-stop indices, ascending (dedups a 3-way sharing one diverge_at).
        let mut diverge_idxs: Vec<usize> = Vec::new();
        for b in &line.branches {
            let d = (b.diverge_at as usize).min(line.stops.len().saturating_sub(1));
            if !diverge_idxs.contains(&d) {
                diverge_idxs.push(d);
            }
        }
        diverge_idxs.sort_unstable(); // trunk index ascending == trunk arclen ascending (monotone)

        // (b) For each divergence stop, gather its per-path (path, arclen), sorted by path index.
        //     Every path whose trunk prefix reaches stop d carries the SAME station there (path_specs
        //     shares the prefix); a point reached by >=2 paths is a real fork. diverge_idxs is sorted
        //     and per-path arclen is monotone in trunk-stop index, so `points` are arclen-ascending on
        //     EVERY shared path.
        struct Pt {
            station: crate::ids::StationId,
            by_path: Vec<(u8, i64)>,
        }
        let mut points: Vec<Pt> = Vec::new();
        for &d in &diverge_idxs {
            let station = line.stops[d];
            let mut by_path: Vec<(u8, i64)> = Vec::new();
            for (pi, path) in line.paths.iter().enumerate() {
                if path.stops.get(d).copied() == Some(station) {
                    let a = path.stop_arclen_mm.get(d).copied().unwrap_or(0);
                    by_path.push((pi as u8, a));
                }
            }
            if by_path.len() >= 2 {
                points.push(Pt { station, by_path });
            }
        }
        if points.is_empty() {
            continue;
        }

        // Two divergence points are COUPLED when a single consist can straddle both on SOME shared
        // service path — i.e. their arclen gap is <= len_mm on ANY path containing both. The runtime
        // mutex (vehicle.rs A.1.5/B.4) keys on PER-PATH spans, and a branch path's smoothed
        // shared-prefix arclen can be SHORTER than the trunk's (Catmull-Rom neighbour influence pulls
        // the branch straighter while the trunk bows toward its post-junction continuation). So
        // coalescing on the TRUNK gap alone under-groups: a pair coupled on a branch but not the trunk
        // stays split, and a branch consist straddling both forms the exact 2-cycle deadlock coalescing
        // exists to kill (the design's Residual Risk #2, found in review). Coalesce on the MIN gap over
        // shared paths — the tightest mutual-reach bound, matching what the mutex actually enforces.
        let coupled = |p: &Pt, q: &Pt| -> bool {
            let (mut i, mut j) = (0usize, 0usize);
            let mut min_gap = i64::MAX;
            while i < p.by_path.len() && j < q.by_path.len() {
                let (pp, pa) = p.by_path[i];
                let (qp, qa) = q.by_path[j];
                if pp == qp {
                    min_gap = min_gap.min((qa - pa).abs());
                    i += 1;
                    j += 1;
                } else if pp < qp {
                    i += 1;
                } else {
                    j += 1;
                }
            }
            min_gap <= len_mm
        };

        // (c) Coalesce adjacent points into one atomic group (chain-merge along the arclen-ascending
        //     order: a consecutive coupling check suffices because per-path arclen is monotone, so a
        //     farther point's gap is never smaller than its predecessor's). key_station = lowest
        //     member StationId (command-order-independent).
        let mut gi = 0usize;
        while gi < points.len() {
            let mut gj = gi;
            while gj + 1 < points.len() && coupled(&points[gj], &points[gj + 1]) {
                gj += 1;
            }
            let mut key_station = points[gi].station;
            let mut span_map: Vec<(u8, i64, i64)> = Vec::new(); // (path, lo, hi)
            for p in &points[gi..=gj] {
                if p.station.index() < key_station.index() {
                    key_station = p.station;
                }
                for &(pa, a) in &p.by_path {
                    match span_map.binary_search_by_key(&pa, |&(k, _, _)| k) {
                        Ok(pos) => {
                            span_map[pos].1 = span_map[pos].1.min(a);
                            span_map[pos].2 = span_map[pos].2.max(a);
                        }
                        Err(pos) => span_map.insert(pos, (pa, a, a)),
                    }
                }
            }
            junctions.push(crate::world::Junction {
                line: LineId(li as u32),
                key_station,
                span_by_path: span_map,
            });
            gi = gj + 1;
        }
    }
    world.junctions = junctions;

    let lines = &world.lines;
    let junctions = &world.junctions; // P4: read the switch clusters for the tick-0 placement snap
    let v = &mut world.vehicles;
    v.clear();

    for (li, line) in lines.iter().enumerate() {
        let total_count = line.trainset.map(|t| t.count).unwrap_or(0);
        if line.removed || total_count == 0 || line.stops.len() < 2 || line.crosses_water_surface {
            continue;
        }
        // Trains split round-robin across the line's service paths (P3): train k runs path
        // k % npaths, so a branched line alternates destinations. Each path is its own circuit with
        // its own block density cap.
        let spec = line.vehicle_spec();
        let min_gap = crate::trainset::block_gap_mm(spec.v_max_mm_s, spec.decel_mm_s2) + spec.length_mm;
        let npaths = line.paths.len().max(1);
        // P4 needs NO junction-specific dispatch cap. A branch switch is a POINT crossing occupied
        // only while a consist's length passes over it (~`length_mm` of travel, a fraction of a
        // second) — its throughput (round_trip / length_mm) dwarfs P1's per-path block density
        // (round_trip / min_gap), so the existing `max_fit` cap always binds first. And the junction
        // MUTEX is deadlock-free by construction (coalescing ⇒ one owner per cluster ⇒ an acyclic
        // depth-1 wait-for forest, unlike P2's single-track P1×P2 cycle), so over-provisioning just
        // queues trains at the gate — it never gridlocks. So the fleet is the full count, metered (not
        // capped) at the switch. (A whole-line cap of ~2 trains — what a block-sized junction cap
        // implies — would cripple every branched line; see docs/capacity-roadmap.md §4.3 residual.)

        // PASS 1 — each path's fleet (round-robin share, P1 block density, P2 per-path single cap).
        let mut counts: Vec<u16> = vec![0; line.paths.len()];
        for (pi, path) in line.paths.iter().enumerate() {
            let total = path.length_mm();
            if total <= 0 || path.stops.len() < 2 {
                continue;
            }
            // This path's round-robin share of the fleet (k in 0..total_count with k % npaths == pi).
            let count_p = ((total_count as usize).saturating_sub(pi) + npaths - 1) / npaths;
            if count_p == 0 {
                continue;
            }
            // Loop: circuit length = one-way; out-and-back: there and back.
            let round = if path.loop_line { total } else { 2 * total };
            // Block density cap (P1, docs/capacity-roadmap.md): trains must sit at least a full-speed
            // braking distance + standoff + their own length apart, so a path holds only so many.
            // Over-provisioning is self-limiting — the surplus is simply not dispatched.
            let max_fit = (round / min_gap.max(1)).max(1);
            let mut count = (count_p as i64).min(max_fit).max(1) as u16;
            // P2 single-track capacity cap: a path carrying SINGLE spans can run at most
            // (passing-places + 1) trains without an opposing-meet deadlock — a passing place is a
            // DOUBLE span (a loop where opposing trains pass); a fully-single out-and-back is a
            // one-train shuttle. Surplus stays undispatched (the same self-limiting pattern as the
            // block cap; the UI reads the clamped count back). Loops are exempt (one-way ⇒ no meets).
            if !path.loop_line {
                let single = path.track_type.iter().filter(|&&t| t == crate::line::track::SINGLE).count();
                if single > 0 {
                    let doubles = path.track_type.len().saturating_sub(single) as u16;
                    count = count.min(doubles.saturating_add(1));
                }
            }
            counts[pi] = count;
        }

        // S1v1 CROSS-PATH single-track cap (docs/capacity-roadmap.md P5). A BRANCHED line single-
        // tracked on its UNIVERSALLY-SHARED trunk prefix [0, D) (D = min diverge_at) runs at most
        // (physically-double shared spans) + 1 trains TOTAL across the trunk AND every branch path —
        // they ALL traverse the shared trunk, so the section's single-track capacity bounds the WHOLE
        // fleet, not each path independently. The per-path cap MISSES this: it dispatches 1 trunk + 1
        // branch train onto a fully-single shared trunk, which then head-on (P2 keys the meet per-path,
        // so the trunk and branch consists never mutex on the one physical rail). A fully-single shared
        // trunk ⇒ cap 1 (a shuttle; even a perfect physical mutex deadlocks 2 trains here — no passing
        // place, they desync and oppose). The budget drains in ascending path order, so the trunk wins.
        // Letting a single span BETWEEN passing places run a real cross-path MEET is the deferred S2
        // physical-block mutex. INERT unless the shared prefix has a physically-single span ⇒ zero
        // re-pins (a non-branched / fully-double / branch-private-single line is untouched).
        if !line.branches.is_empty() {
            let d_min = line
                .branches
                .iter()
                .map(|b| (b.diverge_at as usize).min(line.stops.len().saturating_sub(1)))
                .min()
                .unwrap_or(0);
            // Span k (< d_min) is traversed by every path; it is physically SINGLE iff SINGLE on ANY
            // path (whole-line edits all paths; a per-span edit touches only the trunk — single-if-any
            // is the safe read so an asymmetric edit still constrains the shared section).
            let phys_single = |k: usize| {
                line.paths.iter().any(|p| p.track_type.get(k).copied() == Some(crate::line::track::SINGLE))
            };
            if (0..d_min).any(phys_single) {
                let doubles = (0..d_min).filter(|&k| !phys_single(k)).count() as i64;
                let mut budget = doubles + 1; // single-track capacity of the shared trunk
                for c in counts.iter_mut() {
                    let take = (*c as i64).min(budget.max(0));
                    *c = take as u16;
                    budget -= take;
                }
            }
        }

        // PASS 2 — place each path's (capped) fleet.
        for (pi, path) in line.paths.iter().enumerate() {
            let count = counts[pi];
            if count == 0 {
                continue;
            }
            let total = path.length_mm();
            let round = if path.loop_line { total } else { 2 * total };
            for k in 0..count {
                let p = (round as i128 * k as i128 / count as i128) as i64; // 0..round
                let (s, dir) = if path.loop_line {
                    (p, 1i8)
                } else if p <= total {
                    (p, 1i8)
                } else {
                    (2 * total - p, -1i8)
                };
                // P2: never DISPATCH a train mid-SINGLE-block (it would start strictly inside a span,
                // which the move-phase meet protocol gates ENTRY into but cannot un-place). Snap such
                // a placement to the span's ENTRY gate (a passing place), so the head-on invariant
                // holds from tick 0. Double-track placement is untouched (P1 parity preserved).
                let s = match path.strictly_inside(s) {
                    Some(sp)
                        if !path.loop_line
                            && path.track_type.get(sp).copied().unwrap_or(0) == crate::line::track::SINGLE =>
                    {
                        if dir > 0 { path.stop_arclen_mm[sp] } else { path.stop_arclen_mm[sp + 1] }
                    }
                    _ => s,
                };
                // P4: never DISPATCH a train STRADDLING a junction cluster (its consist would start as
                // an un-arbitrated switch occupant — a tick-0 collision the mutex gates ENTRY into but
                // cannot un-place). Snap its head to the cluster's near gate (the junction station) so
                // the mutex arbitrates entry from tick 1. Coalescing ⇒ at most one cluster straddled,
                // so the (rare) snap is unambiguous. Mirrors the single-track entry-gate snap above;
                // non-branched lines have no clusters ⇒ no snap (parity).
                let len = spec.length_mm;
                let s = junctions
                    .iter()
                    .filter(|j| j.line.index() == li)
                    .find_map(|j| {
                        let &(_, lo, hi) = j.span_by_path.iter().find(|&&(p, _, _)| p == pi as u8)?;
                        let tail = s - dir as i64 * len;
                        let (a, b) = if tail <= s { (tail, s) } else { (s, tail) };
                        (b > lo && a < hi).then_some(if dir > 0 { lo } else { hi })
                    })
                    .unwrap_or(s);
                let (x, y) = path.point_at(s);
                v.line.push(LineId(li as u32));
                v.path.push(pi as u8);
                v.s_mm.push(s);
                v.prev_s_mm.push(s);
                v.dir.push(dir);
                v.x_mm.push(x);
                v.y_mm.push(y);
                v.prev_x_mm.push(x);
                v.prev_y_mm.push(y);
                v.angle.push(path.heading_at(s));
                v.v_mm_s.push(0);
                v.dwell_until_ms.push(0);
                v.onboard.push(0);
                v.onboard_pax.push(Vec::new());
                v.at_station.push(-1);
            }
        }
    }
}
