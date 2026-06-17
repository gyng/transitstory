//! The INTEGRATOR POSITION FINGERPRINT — the load-bearing proof for the upcoming TTD L3 C1
//! geometry-ownership flip (docs/ttd-l3-plan.md, "DE-SCOPING INSIGHT" → "the earned-re-pin
//! identity gate"). The C1 flip moves geometry ownership onto `TrackSegment` (authoritative +
//! HASHED), which LEGITIMATELY moves the golden `state_hash` — so the golden-hash test can no
//! longer prove behaviour was preserved across that commit. This test fills that gap: it pins a
//! SEPARATE FNV-1a fingerprint over the integrator's AUTHORITATIVE motion + gameplay output
//! (vehicle line/path/dir/arc-length-`s_mm` + ridership), which is INDEPENDENT of how that
//! geometry is serialized into `Canonical`. The C1 flip MUST keep this fingerprint byte-identical
//! ("only serialization moved, not positions"); the golden hash may move, this may not.
//!
//! GOLDEN-NEUTRAL: this is a NEW TEST ONLY. It changes no `src`, re-pins no golden. It rebuilds the
//! exact scenarios `determinism.rs` (transit) and `arcadia.rs` (arcadia) golden-pin, then folds the
//! authoritative integer state — index-ordered, integer-only, no HashMap iteration, no float — into
//! the same FNV-1a (offset basis 0xcbf29ce484222325, prime 0x100000001b3) the core's `state_hash`
//! uses. Render-only fields (`x_mm`/`y_mm`/`angle`) are deliberately EXCLUDED — they are derived
//! display values, not motion state.
use sim::*;

/// FNV-1a, the same construction as `sim::hash::fnv1a` / `World::state_hash` — re-implemented here so
/// the fingerprint is self-contained (no dependency on a particular `src` export surface).
struct Fnv1a(u64);

impl Fnv1a {
    #[inline]
    fn new() -> Self {
        Fnv1a(0xcbf2_9ce4_8422_2325)
    }
    #[inline]
    fn byte(&mut self, b: u8) {
        self.0 ^= b as u64;
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }
    #[inline]
    fn u64(&mut self, v: u64) {
        // Little-endian fold of the 8 bytes (deterministic, integer-only).
        for &b in &v.to_le_bytes() {
            self.byte(b);
        }
    }
    #[inline]
    fn i64(&mut self, v: i64) {
        // Reinterpret the two's-complement bit pattern as u64 (no float, no sign branch).
        self.u64(v as u64);
    }
    #[inline]
    fn u32(&mut self, v: u32) {
        self.u64(v as u64);
    }
    #[inline]
    fn i8(&mut self, v: i8) {
        self.u64(v as u8 as u64);
    }
    #[inline]
    fn finish(self) -> u64 {
        self.0
    }
}

/// The fingerprint: fold the AUTHORITATIVE integrator + gameplay output in INDEX ORDER, integer-only.
/// Per vehicle (index order over `world.vehicles`): `(line.0, path, dir, s_mm)` — the line it runs, its
/// service path, its travel direction, and its arc-length position (the integrator's authoritative
/// motion output). Then the gameplay scalars: `ridership_total` and each `per_line[i].ridership`. The
/// stats values are `f64` in the snapshot but are whole counts, so we truncate to `u64` to fold them
/// as integers (no float enters the hash) — they are cumulative ridership tallies, always integral.
fn position_fingerprint(world: &World) -> u64 {
    let mut h = Fnv1a::new();
    let v = &world.vehicles;
    let n = v.line.len();
    for i in 0..n {
        h.u32(v.line[i].0);
        h.u64(v.path[i] as u64);
        h.i8(v.dir[i]);
        h.i64(v.s_mm[i]);
    }
    let stats = world.stats_snapshot();
    h.u64(stats.ridership_total as u64);
    for ls in &stats.per_line {
        h.u64(ls.ridership as u64);
    }
    h.finish()
}

// ---- transit scenario (replicates determinism.rs::sample_log + the 600-tick dt=50 golden run) ----

