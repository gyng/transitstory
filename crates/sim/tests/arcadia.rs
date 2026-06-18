//! S6a — the first fantasy slice in the core: the `"arcadia"` ruleset constructs and runs a
//! source→sink→cart commodity flow on the HEX lattice, deterministically, reusing the transit
//! substrate (RaptorRouter + advance + board_alight) UNCHANGED. Proves the ruleset-at-construction
//! fork lights up end-to-end. The richer supply chain (commodity ids, buffers, recipes) layers on at
//! S7 behind the same seam; this pins the foundation it builds on.
use sim::*;

/// A minimal arcadia world: a hex-grid city with a SOURCE cell (high origin weight, "ore") near node
/// A and a SINK cell (high dest weight, a "town") near node B, plus a route A→B running carts.
fn arcadia_world() -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000, // the hex lattice is live (S5)
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 50.0, dest_w: 2.0, commodity: 0 }, // source (ore)
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 50.0, commodity: 0 }, // sink (town)
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    // A node is a station, a route is a line, a cart is a trainset — the substrate, reused.
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 });
    w.apply(&Command::SetHeadway { line: LineId(0), headway_ms: 120_000 });
    w.apply(&Command::SetRunning { running: true });
    w
}

fn run(ticks: usize) -> World {
    let mut w = arcadia_world();
    for _ in 0..ticks {
        w.tick(50);
    }
    w
}

/// The `"arcadia"` tag is preserved through construction (so `replay`'s disjoint-save guard sees it,
/// and `ruleset::select` chose the fantasy boxes — the engine ran the arcadia mode end-to-end below).
#[test]
fn arcadia_world_carries_its_ruleset_tag() {
    let w = arcadia_world();
    assert_eq!(w.city.ruleset, "arcadia", "the fantasy ruleset tag survives construction");
}

/// A commodity actually flows source→sink on the arcadia ruleset (ridership accrues), AND the run is
/// bit-for-bit reproducible — the fork reuses the deterministic movement core unchanged.
#[test]
fn arcadia_commodity_flows_and_replays() {
    let w = run(4000);
    assert!(
        w.stats_snapshot().ridership_total > 0.0,
        "a commodity rides source→sink on the arcadia ruleset (reusing RaptorRouter+advance+board_alight)"
    );
    assert_eq!(run(4000).state_hash(), run(4000).state_hash(), "arcadia replays bit-for-bit");
}

/// S7a: the Forge-Line production phase fills SOURCE buffers (and only sources), deterministically.
/// Isolated from shipping by building NO line — so no station is "served", nothing ships, and the
/// produced commodity accrues visibly in the buffer (with a line it would be drained by the S7b gate).
#[test]
fn arcadia_sources_produce_into_buffers() {
    use sim::forge::{N_COMMODITIES, ORE};
    let city = CityData {
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 50.0, dest_w: 2.0, commodity: 0 }, // source
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 50.0, commodity: 0 }, // sink
            ],
        },
        ..Default::default()
    };
    let build = || {
        let mut w = World::new(7, city.clone());
        w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
        w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None });
        w.apply(&Command::SetRunning { running: true }); // running, but no line ⇒ nothing ships
        for _ in 0..2000 {
            w.tick(50);
        }
        w
    };
    let w = build();
    let src = w.forge_stock[ORE]; // station 0 = source (origin_w ≫ dest_w)
    let sink = w.forge_stock[N_COMMODITIES + ORE]; // station 1 = sink (dest_w ≫ origin_w)
    assert!(src > 0, "a source node accrues ORE into its buffer when unshipped (got {src})");
    assert_eq!(sink, 0, "a sink node produces nothing (got {sink})");
    assert_eq!(build().state_hash(), build().state_hash(), "forge production replays bit-for-bit");
}

/// S7b: shipping is GATED by production — a node ships only what its buffer holds. Shipping DEMAND
/// (from source weight) far exceeds the production rate here, so every produced unit is shipped almost
/// immediately and the source buffer stays drained near empty (the gate binds; production is the
/// throttle). Rate-coupled by design — the gate's job is to couple shipping to production.
#[test]
fn arcadia_shipping_gated_by_production() {
    use sim::forge::ORE;
    let w = run(2000);
    assert!(w.stats_snapshot().ridership_total > 0.0, "commodities still ship (the gate doesn't block all flow)");
    let src = w.forge_stock[ORE];
    assert!(
        src < 5,
        "the source buffer stays drained — shipping demand outpaces production, so the gate binds (got {src})"
    );
    assert_eq!(run(2000).state_hash(), run(2000).state_hash(), "gated shipping replays bit-for-bit");
}

/// A transit world NEVER runs `produce` (gravity/agent inherit the no-op), so the fantasy buffer state
/// stays empty — proving `forge_stock` is genuinely fantasy-only and transit's only change is the
/// one-time golden re-pin from the (empty) field appearing in `Canonical`.
#[test]
fn transit_has_no_forge_buffers() {
    let mut w = World::new(7, CityData::default());
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None });
    w.apply(&Command::PlaceStation { x_mm: 500_000, y_mm: 0, name: None });
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..500 {
        w.tick(50);
    }
    assert!(w.forge_stock.is_empty(), "transit never fills forge buffers (fantasy-only hashed state)");
}

