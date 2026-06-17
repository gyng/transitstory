//! #legion-ride-trains — legions ride REAL, capacity-contended trains (no more free slide) and choose
//! walk-vs-wait from a single-line ETA. The ON-LINE model: a legion always lives on its line's arc-length
//! `s_mm`; WALKING trudges the corridor on foot, RIDING mirrors a boarded vehicle's `s_mm`. These tests pin:
//! a legion boards and rides a train; while riding its position is a pure mirror (never an independent
//! slide); a legion too big for the stock walks; a slow/infrequent service loses to walking; and the whole
//! thing replays bit-for-bit. Native cargo (no wasm) per the testing conventions.
use sim::*;

/// An arcadia war world on a line of length `~dist_mm`, running `count` trains of rail `spec` at `headway_ms`,
/// with a barracks at the near end (st0) and the target town at the far end (st1). Manpower is granted so a
/// legion fields immediately (this suite is about TRAVEL, not the supply economy that funds launches).
fn ride_world(dist_mm: i64, headway_ms: i64, spec: u8, count: u16) -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 40.0, dest_w: 2.0, commodity: 1 }, // barracks supply source
                DemandCell { x_mm: dist_mm, y_mm: 0, origin_w: 2.0, dest_w: 40.0, commodity: 1 }, // far town
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceBarracks { x_mm: 0, y_mm: 0, name: None }); // st0
    w.apply(&Command::PlaceStation { x_mm: dist_mm, y_mm: 0, name: None }); // st1
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec, count });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms });
    w.apply(&Command::SetRunning { running: true });
    // Field legions straight away (24 manpower ⇒ a few LAUNCH_COST=8 legions) without waiting on the economy.
    w.manpower = 24;
    w
}

/// THE feature: a legion boards a real train and RIDES it — and while riding its arc-length is a PURE MIRROR
/// of the carrying vehicle (never an independent free slide). Heavy stock (cap 15 ≥ the legion's strength 8)
/// on a long line with frequent service makes rail clearly beat walking, so the legion waits, boards, rides.
#[test]
fn legion_rides_a_real_train() {
    // 30 km line, 60 s headway, Heavy rail (spec 1, cap 15), 3 trains.
    let mut w = ride_world(30_000_000, 60_000, 1, 3);
    let mut ever_rode = false;
    for _ in 0..8000 {
        w.tick(50);
        for i in 0..w.armies.len() {
            if w.armies.state[i] == sim::army::RIDING {
                ever_rode = true;
                let rv = w.armies.riding_veh[i];
                assert!(rv >= 0 && (rv as usize) < w.vehicles.len(), "a RIDING legion references a live vehicle slot");
                let rv = rv as usize;
                // It rides ITS line+path, and its position is the carrying vehicle's — mirrored, not integrated.
                assert_eq!(w.vehicles.line[rv], w.armies.line[i], "rides a vehicle on its own line");
                assert_eq!(
                    w.armies.s_mm[i], w.vehicles.s_mm[rv],
                    "RIDING arc-length MIRRORS the carrying vehicle exactly (no free slide)"
                );
            }
        }
    }
    assert!(ever_rode, "a legion should board and ride a real train on a long, frequently-served line");
}

/// Walk-vs-wait, the CAPACITY branch: the default metro (cap 7) cannot seat a strength-8 legion (1 seat per
/// strength), so rail is never an option however fast — the legion WALKS the corridor and never rides.
#[test]
fn legion_too_strong_for_the_stock_walks() {
    // Same long, frequent line — but the default metro (spec 0, cap 7) can't fit the legion.
    let mut w = ride_world(30_000_000, 60_000, 0, 3);
    let mut walked = false;
    let mut rode = false;
    for _ in 0..6000 {
        w.tick(50);
        for i in 0..w.armies.len() {
            match w.armies.state[i] {
                sim::army::WALKING => walked = true,
                sim::army::RIDING => rode = true,
                _ => {}
            }
        }
    }
    assert!(walked, "a legion too big for the metro (8 > cap 7) walks");
    assert!(!rode, "and it never boards a train it cannot fit");
}

/// Walk-vs-wait, the ETA branch: even when capacity fits (Heavy, cap 15), a SHORT hop behind an INFREQUENT
/// service (240 s headway) loses to walking — `headway/2` alone (120 s) dwarfs the ~36 s on-foot crossing — so
/// the legion walks. Proves the decision is a real time estimate, not just a capacity gate.
#[test]
fn legion_walks_a_short_hop_behind_infrequent_service() {
    // 1.5 km line, 240 s headway, Heavy rail (cap 15 — capacity is NOT the blocker here).
    let mut w = ride_world(1_500_000, 240_000, 1, 2);
    let mut walked = false;
    let mut rode = false;
    for _ in 0..4000 {
        w.tick(50);
        for i in 0..w.armies.len() {
            match w.armies.state[i] {
                sim::army::WALKING => walked = true,
                sim::army::RIDING => rode = true,
                _ => {}
            }
        }
    }
    assert!(walked, "a short hop behind a 240 s headway is faster on foot ⇒ the legion walks");
    assert!(!rode, "rail loses the ETA, so the legion never waits to ride");
}

/// A riding legion still reaches its target and besieges it — the ride is a means to the conquest, not a
/// detour. Over the run the far town's resistance is ground down (it falls), proving ride→alight→siege closes.
#[test]
fn a_ridden_legion_still_conquers_its_town() {
    let mut w = ride_world(30_000_000, 60_000, 1, 3);
    for _ in 0..16000 {
        w.tick(50);
    }
    assert!(w.armies.len() >= 1, "a legion was fielded");
    // The far town (st1) is the default target; riding legions arrive, besiege, and flip it.
    assert_eq!(w.town_value[1], 0, "the target town fell to legions that rode the rail to reach it");
    assert_eq!(w.towns_captured, 1, "captured exactly once (the gate-blind guard holds under the ride path)");
}

/// Determinism: the whole ride/walk/board machine — RAPTOR-free single-line ETA, capacity contention, the
/// `s_mm` mirror — replays bit-for-bit (same seed + log + ticks ⇒ identical `state_hash`), twice in-process.
#[test]
fn legion_travel_replays_bit_for_bit() {
    let run = || {
        let mut w = ride_world(30_000_000, 60_000, 1, 3);
        for _ in 0..8000 {
            w.tick(50);
        }
        w.state_hash()
    };
    assert_eq!(run(), run(), "the legions-ride-trains machine is deterministic");
}