/// A representative slice command log — byte-for-byte the same as `determinism.rs::sample_log()`:
/// 3 stations, one line through them, a trainset and headway, running.
fn transit_log() -> Vec<Command> {
    vec![
        Command::PlaceStation { x_mm: 0, y_mm: 0, name: None },
        Command::PlaceStation { x_mm: 5_000_000, y_mm: 0, name: None },
        Command::PlaceStation { x_mm: 10_000_000, y_mm: 2_000_000, name: Some("Marina".into()) },
        Command::CreateLine { color: 0x3366cc, name: None, loop_line: false, mode: 0, literal: false },
        Command::AddStop { line: LineId(0), station: StationId(0), after: None },
        Command::AddStop { line: LineId(0), station: StationId(1), after: None },
        Command::AddStop { line: LineId(0), station: StationId(2), after: None },
        Command::AssignTrainset { line: LineId(0), spec: 0, count: 3 },
        Command::SetHeadway { line: LineId(0), headway_ms: 240_000 },
        Command::SetRunning { running: true },
    ]
}

fn run_transit() -> World {
    let mut w = World::new(42, CityData::default());
    for c in &transit_log() {
        w.apply(c);
    }
    for _ in 0..600 {
        w.tick(50);
    }
    w
}

// ---- arcadia scenario (replicates arcadia.rs::arcadia_world + its golden run(1200)) ----

/// Byte-for-byte the same as `arcadia.rs::arcadia_world()`: a hex-grid arcadia city with a SOURCE
/// cell and a SINK cell plus a route A→B running carts.
fn arcadia_world() -> World {
    let city = CityData {
        id: "arcadia".into(),
        ruleset: "arcadia".into(),
        seed: 12345,
        grid_cell_mm: 100_000,
        demand: DemandGrid {
            cell_m: 500.0,
            cells: vec![
                DemandCell { x_mm: 0, y_mm: 0, origin_w: 50.0, dest_w: 2.0, commodity: 0 },
                DemandCell { x_mm: 1_500_000, y_mm: 0, origin_w: 2.0, dest_w: 50.0, commodity: 0 },
            ],
        },
        ..Default::default()
    };
    let mut w = World::new(7, city);
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

/// `arcadia.rs::golden_arcadia_hash_pinned` pins `run(1200).state_hash()`, so the fingerprint pins
/// the same 1200-tick (dt=50) point of the same world.
fn run_arcadia() -> World {
    let mut w = arcadia_world();
    for _ in 0..1200 {
        w.tick(50);
    }
    w
}

/// PINNED integrator-position fingerprint (TTD L3 C1 identity gate): the C1 geometry-ownership flip
/// MUST keep this BYTE-IDENTICAL (only serialization moves, not positions). If a behaviour-preserving
/// change drifts it, STOP.
const TRANSIT_POSITION_FINGERPRINT: u64 = 0xdccb_466e_60a6_e54a;

/// PINNED integrator-position fingerprint (TTD L3 C1 identity gate): the C1 geometry-ownership flip
/// MUST keep this BYTE-IDENTICAL (only serialization moves, not positions). If a behaviour-preserving
/// change drifts it, STOP.
const ARCADIA_POSITION_FINGERPRINT: u64 = 0xbf4d_cc02_ef2d_3236;

#[test]
fn transit_position_fingerprint_pinned() {
    // Reproducible: computing it over two independent builds must agree (like `replay_equality`).
    let a = position_fingerprint(&run_transit());
    let b = position_fingerprint(&run_transit());
    assert_eq!(a, b, "the transit position fingerprint must be reproducible (two builds agree)");
    assert_eq!(
        a, TRANSIT_POSITION_FINGERPRINT,
        "transit integrator-position fingerprint drifted: 0x{a:016x} != \
         0x{TRANSIT_POSITION_FINGERPRINT:016x}. This pins authoritative motion (vehicle \
         line/path/dir/s_mm) + ridership across the TTD L3 C1 flip — only serialization may move, \
         not positions. If a behaviour-preserving change drifts it, STOP."
    );
}

#[test]
fn arcadia_position_fingerprint_pinned() {
    let a = position_fingerprint(&run_arcadia());
    let b = position_fingerprint(&run_arcadia());
    assert_eq!(a, b, "the arcadia position fingerprint must be reproducible (two builds agree)");
    assert_eq!(
        a, ARCADIA_POSITION_FINGERPRINT,
        "arcadia integrator-position fingerprint drifted: 0x{a:016x} != \
         0x{ARCADIA_POSITION_FINGERPRINT:016x}. This pins authoritative motion (vehicle \
         line/path/dir/s_mm) + ridership across the TTD L3 C1 flip — only serialization may move, \
         not positions. If a behaviour-preserving change drifts it, STOP."
    );
}
