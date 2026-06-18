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
    let mut w = World::new(7, CityData { ruleset: "arcadia".into(), rival_difficulty: 1, ..Default::default() });
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
fn rival_difficulty_scales_the_muster_cadence() {
    // Synthetic difficulty check (#13): identical setup, only `rival_difficulty` differs. HARD's muster
    // cadence (45 s) fires inside a 60 s window; EASY's (180 s) does not — so the same window yields a hard
    // host but no easy one. Cadence/funding scale by difficulty; the FAIRNESS rules (undefended-only
    // re-garrison, the monotonic Standing) are unchanged and hold at EVERY difficulty (the tests above).
    let mut easy = world_with_rival((10_000_000, 0), (2_000_000, 0), false);
    easy.city.rival_difficulty = 0; // EASY — 180 s muster cadence
    let mut hard = world_with_rival((10_000_000, 0), (2_000_000, 0), false);
    hard.city.rival_difficulty = 2; // HARD — 45 s muster cadence
    run(&mut easy, 1200); // 60 s of sim
    run(&mut hard, 1200);
    assert!(hard.rival_hosts.len() >= 1, "HARD (45 s cadence) should muster a host within 60 s");
    assert_eq!(easy.rival_hosts.len(), 0, "EASY (180 s cadence) should NOT muster within 60 s");
}

#[test]
fn rival_regarrison_preserves_the_monotonic_standing() {
    // P1e keystone: the rival re-contests captured ground (raising town_value ⇒ a re-siege), but must NEVER
    // lower the cumulative Standing (`towns_captured`). That gauge is monotonic by design — a strictly-better
    // network never scores lower — so the rival can ADD work but can't make the game unwinnable by erasing
    // progress. (Combined with "only undefended towns" + a light re-garrison, the rival stays fair.)
    let mut w = world_with_rival((10_000_000, 0), (2_000_000, 0), false);
    w.towns_captured = 3; // the player has conquered 3 towns
    run(&mut w, 6000);
    assert!(w.town_value[1] > 0, "precondition: the re-garrison path was exercised");
    assert_eq!(w.towns_captured, 3, "the rival's re-garrison must NOT lower the monotonic Standing");
}

/// A world with a rival hold (faction-1 barracks) far from the capital (origin) + a build budget, so the
/// rival's track-builder (P2) has something to creep toward.
fn world_with_rival_builder(hold: (i64, i64), tribute: i64) -> World {
    let mut w = World::new(9, CityData { ruleset: "arcadia".into(), rival_difficulty: 1, ..Default::default() });
    w.stations.push(Station::new(PointMm::new(hold.0, hold.1), "Rival Hold".into()));
    w.stations[0].faction = 1;
    w.is_barracks = vec![true];
    w.rival_tribute = tribute; // the P2 build budget (capital sits at the CityData-default origin)
    w
}

#[test]
fn rival_builds_rail_toward_the_capital() {
    // P2 — the literal "the enemy builds tracks": the rival lays a faction-1 crimson line that creeps from
    // its far hold toward the player's capital (the CityData-default origin), one segment per cadence,
    // spending its build budget.
    let mut w = world_with_rival_builder((40_000_000, 0), 400);
    run(&mut w, 13000); // several build cadences (120_000 ms = 2400 ticks each)
    let rival_lines: Vec<_> = w.lines.iter().filter(|l| l.faction == 1 && !l.removed).collect();
    assert_eq!(rival_lines.len(), 1, "the rival should have laid exactly ONE rail line");
    assert!(rival_lines[0].stops.len() >= 3, "the rival line should have grown several stops (got {})", rival_lines[0].stops.len());
    let head = w.stations[rival_lines[0].stops.last().unwrap().index()].pos;
    assert!(head.x_mm < 40_000_000, "the rail-head must have advanced toward the capital (origin)");
    assert!(w.rival_tribute < 400, "building must spend rival_tribute");
    assert!(w.stations.iter().filter(|s| s.faction == 1).count() >= 3, "each extension is a new faction-1 node");
}

#[test]
fn rival_build_is_deterministic() {
    // The build (new faction-1 stations/lines + the cadence accumulator) is hashed; integer + no rng ⇒ two
    // identical runs reach an identical state_hash.
    let mut a = world_with_rival_builder((40_000_000, 0), 400);
    let mut b = world_with_rival_builder((40_000_000, 0), 400);
    run(&mut a, 8000);
    run(&mut b, 8000);
    assert_eq!(a.state_hash(), b.state_hash(), "rival track-building diverged across two identical runs");
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
