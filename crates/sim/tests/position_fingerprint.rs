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
    fn i32(&mut self, v: i32) {
        // Reinterpret the two's-complement bit pattern as u32 (no float, no sign branch).
        self.u64(v as u32 as u64);
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

// ============================================================================================
// The K=2 BERTH-MOTION fingerprint — the TTD L4 G3 prerequisite (docs/ttd-l4-plan.md).
//
// The K=1 fingerprints above MUST NOT move through any of L4 (silent-drift tripwire). But L4d
// turns today's SOFT berth relaxation (which only ever pulls a follower FORWARD, never holds)
// into a HARD throat mutex that can DENY — neutral on the K=1 goldens, but it changes K≥2 berth
// motion. The existing `platforms.rs` K=2 tests assert liveness/relaxation qualitatively but are
// NOT fingerprint-pinned, so an L4d regression would slip through. This pins the exact K=2 berth
// motion of BOTH `bunched_line` scenarios so L4d must either hold them BYTE-NEUTRAL (proving the
// hard mutex changes no admitted motion on these scenarios) or be re-classed motion-changing with
// a deliberate, documented re-pin — never a silent K≥2 drift.
//
// Unlike the K=1 fold, this ADDITIONALLY folds `berth_idx` (the berth-allocation state L4c/L4d/L4f
// touch directly): a change in WHICH berth a consist takes — even at an identical `s_mm` — must
// move this fingerprint. `berth_idx` is deterministic scratch today (recomputed each tick); the
// fold reads it test-side, it is not (yet) hashed into `Canonical` (that graduates at L4f).
// ============================================================================================

/// The K=2 berth-motion fold: per vehicle (index order) `(line, path, dir, s_mm, berth_idx)`, then
/// `ridership_total` + per-line ridership — integer-only, index-ordered, no HashMap, no float.
fn k2_position_fingerprint(world: &World) -> u64 {
    let mut h = Fnv1a::new();
    let v = &world.vehicles;
    let n = v.line.len();
    for i in 0..n {
        h.u32(v.line[i].0);
        h.u64(v.path[i] as u64);
        h.i8(v.dir[i]);
        h.i64(v.s_mm[i]);
        h.i32(v.berth_idx[i]);
    }
    let stats = world.stats_snapshot();
    h.u64(stats.ridership_total as u64);
    for ls in &stats.per_line {
        h.u64(ls.ridership as u64);
    }
    h.finish()
}

/// Byte-for-byte the same builder as `platforms.rs::bunched_line` — a short line packed with enough
/// trains to bunch up behind a dwelling leader, with `k` berths on every station. `loop_line=true`
/// runs them all one direction (the relaxation exercise); `false` is out-and-back (opposing meets).
fn bunched_line(k: u16, count: u16, loop_line: bool) -> World {
    let mut w = World::new(7, CityData::default());
    let xs = [0_i64, 600_000, 1_200_000, 1_800_000];
    for &x in &xs {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: if loop_line { x / 3 } else { 0 }, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line, mode: 0, literal: false });
    for s in 0..xs.len() as u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    if k > 1 {
        for s in 0..xs.len() as u32 {
            w.apply(&Command::BuildPlatforms { station: StationId(s), k });
        }
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count });
    w.apply(&Command::SetRunning { running: true });
    w
}

fn run_bunched(k: u16, count: u16, loop_line: bool, ticks: usize) -> World {
    let mut w = bunched_line(k, count, loop_line);
    for _ in 0..ticks {
        w.tick(50);
    }
    w
}

/// PINNED K=2 berth-motion fingerprint — LOOP variant (`bunched_line(2, 5, true)` @ 3000 ticks). The
/// relaxation exercise: same-direction followers pulling into free berths behind a dwelling leader.
/// L4d MUST hold this byte-neutral or re-pin it deliberately (documented motion change). NOT a K=1
/// silent-drift case — it is the K≥2 regression tripwire G3 demands before L4d.
const BUNCHED_LOOP_K2_FINGERPRINT: u64 = 0x6bc6_6df8_97fa_0a3a;

/// PINNED K=2 berth-motion fingerprint — OUT-AND-BACK variant (`bunched_line(2, 6, false)` @ 3000
/// ticks): opposing meets + berths. Same re-pin discipline as the loop variant.
const BUNCHED_OUTBACK_K2_FINGERPRINT: u64 = 0x62f6_7abd_db36_a779;

#[test]
fn bunched_loop_k2_fingerprint_pinned() {
    let a = k2_position_fingerprint(&run_bunched(2, 5, true, 3000));
    let b = k2_position_fingerprint(&run_bunched(2, 5, true, 3000));
    assert_eq!(a, b, "the K=2 loop berth-motion fingerprint must be reproducible (two builds agree)");
    assert_eq!(
        a, BUNCHED_LOOP_K2_FINGERPRINT,
        "K=2 LOOP berth-motion fingerprint drifted: 0x{a:016x} != 0x{BUNCHED_LOOP_K2_FINGERPRINT:016x}. \
         This pins K=2 berth allocation + motion (vehicle line/path/dir/s_mm/berth_idx) so the L4d hard \
         throat mutex can't silently regress K≥2 behaviour. L4d must hold it neutral or re-pin it as a \
         documented motion change. If an L4 step before L4d drifts it, STOP."
    );
}

