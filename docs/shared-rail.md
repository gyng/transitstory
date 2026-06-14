# Shared physical rail — GRID cross-line layer (design)

> **✅ BUILT 2026-06-14 (Phase 1 grid geometry + Phase 2 cross-line mutex).** This is the design
> rationale companion to the canonical status doc
> [p5-shared-track-roadmap.md](p5-shared-track-roadmap.md) (the source of truth for what shipped) and
> the A2 fork of [fantasy-fork.md](fantasy-fork.md) §10. Produced by an understand → dual-liveness
> design → adversarial review → synthesis workflow; reconciled into the canonical roadmap.
>
> **The KEY CORRECTION this contributed — now landed in code + roadmap:** the original plan's
> cross-line liveness was *"resource-ordering (acquire shared segments in global segment-id order)."*
> The adversarial review **broke** it — opposing trains acquire the same segments in opposite
> physical-traversal order, so the total order constrains nothing (see §4). The shipped guarantee is
> **atomic whole-block reservation to a passing place + a cross-LINE capacity cap + a global mutex on
> cyclic shared components + a fair tiebreak** (the cap currently ships **conservative** — 1 train per
> line globally; a per-block capacity + aging tiebreak is the logged follow-up, a fantasy-fork
> prerequisite). Obeys every [AGENTS.md](../AGENTS.md) non-negotiable.
>
> **Owner decision:** ship **L1 (system-guaranteed deadlock-freedom) + LITE (fixed paths)** as the
> foundation; design **L2 (player signals) + FULL (YAPF pathfinding)** as opt-in advanced-mode seams
> (§9), never half-built.

## 0. The verb — and what's actually unbuilt

Shared physical rail = **two distinct lines run over the same physical track and contend for it**
(safety: no two consists in one block; capacity: a shared single section meters throughput). The
grid makes physical identity *exact*: a track segment is a tile-edge, and two lines over the same
edge produce a byte-identical canonical id (integer node equality — no Catmull-Rom fuzz).

What is **already green** (don't rebuild): the *same-line* shared-trunk meet (S2 within one line) —
`dispatch.rs` coalesces single shared-trunk spans into the line-owned `Junction` set; the
`single_span_between_passing_places_runs_a_meet` test passes. **The genuinely unbuilt verb is
CROSS-LINE**, because every occupancy key leads with the line id — `seg_key` packs `line<<40`,
`junc_key` packs `line<<32` (`vehicle.rs`) — so two distinct `LineId`s on one physical edge get
distinct keys and **pass through each other** (the `#[ignore]`d `shared_trunk_..._is_p5` test proves
it). P5/S2 = the **cross-line physical-block mutex + its liveness**.

## 1. Scope: LITE (fixed paths), justified not compromised

Vehicles have **no route choice** — `v.s_mm` is a 1-D scalar along a fixed `Line.paths[pi]`
polyline; `advance()` never searches. Passenger routing is purely **frequency-based and
track-ownership-blind** (`routing/raptor.rs`: wait = headway/2 × npaths, ride = `stop_arclen` /
speed; no timetable, no live positions). So shared rail changes nothing about *who rides* — it is
purely an **occupancy/capacity** concern on the move integrator. Therefore it does **not** force
vehicle pathfinding: it is a bounded extension of the authority layer (one new key family + one gate
phase + one cross-line dispatch cap).

**Honest framing:** LITE delivers shared-track **safety + capacity** — "lay shared track; the
section's capacity is a real constraint you engineer around in Build (add a passing place /
double-track)." It does **not** deliver the runtime routing-choice / signal-mastery puzzle (that is
FULL/L2, §9). Do not market LITE as "signals." It is forward-compatible: the same canonical edge ids
feed a future YAPF without rework.

## 2. The TrackGraph model (derived, never persisted)

No new struct in `Canonical`. The graph is **derived** each tick, exactly like `World.junctions` /
`World.serving` are rebuilt on `dispatch_dirty` and excluded from `state_hash`.

- **Node** = a lattice cell `(x_mm.div_euclid(cell), y_mm.div_euclid(cell))` — the *same quantizer*
  `roadnav.rs::cell_of` and `world.rs` `build_lookup` already use, so the track grid is
  co-registered with the `BuildabilityGrid` for free (one `i64`-mm coordinate system).
- **`edge_key(a, b)`** = sort the node pair `(lo, hi) = (min, max)`, **zigzag-encode** each `i32`
  coord (handles west/north-of-origin negatives), pack into a `u64`. Injective for |coord| ≲ ±3000 km
  at 100 m cells; order-independent; pure integer. **Forbid the XOR/rotate fold** — it aliases
  distinct edges. Two paths over one rail emit identical keys by exact integer node equality.
