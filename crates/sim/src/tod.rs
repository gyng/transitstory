//! Time-of-day demand modulation. The sim clock maps to an in-game 24h clock; demand volume
//! follows a twin-peak rush profile and trips flip direction AM (home→work) vs PM (work→home).
//! Pure functions of the integer clock — deterministic. (Render/demand floats only.)

/// One in-game hour = this many sim-ms (1 sim-minute). A full day = 24 sim-minutes.
pub const HOUR_MS: i64 = 60_000;
/// The day starts at this hour (so a fresh run opens into the morning ramp).
const START_HOUR: f64 = 6.0;

/// Hour of day in [0, 24) for a given sim clock.
pub fn hour_of_day(clock_ms: i64) -> f64 {
    let h = START_HOUR + clock_ms as f64 / HOUR_MS as f64;
    h.rem_euclid(24.0)
}

fn gaussian(x: f64, mu: f64, sigma: f64) -> f64 {
    let z = (x - mu) / sigma;
    (-0.5 * z * z).exp()
}

/// Overall demand multiplier (~0.1 night … ~1.8 at the rush peaks).
pub fn demand_multiplier(hour: f64) -> f32 {
    let day = if (7.0..=20.0).contains(&hour) { 0.5 } else { 0.2 };
    let am = 1.3 * gaussian(hour, 8.0, 1.3);
    let pm = 1.2 * gaussian(hour, 18.0, 1.4);
    ((day + am + pm) as f32).clamp(0.1, 2.0)
}

/// Trip directionality in [0,1]: 1 = fully home→work (AM), 0 = fully work→home (PM), ~0.5 mid.
pub fn work_bias(hour: f64) -> f32 {
    let am = gaussian(hour, 8.0, 2.0);
    let pm = gaussian(hour, 18.0, 2.0);
    (0.5 + 0.5 * am - 0.5 * pm).clamp(0.0, 1.0) as f32
}

pub fn period_label(hour: f64) -> &'static str {
    if !(5.0..23.0).contains(&hour) {
        "Night"
    } else if hour < 9.0 {
        "AM rush"
    } else if hour < 16.0 {
        "Daytime"
    } else if hour < 19.0 {
        "PM rush"
    } else {
        "Evening"
    }
}
