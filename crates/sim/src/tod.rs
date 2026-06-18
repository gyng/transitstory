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
/// In-game clock seconds per sim-second = 3_600_000 / HOUR_MS. THE frame constant: since the
/// clock-unification pass, every physics constant in the sim (vehicle speeds, dwells, headways,
/// patience, walk speeds, travel-time decays) is expressed so that durations READ TRUE against
/// the in-game clock — "a 4-minute headway" means 4 minutes on the clock the player watches,
/// stored as 4 × 60 × 1000 / CLOCK_SCALE = 8_000 sim-ms. Speeds carry ×CLOCK_SCALE (a "80 km/h"
/// train covers 80 km per CLOCK hour), accelerations ×CLOCK_SCALE² (so braking distances stay
/// physical: v²/2a is frame-invariant). Capacities were rescaled ÷CLOCK_SCALE alongside so
/// per-trip loads, queues, fares/day, and opex trajectories are unchanged. One deliberate
/// exception: the globe's AIR mode keeps its story-scaled speeds ("a hop is near-instant") —
/// its gate turnarounds happen to read plausibly in clock terms anyway.
pub const CLOCK_SCALE: i64 = 3_600_000 / HOUR_MS;
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

/// In-game hour [0,24) as a pure INTEGER — the HASHABLE form (never the f64 `hour_of_day`, which is for
/// render/demand floats only). The one place clock→hour lives for integer (state-affecting) callers.
pub fn hour_int(clock_ms: i64) -> i64 {
    (6 + clock_ms.div_euclid(HOUR_MS)).rem_euclid(24)
}

/// Is it DAYLIGHT (06:00–20:00) vs night? Pure integer ⇒ safe to gate HASHED legion movement
/// (`army_travel_step`) without f64 drift: a foot-marching legion advances by day and makes CAMP at
/// night (rail-borne legions ride on — your rail is the 24/7 logistics; only the overland march rests).
pub fn is_daylight(clock_ms: i64) -> bool {
    (6..20).contains(&hour_int(clock_ms))
}

/// Time-of-day congestion penalty (%) — how much rush-hour traffic alone slows a road, before
/// local density. INTEGER step over the in-game hour (pure integer — never the f64 multiplier — so
/// it can scale HASHED vehicle motion without float drift). 0 overnight, worst at the peaks.
fn time_penalty(clock_ms: i64) -> i64 {
    let hour = hour_int(clock_ms);
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