/// S7c+S7d: the full supply loop — a commodity is produced at the source, shipped (draining the source
/// buffer), ridden to the town, DELIVERED into its buffer, and CONSUMED into global TRIBUTE (the
/// score). Tribute > 0 is the terminal proof the whole chain connected: produce→ship→deliver→consume.
#[test]
fn arcadia_commodity_loop_closes() {
    let w = run(3000);
    assert!(w.stats_snapshot().ridership_total > 0.0, "commodities ship");
    assert!(w.tribute > 0, "delivered supply is consumed into tribute — the loop closed end-to-end (got {})", w.tribute);
    assert_eq!(run(3000).state_hash(), run(3000).state_hash(), "the closed loop replays bit-for-bit");
}

/// Tribute is MONOTONIC non-decreasing (a town never un-feeds) — the supply-gauge invariant the design
/// requires (fantasy-game-design.md: a strictly-better supply network never lowers the score).
#[test]
fn arcadia_tribute_is_monotonic() {
    let mut w = arcadia_world();
    let mut prev = w.tribute;
    for _ in 0..3000 {
        w.tick(50);
        assert!(w.tribute >= prev, "tribute dropped from {prev} to {} — the supply gauge must be monotonic", w.tribute);
        prev = w.tribute;
    }
    assert!(w.tribute > 0, "the town accrued tribute over the run");
}

/// S11 split gauge — the fantasy PROGRESS coverage is MONOTONIC: extending the network to supply another
/// town never lowers it (the supply channel). The conquest channel is monotonic by construction —
/// `towns_captured` only ever rises (siege never decrements it). A superset network ⇒ score not lower.
#[test]
fn arcadia_coverage_gauge_is_monotonic_under_a_superset_network() {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 7,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 90.0, dest_w: 2.0, commodity: 0 }, // source
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 90.0, commodity: 0 }, // town A
                DemandCell { x_mm: 0, y_mm: 1_500_000, origin_w: 2.0, dest_w: 90.0, commodity: 0 }, // town B
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 0, name: None }); // source = 0
    w.apply(&Command::PlaceStation { x_mm: 1_500_000, y_mm: 0, name: None }); // town A = 1
    w.apply(&Command::PlaceStation { x_mm: 0, y_mm: 1_500_000, name: None }); // town B = 2
    // Serve town A only.
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(0), station: StationId(1), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 2 });
    w.apply(&Command::SetRunning { running: true });
    for _ in 0..200 {
        w.tick(50);
    }
    let before = w.stats_snapshot().coverage_score;
    assert!(before > 0, "supplying a town registers on the progress gauge");
    // Extend with a SECOND line that also supplies town B — a strict superset of served towns.
    w.apply(&Command::CreateLine { color: 2, name: None, loop_line: false, mode: 0, literal: false });
    w.apply(&Command::AddStop { line: LineId(1), station: StationId(0), after: None });
    w.apply(&Command::AddStop { line: LineId(1), station: StationId(2), after: None });
    w.apply(&Command::AssignTrainset { line: LineId(1), spec: 0, count: 2 });
    for _ in 0..200 {
        w.tick(50);
    }
    let after = w.stats_snapshot().coverage_score;
    assert!(after >= before, "serving another town never lowers the gauge ({before} -> {after})");
}

