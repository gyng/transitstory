//! Local planar geometry in integer millimetres. NO lng/lat, NO Mercator — all
//! projection happens in the frontend's `coords/geo.ts`. Integer math keeps the
//! sim bit-reproducible.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointMm {
    pub x_mm: i64,
    pub y_mm: i64,
}

impl PointMm {
    #[inline]
    pub fn new(x_mm: i64, y_mm: i64) -> Self {
        Self { x_mm, y_mm }
    }

    /// Euclidean distance in mm, floored to an integer (i128 intermediate avoids overflow).
    #[inline]
    pub fn dist_mm(&self, other: &PointMm) -> i64 {
        let dx = (self.x_mm - other.x_mm) as i128;
        let dy = (self.y_mm - other.y_mm) as i128;
        isqrt_i128(dx * dx + dy * dy) as i64
    }
}

/// Deterministic integer square root (no floats). Newton's method on i128.
#[inline]
pub fn isqrt_i128(n: i128) -> i128 {
    if n < 2 {
        return n.max(0);
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
