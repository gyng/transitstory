//! #13 P1d — the symmetric enemy AI's mustered HOSTS (`sim::rival`). Asserts the loop that makes the rival
//! a LEGIBLE, FAIR adversary:
//!   • muster — given a rival hold (faction-1 barracks), `rival_manpower`, and captured ground, it fields a
//!     host that marches at the nearest captured town and RE-GARRISONS it (the territory re-contest);
//!   • fairness — a captured town held inside the player's rail cordon (defended) is SPARED (the antidote to
//!     "too aggro": you can defend conquered ground by railing it);
//!   • determinism — the host SoA is hashed, integer + index-ordered + no-rng, so two identical runs reach
//!     an identical `state_hash`.
//!
//! Direct state setup (a focused unit test of `rival::step` — placing a faction-1 hold needs a baked
//! reservoir via `SeedRival`, out of scope here; the determinism INVARIANT over the full command path is
//! covered by the re-pinned goldens in determinism.rs/arcadia.rs).
use sim::city::CityData;
use sim::command::Command;
use sim::geo_local::PointMm;
use sim::ids::{LineId, StationId};
use sim::station::Station;
use sim::world::World;

/// A world with a rival hold (faction-1 barracks, id 0) and a CAPTURED town (id 1, `town_value == 0`).
/// `defended` rails a one-stop player line onto the town so the network's cordon covers it.
fn world_with_rival(hold: (i64, i64), town: (i64, i64), defended: bool) -> World {
    let mut w = World::new(7, CityData { ruleset: "arcadia".into(), ..Default::default() });
    // The rival hold — a faction-1 barracks.
    w.stations.push(Station::new(PointMm::new(hold.0, hold.1), "Rival Hold".into()));
    w.stations[0].faction = 1;
    w.is_barracks = vec![true];
    w.rival_manpower = 100;
    // A captured town (the conquest-flip signal is town_value == 0). The hold gets a non-zero value so it
    // is NOT itself mistaken for a captured town.
    w.stations.push(Station::new(PointMm::new(town.0, town.1), "Town".into()));
    w.town_value = vec![100, 0];
    if defended {
        // Rail a one-stop player line ONTO the town ⇒ it sits inside the cordon (intercepted) ⇒ safe.
        w.apply(&Command::PlaceStation { x_mm: town.0, y_mm: town.1, name: None }); // id 2 (defender)
        w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(2), after: None });
    }
    w
}

/// Run rival::step until the host has mustered + marched + resolved (or the budget lapses).
fn run(w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        sim::rival::step(w, 50);
    }
}

#[test]
fn rival_musters_and_regarrisons_an_undefended_captured_town() {
    // hold 8 km out; the captured town near the origin (undefended).
    let mut w = world_with_rival((10_000_000, 0), (2_000_000, 0), false);
    assert_eq!(w.town_value[1], 0, "precondition: the town is captured (value 0)");
    run(&mut w, 6000); // > muster cadence (1800 ticks) + the ~8 km march
    assert!(w.rival_hosts.len() >= 1, "the rival never mustered a host from its hold");
    assert!(w.town_value[1] > 0, "the rival reached the undefended captured town but did not re-garrison it");
    assert!(w.rival_manpower < 100, "mustering a host must spend rival_manpower");
}

#[test]
fn rival_spares_a_defended_captured_town() {
    // Same setup, but the captured town is railed (defended) ⇒ the host is repelled, the holding stays the
    // player's (fairness: you can defend conquered ground).
    let mut w = world_with_rival((10_000_000, 0), (2_000_000, 0), true);
    run(&mut w, 6000);
    assert_eq!(w.town_value[1], 0, "a DEFENDED captured town must NOT be re-garrisoned (railed ground is safe)");
}

#[test]
fn rival_host_state_is_deterministic() {
    // Two identical runs ⇒ identical state_hash (the host SoA + muster accumulator are hashed; the logic is
    // integer + index-ordered + rng-free).
    let mut a = world_with_rival((10_000_000, 0), (2_000_000, 0), false);
    let mut b = world_with_rival((10_000_000, 0), (2_000_000, 0), false);
    run(&mut a, 4000);
    run(&mut b, 4000);
    assert_eq!(a.state_hash(), b.state_hash(), "rival-host evolution diverged across two identical runs");
}