- **Passing place** = a lattice node that is a **double-track boundary common to every traverser**
  (a node where each line on that run has a double span or a stop). A property of the *track*, not of
  any one line's stop list — this is the adversary's "asymmetric passing place" fix.
- **Block** = a maximal contiguous run of physically-**single** shared edges between two passing
  places, honored identically by every traversing line. Block id = the **min `edge_key`** in the run
  (command-order-independent, mirroring `dispatch.rs`'s min-`StationId` junction key). A double shared
  edge is a passing place, never a block (parity with the existing single/double rule).

For LITE no adjacency list is materialized — per vehicle per tick we only need the set of blocks its
consist `[tail, head]` overlaps, derived by walking its path's polyline segments in index order,
reusing `group_overlap`.

## 3. Re-key + authority integration

The `min()`-of-clamps architecture in `vehicle.rs::advance` is preserved exactly — `desired_ds[i]`
is only ever shrunk; Phase C stays the sole commit. Cross-line adds the **4th `min()` term**:

```
authorized_s = min( leader_gap            [P1, Phase A.2],
                    single_track_limit    [P2, Phase B],
                    junction_conflict_limit [P4, Phase B.4],
                    shared_block_limit    [P5 CROSS-LINE, NEW Phase B.6] )
```

The `occ_claim` / `occ_owner` / `try_claim` / `group_overlap` machinery is **reused verbatim** — the
only change is the key goes from line-leading `seg_key`/`junc_key` to the **line-independent**
`edge_key`/block id, so two lines land in the *same* `phys_occ` row and contend.

- **NEW Phase A.1.7** (after A.1.5, before A.2): build `phys_occ: Vec<(u64 block_id, u32 owner)>` in
  strict index order. For each consist overlapping a shared single block on its own path window,
  `occ_claim(&mut phys_occ, block_id, i)`. `track_type` is **read live** here (see §5). Gated inert
  by `grid flag AND shared_blocks_present`.
- **NEW Phase B.6** (after B.4, before B.5): the **atomic whole-block** cross-line meet gate. A
  consist may cross into a shared single block it doesn't own only if it can reserve the entire run
  to the next common passing place all-or-nothing — `admit = match occ_owner(&phys_occ, block_id)
  { Some(o) if o==i => true; Some(_) => false /*HOLD*/; None => try_claim(...) }`. On `!admit`, clamp
  `desired_ds` to the block's near gate via the existing `room2` idiom. Re-clamping a smaller `ds` is
  idempotent, so B.6 cannot perturb P1/P2/P4.
- **Phase B.5 generalized**: "no consist comes to rest strictly inside a shared single block it
  doesn't own" (clamp back to the entry gate) — the forest-of-advancing-roots property extended
  cross-line.
- **Dispatch PASS 2** gains one snap: never dispatch a consist straddling a shared block it would
  un-arbitrably occupy (mirrors the existing single-track / junction placement snaps), so safety
  holds from tick 0.

## 4. Liveness — deadlock-free by construction (the corrected mechanism)

> **The naive proof was wrong.** "Acquire edges in canonical-id order ⇒ no cyclic wait-for" is
> **invalid**: opposing trains traverse the same edges in *opposite* edge-key order, so a total order
> constrains nothing. The adversary also found a ring shared by two lines deadlocks the depth-1
> argument, per-line block boundaries let opposing consists collide *through the seam*, and
> lowest-index tiebreak **starves the higher line**.

The corrected guarantee is a **stack of three, shipped in one commit** (the half-fix is structurally
impossible — a mutex with an insufficient cap is itself a subtler half-fix):

1. **Atomic whole-block reservation to a physical passing place** (Phase B.6) — a held shared block is
   only ever held by a consist that has reserved a clear run to a passing place where it can park
   owning nothing ⇒ every blocked train waits at a passing place ⇒ the wait-for graph is an **acyclic
   depth-1 forest** (generalizes P4 coalescing to the shared graph).
2. **Cross-LINE dispatch capacity cap** — combined fleet ≤ (passing places + 1) per acyclic shared
   block, **plus a global mutex (capacity 1) on any cyclic shared component** (the ring fix). This is
   *new* work: the landed S1v1 cap only drains across *paths of one line*; this drains across *lines*.
   Liveness is guaranteed **upstream** by this cap, exactly as P2's meet relies on its dispatch cap.
3. **Fair arbitration token** — longest-waiting integer counter / per-block round-robin in edge-key
   order, **not** raw lowest-index (which starves the higher `LineId` because the SoA is filled
   line-by-line).

## 5. Determinism — LITE's win: zero new hashed state, zero re-pin

- **`track_type` stays per-`(line,path,span)`** and is **read live** each tick in Phase A.1.7 (as
  Phase A.1 already does). This is load-bearing: `SetSegmentTrack` deliberately does **not** set
  `dispatch_dirty`, so any dispatch-time *cache* of single/double-derived block topology would go
  stale. The derived `edge_map` caches only pure-geometry node pairs; the single/double classification
  and block-vs-passing-place decision are derived **live**. ⇒ **no `Canonical` shape change, no
  golden-hash re-pin**, and a non-grid / fully-double / non-shared network is byte-identical.
- `phys_occ` / `phys_claimed` are `advance()`-local per-tick scratch — sorted `Vec<(u64,u32)>`, binary
  search, lowest-index-wins (arbitration overlaid per §4.3), **re-derived every tick, never persisted,
  never hashed, no HashMap iteration**. The cross-line block-coalescing pass (in `dispatch.rs` on
  `dispatch_dirty`, geometry only) uses a sorted Vec keyed by `edge_key`, numbered in sorted edge-key
  order ⇒ command-order-independent.
- **Grid geometry must be integer-exact by construction.** `samples=1` on the Catmull-Rom builder is
  **not** enough — it routes vertices through `catmull(...).round()` floats, and two lines with
  different neighbor stops can round a shared vertex across a cell boundary. Grid mode is a **separate
  straight-segment builder** that snaps stops + inter-stop vertices to the lattice with integer
  arithmetic, bypassing `catmull` entirely.
- **Two tripwires** (the `run()==run()` suite is structurally blind to a *uniform* hash shift — zero
  pinned constants today): (a) **double-gate** Phase A.1.7/B.6 on (explicit grid ruleset flag) AND
  (shared-block present) so no continuous fixture enters the clamp by quantization coincidence; (b)
  **add golden-constant pins** — one on an existing non-grid network, one on a shared-grid network.

## 6. Command surface — no new player lever

Shared rail is detected **geometrically** from existing hashed geometry (two grid paths on the same
tile-edge) — sharing is implicit in where the player drew the lines, exactly as junctions are implicit
in branches. `SetSegmentTrack` continues to flip per-span `track_type` unchanged; on a shared edge the
cross-line cap reads single/double live (single-if-any). **The cross-line capacity clamp lives in
`dispatch.rs` and is read back via the `Stats` snapshot** — no wasm setter, no new lever. **Capacity +
Headway remain the only two player levers.** When the cap binds, surface it through the existing
`left_behind`/waiting-dot pressure channel: *"shared section capped at N trains — add a passing place /
double-track to run more,"* converting the ceiling into in-loop teaching that points at a reversible
Build edit.

**The one command decision = the grid flag** → **CityData/ruleset bake property** set at
`Sim::new(seed, city_json)` (zero Command churn; matches the [[dynamic-city-architecture]] "frozen
offline bake" rule and the §3 ruleset-at-construction posture). Fallback only if mixed grid/continuous
lines in one city are ever needed: an additive `CreateLine { grid: bool }` mirrored in
`types.ts`/`codec.ts` + round-trips in the same commit.

## 7. Migration ladder (gate green at every commit; inert-by-default)

- **Step 0 — crisp grid geometry (prerequisite).** A separate integer-only straight-segment builder
  behind the grid flag; snap stops + vertices to `cell_size_mm` with integer math (NOT `literal` —
  `LITERAL_SAMPLES=2` still rounds). RED-first: **two distinct lines over one rail emit identical
  `edge_key` sequences**, `grid_line_replays_bit_for_bit` (3000 ticks), zero-length-span,
  identical-arclen-tie. Existing hashes byte-identical (flag default-off).
- **Step 1 — `edge_key` + derivation (no behavior change).** Add `node_of`/`edge_key` beside
  `seg_key`/`junc_key` (sorted pair, zigzag pack, forbid XOR). Add the cross-line scan in `dispatch.rs`
  (behind grid flag): build the shared-single-block set across **all** lines into a derived
  (non-`Canonical`) World field, boundaries = physical double-track passing places common to all
  traversers, coalesced in edge-key order. Assert no hash change. Add
  `cross_line_block_ids_are_command_order_independent`.
- **Step 2 — the mutex + ALL liveness TOGETHER (one commit, NEVER split).** Phase A.1.7 + B.6 +
  generalized B.5 in `vehicle.rs`; the cross-LINE capacity cap + cyclic-component global mutex + the
  PASS-2 snap + the fair tiebreak in `dispatch.rs`. Land the cross-line RED never-freeze tests first
  (single-span meet, 4-way crossing, ring-no-deadlock, asymmetric passing place, no-starvation), make
  them green.
- **Step 3 — invariants + golden pins.** Seeded property test (N random grid nets, cyclic + acyclic
  shared components ⇒ no frozen consist + no head-on + monotonic gauge); the staggered-partial-sharing
  edge case; the two golden-constant pins. Re-run `determinism.rs`/`junction.rs`/`single_track.rs` —
  green, hashes unchanged.
- **Step 4 — UI read-back only.** The capped fleet + per-block contention surface via `Stats`; the
  bind-message via the waiting-dot channel. No wasm setter, no new lever.
- **Step 5 — docs + hygiene.** Fold into [capacity-roadmap.md](capacity-roadmap.md) (P5/S2 cross-line)
  and [fantasy-fork.md](fantasy-fork.md) §10 A2; log the LITE+L1 decision and the deferred FULL/L2
  seams in `PROGRESS.md`. Both lockfiles untouched (no dep change). Branch off `main` per convention.

## 8. Tests (RED-first)

`grid` integer-exact `edge_key` equality across two lines · `cross_line_single_span_meet_no_headon` ·
`cross_line_4way_crossing_meets_no_freeze` · `cross_line_ring_shared_by_two_lines_never_deadlocks` ·
`asymmetric_passing_place_one_line_stops` · `cross_line_shared_edge_no_starvation` (both lines'
min-traveled > line length) · `cross_line_block_boundary_agreement` (both lines mutex byte-identical
blocks) · zero-length / identical-arclen ties · `grid_shared_rail_replays_bit_for_bit` (3000 ticks) ·
seeded never-freeze property test · two golden-constant pins. **Liveness is NEVER inferred from
replay-equality** — a deterministic deadlock replays green; only the never-freeze property tests catch
it.

## 9. Phase-2 seams (designed, deferred, never half-built)

The user chose "capacity now, signals later." These opt-in advanced-mode seams are designed now so
Phase 1 doesn't foreclose them, and are not built until explicitly taken:

- **FULL = `trait TrackRouter`.** Vehicles pathfind over the materialized TrackGraph (OpenTTD-YAPF):
  pick the cheapest *free* route at a junction. Adds **hashed per-vehicle route state** + per-tick A*
  (the `roadnav.rs` integer grid A* is the precedent) + dynamic re-planning. The same canonical
  `edge_key`s from LITE feed it directly. Behind `trait TrackRouter` so `advance()`'s signature is
  untouched.
- **L2 = signals advanced ruleset.** Player block/chain signals as placeable track objects —
  `PlaceSignal { line, node_x_mm, node_y_mm, kind }` (kind 0=block / 1=chain / 255=remove; `i64` mm
  per AGENTS rule 4/7), `apply()` arm by `SetSegmentTrack`, sets `dispatch_dirty`, mirrored in
  `types.ts`/`codec.ts` in one commit. System guarantees **safety only**; liveness becomes the player
  skill; deadlock is possible.
- **The re-spec L2/FULL forces (call out before building):** (a) a player-facing deadlock is invisible
  on the monotonic, liveness-decoupled coverage gauge (`world.rs::coverage_score`) — a green A+ on a
  frozen network — so L2 **must add a dedicated never-hashed stuck-block / ridership-flatline health
  indicator** and re-specify the gauge story; (b) `Line.signals: Vec<Signal>` (even empty
  `#[serde(default)]`) appends bytes to postcard ⇒ a **uniform hash re-pin** — land the field +
  mirror + round-trips + regenerated golden fixtures in one isolated commit. These belong to an
  **advanced ruleset** the player opts into, leaving the base game's forgiving/2-lever posture intact.

## 10. Invariants this design must hold (checklist)

- Mutex (Phase B.6) and **all** liveness (atomic block reservation + cross-line cap +
  cyclic-component global mutex + fair token) ship in **one commit**; cross-line never-freeze tests
  green before merge.
- Liveness is proven by RED-first never-freeze property tests, **never** by replay-equality.
- Grid geometry is integer-exact by a dedicated straight-segment builder (not `samples=1`); two lines
  over one rail emit identical `edge_key` sequences.
- Phases double-gated on (grid flag) AND (shared-block present); golden-constant pins committed (one
  non-grid, one shared-grid) to defeat `run()==run()` blindness.
- `track_type` stays per-`(line,path,span)`, read live each tick; **no** relocation to a per-edge
  hashed structure for LITE; no `Canonical` shape change; no re-pin.
- All reservation state is `advance()`-local per-tick scratch — sorted `Vec`, binary search, never
  hashed, no HashMap iteration; `edge_key` is sorted-pair zigzag packing (forbid XOR).
- Block boundaries are physical track topology (passing place = double node common to all
  traversers), not per-line stop lists; pinned by `cross_line_block_boundary_agreement`.
- Cross-line arbitration is fair (longest-waiting / round-robin), not pure lowest-index; pinned by
  `cross_line_shared_edge_no_starvation`.
- No new player lever (clamp in `dispatch.rs`, read back via `Stats`); Capacity + Headway stay the two
  levers; FULL (`trait TrackRouter`) and L2 (signals ruleset) are named, deferred, never-half-built.
- Inert-by-default parity: a non-grid / fully-double / non-shared network is byte-identical; the
  continuous OSM slice is unchanged.
