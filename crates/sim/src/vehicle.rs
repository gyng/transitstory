//! Struct-of-Arrays vehicle store + the 1-D arc-length motion integrator (trapezoidal
//! accel/cruise/brake + fixed dwell, out-and-back). Positions are integer mm. Holds
//! previous-tick AND current-tick positions so the frontend interpolates at 60fps.
use crate::ids::LineId;
use crate::trainset::spec as trainset_spec;
use crate::world::World;

#[derive(Default)]
pub struct VehicleSoA {
    pub line: Vec<LineId>,
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
pub(crate) fn advance(world: &mut World, dt_ms: i64) {
    let clock = world.clock_ms;
    let lines = &world.lines;
    let v = &mut world.vehicles;

    for i in 0..v.len() {
        v.prev_s_mm[i] = v.s_mm[i];
        v.prev_x_mm[i] = v.x_mm[i];
        v.prev_y_mm[i] = v.y_mm[i];

        let line = &lines[v.line[i].index()];
        let total = line.length_mm();
        if total <= 0 || line.arclen_mm.len() < 2 {
            continue;
        }
        if clock < v.dwell_until_ms[i] {
            v.v_mm_s[i] = 0;
            continue;
        }

        let spec = line
            .trainset
            .map(|t| trainset_spec(t.spec))
            .unwrap_or_else(|| trainset_spec(0));
        let dir = v.dir[i] as i64;
        let s = v.s_mm[i];
        // Stops sit at specific arc-lengths along the smoothed polyline.
        let stop_idx = next_stop_index(&line.stop_arclen_mm, s, dir);
        let next_arc = line.stop_arclen_mm[stop_idx];
        let dist_to_stop = (next_arc - s).abs();

        let accel_step = spec.accel_mm_s2 * dt_ms / 1000;
        let decel_step = spec.decel_mm_s2 * dt_ms / 1000;
        let vcur = v.v_mm_s[i];
        let brake_dist =
            (vcur as i128 * vcur as i128 / (2 * spec.decel_mm_s2.max(1) as i128)) as i64;

        let mut nv = if dist_to_stop <= brake_dist {
            (vcur - decel_step).max(0)
        } else {
            (vcur + accel_step).min(spec.v_max_mm_s)
        };
        if nv == 0 && dist_to_stop > 0 {
            nv = accel_step.max(1); // crawl so we always reach the stop (no stall)
        }

        let ds = nv * dt_ms / 1000;
        let mut new_s = s + dir * ds;
        let crossed = (dir > 0 && new_s >= next_arc) || (dir < 0 && new_s <= next_arc);
        if crossed {
            new_s = next_arc;
            nv = 0;
            v.dwell_until_ms[i] = clock + spec.dwell_ms;
            // Record arrival at this stop's station for the board/alight phase.
            v.at_station[i] = line.stops[stop_idx].0 as i32;
            if stop_idx + 1 >= line.stop_arclen_mm.len() {
                v.dir[i] = -1; // forward end -> reverse
            } else if stop_idx == 0 {
                v.dir[i] = 1; // back end -> reverse
            }
        }

        v.s_mm[i] = new_s;
        v.v_mm_s[i] = nv;
        let (x, y) = line.point_at(new_s);
        v.x_mm[i] = x;
        v.y_mm[i] = y;
        let h = line.heading_at(new_s);
        v.angle[i] = if dir < 0 { h + std::f32::consts::PI } else { h };
    }
}
