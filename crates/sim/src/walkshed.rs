//! Network-walkshed catchment geometry: how far a rider really has to walk from a station to a
//! point, accounting for pedestrian barriers in the buildability raster. A station's catchment is
//! a *walk shed*, not a crow-flies disc — water severs it (you can't walk across a river), and a
//! major road / rail corridor crossing it costs extra (you detour to a crossing). We model that
//! with cheap, deterministic **line-of-sight sampling** over the raster (integer steps + `classify`
//! lookups — no float positions, no map iteration), so the pure-geometry (no-grid) case stays
//! bit-identical to the old Euclidean shed. Used by demand capture (`demand::prepare`) and the
//! visual shed query (`World::station_walkshed`); the cost policy lives here and nowhere else.
use crate::city::class;
use crate::geo_local::PointMm;
use rustc_hash::FxHashMap;

/// Effective-distance multiplier for the share of a sightline that runs through a ROAD/RAIL
/// corridor: a path entirely along a motorway crosses as if it were `1 + this` times as long, so a
/// single corridor crossing adds a modest penalty and a path that hugs one is heavily discounted.
const ROAD_CROSS_MULT: f64 = 3.0;

/// Class of the buildability cell containing `(x_mm, y_mm)` — OPEN outside the grid (matches
/// `World::classify`, kept here as a free fn so the sampler borrows only the lookup, not all of
/// `World`, and so demand-prepare can hold its other field borrows alongside).
#[inline]
fn class_at(lookup: &FxHashMap<(i32, i32), u8>, cell_mm: i64, x_mm: i64, y_mm: i64) -> u8 {
    let key = (x_mm.div_euclid(cell_mm) as i32, y_mm.div_euclid(cell_mm) as i32);
    lookup.get(&key).copied().unwrap_or(class::OPEN)
}

/// The effective walking distance (mm) from `from` to `to` once pedestrian barriers are accounted
/// for, or `None` when the point is unreachable on foot — either the straight Euclidean distance
/// already exceeds the budget `r`, the sightline (or the destination itself) hits impassable WATER,
/// or a ROAD/RAIL crossing penalty pushes the effective distance past `r`. Sampled at the raster
/// resolution with integer interpolation → fully deterministic. With an empty grid every sample is
/// OPEN, so this returns `Some(euclidean)` and the catchment collapses to the old crow-flies disc.
pub(crate) fn effective_walk_dist(
    lookup: &FxHashMap<(i32, i32), u8>,
    cell_mm: i64,
    from: PointMm,
    to: PointMm,
    r: f64,
) -> Option<f64> {
    let dx = (to.x_mm - from.x_mm) as f64;
    let dy = (to.y_mm - from.y_mm) as f64;
    let d = (dx * dx + dy * dy).sqrt();
    if d > r {
        return None; // out of the nominal walk budget before any detour
    }
    if cell_mm <= 0 {
        return Some(d); // no usable raster → pure Euclidean (degenerate guard)
    }
    // Sample the segment finely enough to catch a one-cell-wide barrier strip (half a cell step).
    let step = (cell_mm / 2).max(1);
    let n = ((d / step as f64).ceil() as i64).clamp(2, 64);
    let mut road_samples = 0i64;
    // k in 1..=n includes the destination (k=n) so a point ON water severs; excludes the origin
    // cell (k=0) so a station next to a road never penalises its own platform.
    for k in 1..=n {
        let sx = from.x_mm + (to.x_mm - from.x_mm) * k / n;
        let sy = from.y_mm + (to.y_mm - from.y_mm) * k / n;
        match class_at(lookup, cell_mm, sx, sy) {
            class::WATER => return None, // impassable — the far bank is a different walk shed
            class::ROAD | class::RAIL => road_samples += 1,
            _ => {}
        }
    }
    let frac = road_samples as f64 / n as f64;
    let eff = d * (1.0 + ROAD_CROSS_MULT * frac);
    if eff > r {
        return None; // a corridor crossing pushed it past the budget → drops out of the shed
    }
    Some(eff)
}
