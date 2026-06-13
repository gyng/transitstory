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

        // (b) Build the PHYSICAL BLOCKS to mutex (P4 switches + S2 single shared spans). Each block is
        //     a per-path arc-length WINDOW `(path, lo, hi)` (a divergence POINT has lo==hi; a single
        //     SHARED-TRUNK span has lo<hi). The runtime mutex (vehicle.rs A.1.5/B.4) keys per-path
        //     against this window, so a branch path's Catmull-Rom-drifted arclen is handled per-path
        //     (never a shared scalar). diverge_idxs is sorted and per-path arclen is monotone in
        //     trunk-stop index, so blocks sort arclen-ascending on every shared path.
        struct Blk {
            station: crate::ids::StationId, // identity (min over members → the junc_key)
            lo_trunk: i64,                  // trunk-arclen sort key (i64::MAX if not on the trunk path)
            by_path: Vec<(u8, i64, i64)>,   // (path, lo, hi), sorted by path
        }
        let trunk_clamp = line.stops.len().saturating_sub(1);
        let diverge_of = |pi: usize| -> usize {
            // path 0 = trunk (reaches every trunk stop); path pi = branch pi-1 (diverges at its stop).
            if pi == 0 { trunk_clamp } else { (line.branches[pi - 1].diverge_at as usize).min(trunk_clamp) }
        };
        let mut blocks: Vec<Blk> = Vec::new();
        // P4 — divergence-point blocks: a switch reached by >=2 paths.
        for &d in &diverge_idxs {
            let station = line.stops[d];
            let mut by_path: Vec<(u8, i64, i64)> = Vec::new();
            for (pi, path) in line.paths.iter().enumerate() {
                if path.stops.get(d).copied() == Some(station) {
                    let a = path.stop_arclen_mm.get(d).copied().unwrap_or(0);
                    by_path.push((pi as u8, a, a));
                }
            }
            if by_path.len() >= 2 {
                let lo_trunk = by_path.iter().find(|&&(p, _, _)| p == 0).map_or(i64::MAX, |&(_, lo, _)| lo);
                blocks.push(Blk { station, lo_trunk, by_path });
            }
        }
        // S2 — single-span blocks: a SHARED trunk span (>=2 traversing paths) that is physically SINGLE
        // (single on ANY traversing path — single-if-any: a per-span SetSegmentTrack edits only the
        // trunk, so the trunk being single makes the physical rail single even where a branch reads
        // double). The meet mutex then serialises the trunk + branch consists on that one physical rail.
        for k in 0..trunk_clamp {
            let trav: Vec<u8> =
                (0..line.paths.len() as u8).filter(|&pi| diverge_of(pi as usize) > k).collect();
            if trav.len() < 2 {
                continue; // a span past all branches is trunk-private → P2's per-path meet owns it
            }
            let single = trav
                .iter()
                .any(|&pi| line.paths[pi as usize].track_type.get(k).copied() == Some(crate::line::track::SINGLE));
            if !single {
                continue; // a fully-double shared span is a passing place, not a block (parity)
            }
            let mut by_path: Vec<(u8, i64, i64)> = Vec::new();
            for &pi in &trav {
                let p = &line.paths[pi as usize];
                let lo = p.stop_arclen_mm.get(k).copied().unwrap_or(0);
                let hi = p.stop_arclen_mm.get(k + 1).copied().unwrap_or(lo);
                by_path.push((pi, lo, hi));
            }
            let station = line.stops[k].min(line.stops[k + 1]);
            let lo_trunk = by_path.iter().find(|&&(p, _, _)| p == 0).map_or(i64::MAX, |&(_, lo, _)| lo);
            blocks.push(Blk { station, lo_trunk, by_path });
        }
        if blocks.is_empty() {
            continue;
        }
        blocks.sort_by_key(|b| b.lo_trunk);

        // Two blocks are COUPLED when a single consist can straddle both on SOME shared path — i.e. the
        // arclen GAP between them (`q.lo - p.hi`) is <= len_mm (within a consist-length, or <0 if they
        // overlap) on ANY path traversing both. Coalescing on the MIN gap over shared paths (the
        // tightest mutual-reach bound) merges contiguous single spans into one section AND folds a
        // single approach into its adjacent switch — so a consist bridging the single span and the
        // switch holds ONE resource (no P5×P4 wait-for cycle), exactly as P4 coalesces coupled switches.
        let coupled = |p: &Blk, q: &Blk| -> bool {
            let (mut i, mut j) = (0usize, 0usize);
            let mut min_gap = i64::MAX;
            while i < p.by_path.len() && j < q.by_path.len() {
                let (pp, _, ph) = p.by_path[i];
                let (qp, ql, _) = q.by_path[j];
                if pp == qp {
                    min_gap = min_gap.min(ql - ph);
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

        // (c) Coalesce adjacent blocks into one atomic group (chain-merge along the arclen-ascending
        //     order; a consecutive coupling check suffices because per-path arclen is monotone).
        //     key_station = lowest member StationId (command-order-independent).
        let mut gi = 0usize;
        while gi < blocks.len() {
            let mut gj = gi;
            while gj + 1 < blocks.len() && coupled(&blocks[gj], &blocks[gj + 1]) {
                gj += 1;
            }
            let mut key_station = blocks[gi].station;
            let mut span_map: Vec<(u8, i64, i64)> = Vec::new(); // (path, lo, hi)
            for b in &blocks[gi..=gj] {
                if b.station.index() < key_station.index() {
                    key_station = b.station;
                }
                for &(pa, lo, hi) in &b.by_path {
                    match span_map.binary_search_by_key(&pa, |&(k, _, _)| k) {
                        Ok(pos) => {
                            span_map[pos].1 = span_map[pos].1.min(lo);
                            span_map[pos].2 = span_map[pos].2.max(hi);
                        }
                        Err(pos) => span_map.insert(pos, (pa, lo, hi)),
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

    // --- Phase 2: cross-line shared physical-rail blocks (docs/shared-rail.md) ---------------------
    // On a GRID, two DISTINCT lines over the same single edge must mutex (Step 2). Step 1 derives the
    // shared single-edge blocks (line-independent ids) into a transient, NEVER-hashed field — inert
    // (empty) for continuous / non-grid / non-shared networks ⇒ zero re-pins.
    world.cross_blocks = derive_cross_blocks(world);

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

        // CROSS-PATH single-track cap (docs/capacity-roadmap.md P5). A BRANCHED line single-tracked on
        // its UNIVERSALLY-SHARED trunk prefix [0, D) (D = min diverge_at) runs at most (physically-
        // double shared spans) + 1 trains TOTAL across the trunk AND every branch path — they ALL
        // traverse the shared trunk, so the section's single-track capacity bounds the WHOLE fleet, not
        // each path independently (the per-path cap MISSES this and dispatches 1 trunk + 1 branch onto
        // a fully-single shared trunk). The fleet is shared ROUND-ROBIN: a fully-single shared trunk
        // (capacity 1) is a trunk-only shuttle, but a single span BETWEEN passing places (capacity >1)
        // shares its budget so the trunk AND the branch run and MEET — the S2 block mutex (the
        // single-span windows in the junction set above) serialises that meet. A trunk-takes-all drain
        // would instead starve the branch to 0. INERT unless the shared prefix has a physically-single
        // span ⇒ zero re-pins (a non-branched / fully-double / branch-private-single line is untouched).
        if !line.branches.is_empty() {
            let nspans = line.stops.len().saturating_sub(1);
            // Branch pi-1 diverges at this trunk-stop index (path 0 = trunk reaches every trunk span).
            let diverge_of = |pi: usize| -> usize {
                if pi == 0 { nspans } else { (line.branches[pi - 1].diverge_at as usize).min(nspans) }
            };
            // The shared trunk region is spans [0, d_max), d_max = the FURTHEST branch divergence (a
            // single span in the staggered region [min, max) is shared by the trunk + a late branch and
            // must be bounded too — the cap was previously scoped to [0, min) and missed it: a deadlock).
            let d_max = line
                .branches
                .iter()
                .map(|b| (b.diverge_at as usize).min(nspans))
                .max()
                .unwrap_or(0);
            // Shared span k (< d_max) is physically SINGLE iff SINGLE on ANY path that TRAVERSES it
            // (trunk + branches diverging past k); single-if-any, and ONLY traversing paths (a branch's
            // track_type[k] past its OWN divergence is a different physical rail, not shared span k).
            let phys_single = |k: usize| -> bool {
                (0..line.paths.len()).any(|pi| {
                    diverge_of(pi) > k
                        && line.paths[pi].track_type.get(k).copied() == Some(crate::line::track::SINGLE)
                })
            };
            if (0..d_max).any(phys_single) {
                // Single-track capacity = (passing places) + 1, where a passing place is a maximal RUN
                // of DOUBLE shared spans ADJACENT to a single span. Counting RUNS (not individual
                // doubles) is load-bearing: the S2 coalescing merges a contiguous single run into ONE
                // block that holds 1 train, and bunched doubles are ONE passing place — so a raw
                // double-count over-admits and the over-provisioned single block deadlocks (the meet
                // protocol cannot untangle a P1×P2 cycle once trains outnumber the passing capacity).
                let mut passes = 0usize;
                let mut max_run = 0usize; // longest contiguous phys-single run = a coalesced block length
                let mut k = 0usize;
                while k < d_max {
                    if phys_single(k) {
                        let mut j = k;
                        while j < d_max && phys_single(j) {
                            j += 1;
                        }
                        max_run = max_run.max(j - k);
                        k = j;
                    } else {
                        let mut j = k;
                        while j < d_max && !phys_single(j) {
                            j += 1;
                        }
                        if (k > 0 && phys_single(k - 1)) || (j < d_max && phys_single(j)) {
                            passes += 1; // a double run touching a single section is a passing place
                        }
                        k = j;
                    }
                }
                // FAIRNESS (S2 review): a coalesced single RUN of >=2 spans (an interior station) holds a
                // train long enough that the block's lowest-index try_claim lets >=2 lower-index (trunk)
                // trains monopolise it and STARVE a higher-index branch consist forever (deadlock-free
                // but not starvation-free). Conservatively cap such a region to 2 trains so the trunk +
                // branch ALTERNATE fairly on the block. A region of only single-SPAN blocks does not
                // starve (short occupancy leaves admission windows) and keeps the full passing-place
                // capacity. (A fairness/aging tiebreak restoring the higher multi-span capacity is a
                // logged follow-up — docs/p5-shared-track-roadmap.md.)
                // No passing place at all ⇒ a one-train shuttle (no way to meet), regardless of run
                // length. Else a >=2-span run caps to 2 (fair alternation); single-span blocks keep the
                // full passing-place capacity.
                let mut budget = if passes == 0 {
                    1
                } else if max_run >= 2 {
                    2
                } else {
                    passes as i64 + 1
                };
                let pass1 = counts.clone();
                for c in counts.iter_mut() {
                    *c = 0;
                }
                // Hand out the budget one train at a time, cycling paths in index order (deterministic),
                // never exceeding a path's PASS-1 demand — a fair share so neither the trunk nor a
                // branch is starved while capacity remains.
                let mut progress = true;
                while budget > 0 && progress {
                    progress = false;
                    for pi in 0..counts.len() {
                        if budget > 0 && (counts[pi] as i64) < (pass1[pi] as i64) {
                            counts[pi] += 1;
                            budget -= 1;
                            progress = true;
                        }
                    }
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

/// Derive the cross-line shared physical-rail blocks (Phase 2 Step 1, docs/shared-rail.md). GRID lines
/// only. A block = a maximal node-connected run of physically-SINGLE grid edges traversed by **>=2
/// distinct lines**, keyed line-independently. Pure integer; sorted-Vec grouping (no HashMap
/// iteration); command-order-independent (block ids in min-edge order). Empty (inert) unless a grid
/// network actually shares a single edge ⇒ continuous / non-grid / non-shared networks are unchanged.
fn derive_cross_blocks(world: &World) -> Vec<crate::world::CrossBlock> {
    let cell = world.city.grid_cell_mm;
    if cell <= 0 {
        return Vec::new();
    }
    type Node = (i64, i64);
    type Edge = (Node, Node);
    let node_of = |p: &crate::geo_local::PointMm| -> Node { (p.x_mm.div_euclid(cell), p.y_mm.div_euclid(cell)) };

    // 1. Every grid edge-use: which (line,path) traverses which physical edge, single?, arclen window.
    struct Use {
        edge: Edge,
        line: u32,
        path: u8,
        vi: usize,
        single: bool,
        lo: i64,
        hi: i64,
    }
    let mut uses: Vec<Use> = Vec::new();
    for (li, line) in world.lines.iter().enumerate() {
        if line.removed || line.trainset.is_none() || line.stops.len() < 2 || line.crosses_water_surface {
            continue;
        }
        for (pi, path) in line.paths.iter().enumerate() {
            let poly = &path.polyline;
            for i in 0..poly.len().saturating_sub(1) {
                let a = node_of(&poly[i]);
                let b = node_of(&poly[i + 1]);
                if a == b {
                    continue; // zero-length (same-cell) edge
                }
                let edge = if a <= b { (a, b) } else { (b, a) };
                let lo = path.arclen_mm[i];
                let hi = path.arclen_mm[i + 1];
                let span = path.span_of((lo + hi) / 2);
                let single = path.track_type.get(span).copied().unwrap_or(0) == crate::line::track::SINGLE;
                uses.push(Use { edge, line: li as u32, path: pi as u8, vi: i, single, lo, hi });
            }
        }
    }
    if uses.is_empty() {
        return Vec::new();
    }

    // 2. Group by edge → BLOCK edge (>=2 distinct lines AND single-on-any) vs PASSING edge (shared,
    //    fully double). Sorted iteration ⇒ deterministic, no HashMap.
    let mut idx: Vec<usize> = (0..uses.len()).collect();
    idx.sort_by(|&a, &b| uses[a].edge.cmp(&uses[b].edge));
    let mut block_edges: Vec<Edge> = Vec::new();
    let mut passing_edges: Vec<Edge> = Vec::new();
    let mut g = 0;
    while g < idx.len() {
        let edge = uses[idx[g]].edge;
        let mut h = g;
        let mut lines_seen: Vec<u32> = Vec::new();
        let mut any_single = false;
        while h < idx.len() && uses[idx[h]].edge == edge {
            let u = &uses[idx[h]];
            if !lines_seen.contains(&u.line) {
                lines_seen.push(u.line);
            }
            any_single |= u.single;
            h += 1;
        }
        if lines_seen.len() >= 2 {
            if any_single {
                block_edges.push(edge);
            } else {
                passing_edges.push(edge);
            }
        }
        g = h;
    }
    if block_edges.is_empty() {
        return Vec::new();
    }
    block_edges.sort();

    // 3. Union-find: coalesce block edges sharing a NODE into components.
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let m = block_edges.len();
    let mut parent: Vec<usize> = (0..m).collect();
    let mut inc: Vec<(Node, usize)> = Vec::with_capacity(2 * m);
    for (ei, &(a, b)) in block_edges.iter().enumerate() {
        inc.push((a, ei));
        inc.push((b, ei));
    }
    inc.sort();
    let mut k = 0;
    while k < inc.len() {
        let node = inc[k].0;
        let base = inc[k].1;
        let mut kk = k + 1;
        while kk < inc.len() && inc[kk].0 == node {
            let (r1, r2) = (find(&mut parent, base), find(&mut parent, inc[kk].1));
            if r1 != r2 {
                parent[r1] = r2;
            }
            kk += 1;
        }
        k = kk;
    }

    // 4. edge → component root (sorted lookup); roots in canonical order for stable block ids.
    let edge_comp: Vec<(Edge, usize)> = (0..m).map(|ei| (block_edges[ei], find(&mut parent, ei))).collect();
    let mut roots: Vec<usize> = (0..m).map(|ei| find(&mut parent, ei)).collect();
    roots.sort_unstable();
    roots.dedup();

    // 5. Per component → a CrossBlock: cyclic? + passing places + per-(line,path) traversal windows.
    let mut blocks: Vec<crate::world::CrossBlock> = Vec::new();
    for (bid, &root) in roots.iter().enumerate() {
        let mut comp_nodes: Vec<Node> = Vec::new();
        let mut comp_edge_count = 0usize;
        for ei in 0..m {
            if find(&mut parent, ei) == root {
                comp_edge_count += 1;
                comp_nodes.push(block_edges[ei].0);
                comp_nodes.push(block_edges[ei].1);
            }
        }
        comp_nodes.sort_unstable();
        comp_nodes.dedup();
        // Connected component with edges >= nodes contains a cycle (a ring shared by lines).
        let cyclic = comp_edge_count >= comp_nodes.len();
        let passing_places = passing_edges
            .iter()
            .filter(|&&(a, b)| comp_nodes.binary_search(&a).is_ok() || comp_nodes.binary_search(&b).is_ok())
            .count() as u32;

        // Per-(line,path) windows: this component's uses, grouped by lane, split by contiguous-vi runs
        // (a lane that revisits the block gets multiple windows).
        let mut comp_uses: Vec<&Use> = uses
            .iter()
            .filter(|u| edge_comp.binary_search_by(|x| x.0.cmp(&u.edge)).map(|p| edge_comp[p].1 == root).unwrap_or(false))
            .collect();
        comp_uses.sort_by(|a, b| (a.line, a.path, a.vi).cmp(&(b.line, b.path, b.vi)));
        let mut by_lane: Vec<(u32, u8, i64, i64)> = Vec::new();
        let mut q = 0;
        while q < comp_uses.len() {
            let (line, path) = (comp_uses[q].line, comp_uses[q].path);
            let lo = comp_uses[q].lo;
            let mut hi = comp_uses[q].hi;
            let mut last_vi = comp_uses[q].vi;
            let mut r = q + 1;
            while r < comp_uses.len()
                && comp_uses[r].line == line
                && comp_uses[r].path == path
                && comp_uses[r].vi == last_vi + 1
            {
                hi = comp_uses[r].hi;
                last_vi = comp_uses[r].vi;
                r += 1;
            }
            by_lane.push((line, path, lo, hi));
            q = r;
        }
        blocks.push(crate::world::CrossBlock { block_id: bid as u64, cyclic, passing_places, by_lane });
    }
    blocks
}