#[test]
fn bunched_outback_k2_fingerprint_pinned() {
    let a = k2_position_fingerprint(&run_bunched(2, 6, false, 3000));
    let b = k2_position_fingerprint(&run_bunched(2, 6, false, 3000));
    assert_eq!(a, b, "the K=2 out-and-back berth-motion fingerprint must be reproducible (two builds agree)");
    assert_eq!(
        a, BUNCHED_OUTBACK_K2_FINGERPRINT,
        "K=2 OUT-AND-BACK berth-motion fingerprint drifted: 0x{a:016x} != \
         0x{BUNCHED_OUTBACK_K2_FINGERPRINT:016x}. This pins K=2 berth allocation + motion across \
         opposing meets so the L4d hard throat mutex can't silently regress K≥2 behaviour. L4d must \
         hold it neutral or re-pin it as a documented motion change. If an L4 step before L4d drifts \
         it, STOP."
    );
}

// ============================================================================================
// The WITH-SIGNALS position fingerprint — the TTD L5b motion pin (docs/ttd-l5-plan.md).
//
// The K=1/K=2 fingerprints + goldens above pin the NO-SIGNALS path, which L5b must keep BYTE-
// IDENTICAL (the signal-free goldens place no signal, so the sub-block keying degenerates to the
// whole span). This NEW fingerprint pins the WITH-SIGNALS motion the relaxation introduces: a
// mostly-double out-and-back line with one long SINGLE span (span 3) carrying 3 player signals,
// over which a SAME-DIRECTION convoy follows (sub-block following). Any future change to the L5b
// admission relaxation that perturbs the admitted motion must move this — or be re-pinned with a
// documented reason. Same K=1 fold (vehicle line/path/dir/s_mm + ridership), integer-only.
// ============================================================================================

/// Byte-for-byte the same builder as `signal_blocks.rs::mostly_double_one_single(3, 8)` — a 7-stop
/// out-and-back line, span 3 single + 3 evenly-spaced signals, the rest double (passing places), a
/// demand corridor. The dispatch cap (`doubles + 1`) clamps the fleet to 6; the signal relaxation
/// raises throughput via same-direction sub-block following.
fn signalled_following_world() -> World {
    const SINGLE: u8 = 1;
    const CELL: i64 = 100_000;
    let cells: Vec<DemandCell> = (0..60)
        .map(|k| DemandCell { x_mm: k * CELL + 50_000, y_mm: 50_000, origin_w: 8.0, dest_w: 8.0, commodity: 0 })
        .collect();
    let mut w = World::new(7, CityData { grid_cell_mm: CELL, demand: DemandGrid { cell_m: 100.0, cells }, ..Default::default() });
    let xs = [0i64, 3, 6, 9, 25, 28, 31].map(|c| c * CELL + 50_000);
    for x in xs {
        w.apply(&Command::PlaceStation { x_mm: x, y_mm: 50_000, name: None });
    }
    w.apply(&Command::CreateLine { color: 1, name: None, loop_line: false, mode: 0, literal: false });
    for s in 0..7u32 {
        w.apply(&Command::AddStop { line: LineId(0), station: StationId(s), after: None });
    }
    w.apply(&Command::SetSegmentTrack { line: LineId(0), seg: TrackSegmentId(3), track: SINGLE });
    let lo = w.lines[0].paths[0].stop_arclen_mm[3];
    let hi = w.lines[0].paths[0].stop_arclen_mm[4];
    for g in 0..3u32 {
        let at = lo + (hi - lo) * (g as i64 + 1) / 4;
        w.apply(&Command::PlaceSignal { line: LineId(0), path: 0, span: 3, at_mm: at });
    }
    w.apply(&Command::AssignTrainset { line: LineId(0), spec: 0, count: 8 });
    w.apply(&Command::SetRunning { running: true });
    w
}

fn run_signalled() -> World {
    let mut w = signalled_following_world();
    for _ in 0..3000 {
        w.tick(50);
    }
    w
}

/// PINNED with-signals position fingerprint (TTD L5b motion pin): the SAME-direction sub-block
/// following motion the relaxation introduces. Computed over `signalled_following_world()` @ 3000
/// ticks. NOT a silent-drift case (it is the NEW motion L5b adds) — a future change to the admission
/// relaxation that perturbs admitted motion must move this, with a documented re-pin.
const SIGNALLED_FOLLOWING_FINGERPRINT: u64 = 0x77d0_16f8_2198_cf36;

#[test]
fn signalled_following_fingerprint_pinned() {
    let a = position_fingerprint(&run_signalled());
    let b = position_fingerprint(&run_signalled());
    assert_eq!(a, b, "the with-signals following fingerprint must be reproducible (two builds agree)");
    assert_eq!(
        a, SIGNALLED_FOLLOWING_FINGERPRINT,
        "with-signals following fingerprint drifted: 0x{a:016x} != \
         0x{SIGNALLED_FOLLOWING_FINGERPRINT:016x}. This pins the TTD L5b SAME-direction sub-block \
         following motion (vehicle line/path/dir/s_mm + ridership). The no-signals goldens/K=1 \
         fingerprints stay byte-identical; THIS pins the new motion. A change to the L5b admission \
         relaxation must move it deliberately, with a documented re-pin."
    );
    // Non-vacuous: the scenario dispatches a fleet and serves riders (the relaxation actually fires).
    let w = run_signalled();
    assert!(!w.vehicles.is_empty(), "the signalled scenario must dispatch vehicles");
    assert!(w.ridership_total > 0, "the signalled scenario must serve riders");
}
