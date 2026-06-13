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

    let lines = &world.lines;
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