/// The FANTASY golden pin (separate from the transit pin): the exact `state_hash` of the arcadia
/// slice today. Guards the arcadia path against a uniform hash shift the `run()==run()` self-equality
/// can't see — the same role `GOLDEN_TRANSIT_HASH` plays for transit. Re-pinned at every arcadia
/// Canonical change (S7 buffers, S8 army SoA, S10 CA field).
// Re-pinned at S7d: towns now CONSUME delivered supply into tribute (the loop closes end-to-end).
// Prior: 0x88cd…93a5 (S6a gravity-flow), 0xe6a5…85b9 (S6b steady source→sink), 0x10d1…be61 (S7a buffers
// fill), 0xb026…4c90 (S7b production-gated shipping), 0xbdca…fd34 (S7c deposit-at-sink).
// S8: war_step + the is_barracks/bounty fields. S9: war_step now also advances `decadence` (which
// GROWS for arcadia_world — it runs but never conquers), so the arcadia state evolves further.
// Balance pass (baked-world): decadence::step now uses an exact milli-unit remainder accumulator
// (decadence_accum) instead of truncating net·dt/1000, so the fixture's idle growth is a true 50/s
// (was 40/s under truncation) ⇒ `decadence` evolves further over the 1200-tick run. Intended re-pin;
// the accumulator is excluded from Canonical and transit stays 0, so the TRANSIT golden is unchanged.
// Prior (pre-accumulator): 0x5375_1cb0_558d_3b0f.
// S10b: the spatial decadence CA — the empty `decadence_cells` slice joins Canonical (the demo arcadia
// fixture has no buildability ⇒ no CA domain ⇒ the slice stays empty; only the appended length-0 byte
// shifts the hash). Prior (pre-S10b): 0x52d2_05b0_5502_b2aa.
// S11 tech: the `tech_unlocked` u32 joins Canonical (0 — the fixture's log predates UnlockTech, so its
// behaviour is byte-identical; only the appended 4 zero bytes shift the hash). Prior: 0xbd92_54a0_7395_96de.
// S11 economy split: `mana`+`manpower` i64s join Canonical (0 — the fixture delivers only ORE, a GOLD
// commodity, so both specialised channels stay 0; only the appended zero bytes shift the hash). Prior:
// 0xb53c_aaa4_672f_5b3a.
// S11 rival: the raider SoA slices + spawn-accum/cursor/breach/breach-heal-accum join Canonical (empty/0 —
// the demo fixture has no buildability ⇒ no decadence field ⇒ no reservoir ⇒ the rival never fields;
// appended zero bytes only). Prior: 0x1757_0632_3aee_0a4a.
// S11 spell arm: the `spells_cast` u32 joins Canonical (0 — the fixture's log predates SPELLCRAFT, so the
// spell arm never casts; appended zero only). Prior: 0xbb8a_7ea7_9311_814e.
// War-batch rail-attack: the `line_disabled_until_ms` slice joins Canonical (EMPTY — the fixture has no
// reservoir ⇒ no raiders ⇒ no cut lines; the re-pin is the appended length-0 byte). Prior: 0xbdd6_84be_e6be_b78a.
// War-batch saboteur targeting: `raider_tx_mm`+`raider_ty_mm` join Canonical (EMPTY — no reservoir ⇒ no
// raiders to target; the re-pin is two appended length-0 bytes). Prior: 0x523b_1a62_1611_df7e.
// TTD L2 multi-platform: `Station.platform_count: u8` joins Canonical (= 1 for every station — the fixture
// issues no BuildPlatforms, so K=1 everywhere; behaviour-byte-identical, the re-pin is one byte per station).
// Prior: 0x8626_936b_2105_852e.
// Legions-ride-trains travel fields: the ON-LINE model's 4 ArmySoA travel fields (wait_line/wait_dir/
// riding_veh/wait_until_ms) join Canonical, EMPTY here (the demo fixture has no barracks ⇒ no legion afield ⇒
// appended length-0 slices only). Behaviour byte-identical. (Supersedes the over-broad 9-field step-1 pin
// 0xf6ff_edf4_0d41_3774.) Prior: 0xddb2_bee9_22ac_67fc.
// TTD L3 C1 — the EARNED geometry-ownership flip: geometry now lives authoritatively on the (hashed)
// TrackSegment slab + each Path's hashed `segments` binding. The arcadia fixture is a single GRID line
// (grid_cell_mm == 100_000), so its Path is BOUND to segments: its geometry (polyline/arclen/track_type/
// span_mode/min_radius/speed_cap) is OMITTED from the Path hash and the authoritative geometry is hashed in
// the segment slab instead — an earned re-pin (Path genuinely STOPS authoring the hashed geometry; the runtime
// polyline is reconstructed from the slab by `bind_path_segments`). The position fingerprint
// (tests/position_fingerprint.rs) is UNCHANGED — only SERIALIZATION moved, not the integrator's positions or
// ridership. The shared-corridor single-canonical-curve change (#36 "one track, many services") is invisible
// to this single-line, non-sharing golden. Prior: 0x98c9_2af6_50fd_babc.
// TTD L5a re-pin (0x94e8_209e_3424_d550): `Canonical` gains the player-placed `signals` store, appended
// LAST. The arcadia fixture places NO signal ⇒ a length-0 slice ⇒ a PURE EMPTY-SLICE SHIFT; the position
// fingerprint is UNCHANGED (signals don't re-key occupancy until L5b). Prior: 0xbc3c_87c3_28ba_0d70.
// 2026-06 economy ÷1000 rescale re-pin (0x58ef_2f8b_f079_1c77): the fixture's GRID line has a hashed
// `capital_cost` (Canonical.lines), built from the per-km capital constants + RAIL_COST[0] — all ÷1000.
// So capital_cost shrank ×1000 and the hash moved by DESIGN. Gold spend is UNCHANGED (build_gold_divisor
// scaled in lockstep, so build_gold_cost = capital/divisor is identical); the position fingerprint is
// UNCHANGED (capital is not a position). A VALUE change, not a determinism break. Prior: 0x94e8_209e_3424_d550.
const GOLDEN_ARCADIA_HASH: u64 = 0x58ef_2f8b_f079_1c77;

#[test]
fn golden_arcadia_hash_pinned() {
    let h = run(1200).state_hash();
    assert_eq!(
        h, GOLDEN_ARCADIA_HASH,
        "arcadia golden state_hash drifted: 0x{h:016x} != 0x{GOLDEN_ARCADIA_HASH:016x}. \
         Re-pin in a reviewed commit if this was an intentional arcadia Canonical change."
    );
}
