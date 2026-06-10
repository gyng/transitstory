//! Time-of-day demand modulation. The sim clock maps to an in-game 24h clock; demand volume
//! follows a twin-peak rush profile and trips flip direction AM (home→work) vs PM (work→home).
//! Pure functions of the integer clock — deterministic. (Render/demand floats only.)

/// One in-game hour = this many sim-ms (2 sim-minutes). A full day = 48 sim-minutes — long enough
/// that a rush period (≈3 in-game hours ≈ 6 sim-minutes) spans MULTIPLE default headways, so
/// time-of-day demand is something the player can actually tune service against, not a flicker
/// that passes between two trains. (At 8× speed a full day is ~6 wall-minutes.) Anything paced
/// "per day" must derive from this constant — see `agents::DAY_MS` and
/// `World::agent_population_target`, which scales with day length to keep trips-per-sim-minute
/// constant.
pub const HOUR_MS: i64 = 120_000;
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

/// Time-of-day congestion penalty (%) — how much rush-hour traffic alone slows a road, before
/// local density. INTEGER step over the in-game hour (pure integer — never the f64 multiplier — so
/// it can scale HASHED vehicle motion without float drift). 0 overnight, worst at the peaks.
fn time_penalty(clock_ms: i64) -> i64 {
    let hour = (6 + clock_ms.div_euclid(HOUR_MS)).rem_euclid(24);
    match hour {
        7 | 8 | 9 | 17 | 18 | 19 => 35, // AM/PM rush — heavy
        10..=16 => 15,                  // daytime — moderate
        20 | 21 | 22 => 8,              // evening — light
        _ => 0,                         // 23 + 0..6 — clear
    }
}

/// Road congestion factor (%) on an OPEN road — time-of-day only. A bus runs at this fraction of
/// its speed when nothing's built around it.
pub fn congestion_pct(clock_ms: i64) -> i64 {
    100 - time_penalty(clock_ms)
}

/// Road congestion factor (%) at a cell, from three road-user sources:
///   • time-of-day background traffic (`congestion_pct`),
///   • structural background — denser surroundings carry more cars (`built_neighbours` = BUILT
///     cells in the 3×3 around the road),
///   • the player's OWN buses sharing the cell (`bus_traffic`) — a corridor packed with bus lines
///     jams ITSELF, so service has to spread out (the first bus is free; each extra one adds jam).
/// Floored at 50 so a fully-jammed road is never slower than going off-road.
pub fn congestion_at(clock_ms: i64, built_neighbours: i64, bus_traffic: i64) -> i64 {
    let self_pen = bus_traffic.saturating_sub(1).clamp(0, 6) * 8;
    (congestion_pct(clock_ms) - built_neighbours.clamp(0, 9) * 4 - self_pen).clamp(50, 100)
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
