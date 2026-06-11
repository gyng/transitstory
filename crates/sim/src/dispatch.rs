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
            for &st in &line.stops {
                if st.index() < nstations {
                    serving[st.index()].push(LineId(li as u32));
                }
            }
        }
    }
    world.serving = serving;

    let lines = &world.lines;
    let v = &mut world.vehicles;
    v.clear();

    for (li, line) in lines.iter().enumerate() {
        let count = line.trainset.map(|t| t.count).unwrap_or(0);
        let total = line.length_mm();
        if line.removed || count == 0 || total <= 0 || line.stops.len() < 2 || line.crosses_water_surface {
            continue;
        }
        // Loop: circuit length = one-way; out-and-back: there and back.
        let round = if line.loop_line { total } else { 2 * total };
        // Block density cap (P1, docs/capacity-roadmap.md): trains must sit at least a full-speed
        // braking distance + standoff + their own length apart, so a line physically holds only so
        // many. Over-provisioning past this is self-limiting — the surplus is simply not dispatched
        // and the effective headway floors at the block. MAX_TRAINS_PER_LINE is now just a
        // buffer-sizing backstop; the real ceiling is geometric. Long lines are unaffected (their
        // block fits far more than the 24-train cap), only short over-subscribed ones bind.
        let spec = line.vehicle_spec();
        let min_gap = crate::trainset::block_gap_mm(spec.v_max_mm_s, spec.decel_mm_s2) + spec.length_mm;
        let max_fit = (round / min_gap.max(1)).max(1);
        let count = (count as i64).min(max_fit).max(1) as u16;
        for k in 0..count {
            let p = (round as i128 * k as i128 / count as i128) as i64; // 0..round
            let (s, dir) = if line.loop_line {
                (p, 1i8)
            } else if p <= total {
                (p, 1i8)
            } else {
                (2 * total - p, -1i8)
            };
            let (x, y) = line.point_at(s);
            v.line.push(LineId(li as u32));
            v.s_mm.push(s);
            v.prev_s_mm.push(s);
            v.dir.push(dir);
            v.x_mm.push(x);
            v.y_mm.push(y);
            v.prev_x_mm.push(x);
            v.prev_y_mm.push(y);
            v.angle.push(line.heading_at(s));
            v.v_mm_s.push(0);
            v.dwell_until_ms.push(0);
            v.onboard.push(0);
            v.onboard_pax.push(Vec::new());
            v.at_station.push(-1);
        }
    }
}
