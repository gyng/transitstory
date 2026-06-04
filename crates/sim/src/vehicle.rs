//! Struct-of-Arrays vehicle store. Positions are integer mm; the motion integrator
//! (arc-length + trapezoidal profile) lands in T14. Holds previous-tick AND current-tick
//! cartesian positions so the frontend can interpolate at 60fps between sim snapshots.
use crate::ids::LineId;

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
    /// Dwell timer: vehicle is stopped boarding/alighting until this clock time.
    pub dwell_until_ms: Vec<i64>,
    /// Onboard passenger count (capacity-capped in T16b).
    pub onboard: Vec<u16>,
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
        self.dwell_until_ms.clear();
        self.onboard.clear();
    }
}
