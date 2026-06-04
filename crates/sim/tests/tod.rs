//! Time-of-day profile: rush peaks exceed off-peak; AM trips are work-bound, PM home-bound.
use sim::tod::*;

#[test]
fn rush_peaks_exceed_offpeak() {
    assert!(demand_multiplier(8.0) > demand_multiplier(3.0), "AM rush > night");
    assert!(demand_multiplier(18.0) > demand_multiplier(12.0), "PM rush > midday");
    assert!(demand_multiplier(3.0) < 0.4, "night is quiet");
}

#[test]
fn am_work_bound_pm_home_bound() {
    assert!(work_bias(8.0) > 0.8, "morning is home->work");
    assert!(work_bias(18.0) < 0.2, "evening is work->home");
    let mid = work_bias(12.5);
    assert!(mid > 0.3 && mid < 0.7, "midday is mixed");
}

#[test]
fn clock_offsets_and_wraps() {
    assert!((hour_of_day(0) - 6.0).abs() < 1e-9, "day opens at 06:00");
    assert!((hour_of_day(HOUR_MS * 20) - 2.0).abs() < 1e-6, "26:00 wraps to 02:00");
}
