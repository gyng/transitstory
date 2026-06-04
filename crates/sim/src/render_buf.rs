//! The wasm->ts state port: copy-out of the vehicle SoA into flat f32/u32 buffers in
//! local metres (render-only floats — never fed back into state-affecting math). The
//! frontend copies these into a reused typed array each frame (PLAN §0.5: copy, not a
//! long-lived zero-copy view). Empty until T14 dispatches vehicles.
use crate::world::World;

#[inline]
fn mm_to_m(v: i64) -> f32 {
    v as f32 / 1000.0
}

/// Interleaved current positions `[x0,y0, x1,y1, ...]` in metres.
pub fn vehicle_positions_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out = Vec::with_capacity(v.len() * 2);
    for i in 0..v.len() {
        out.push(mm_to_m(v.x_mm[i]));
        out.push(mm_to_m(v.y_mm[i]));
    }
    out
}

/// Interleaved previous-tick positions `[x0,y0, ...]` in metres (for alpha interpolation).
pub fn vehicle_prev_positions_m(w: &World) -> Vec<f32> {
    let v = &w.vehicles;
    let mut out = Vec::with_capacity(v.len() * 2);
    for i in 0..v.len() {
        out.push(mm_to_m(v.prev_x_mm[i]));
        out.push(mm_to_m(v.prev_y_mm[i]));
    }
    out
}

pub fn vehicle_angles(w: &World) -> Vec<f32> {
    w.vehicles.angle.clone()
}

pub fn vehicle_line_ids(w: &World) -> Vec<u32> {
    w.vehicles.line.iter().map(|l| l.0).collect()
}
