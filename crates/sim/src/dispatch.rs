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

    let lines = &world.lines;
    let v = &mut world.vehicles;
    v.clear();

    for (li, line) in lines.iter().enumerate() {
        let count = line.trainset.map(|t| t.count).unwrap_or(0);
        let total = line.length_mm();
        if count == 0 || total <= 0 || line.stops.len() < 2 {
            continue;
        }
        let round = 2 * total; // out-and-back loop length
        for k in 0..count {
            let p = (round as i128 * k as i128 / count as i128) as i64; // 0..round
            let (s, dir) = if p <= total { (p, 1i8) } else { (2 * total - p, -1i8) };
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
            v.onboard_dest.push(Vec::new());
            v.at_station.push(-1);
        }
    }
}
