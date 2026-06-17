# TTD L3 — TrackSegment becomes authoritative geometry (de-risked execution plan)

**Status:** PLANNING (2026-06-17). Produced by a map→design→adversarial-review workflow over the live
model. L3 is "the cliff" (geometry ownership flips through the determinism core). This doc is the
green-at-every-commit execution spec, **with the review's confirmed gaps folded into each step as
MUST-FIX constraints.** Supersedes the L3 bullet in [ttd-track-model.md](ttd-track-model.md).

## The flip is incremental: ADDITIVE → MIGRATE-READERS → DROP

Geometry is hashed *only* through `Canonical.lines` (the `Path` serde tree); the `TrackGraph` is a
derived, **non-hashed** pure function of `lines`/`stations`. That non-hashed staging area lets us:

- **Phase A (ADDITIVE, golden-NEUTRAL):** stand up authoritative segments *alongside* `Path`, kept
  byte-identical-derivable from `lines` and **not in `Canonical`**. Every reader still reads `Path`.
- **Phase B (MIGRATE readers one at a time):** flip each consumer to read the segment store. Neutral
  *only* where the read values are provably identical (see G1 — NOT automatically true).
- **Phase C (DROP, ONE documented re-pin):** segment store becomes authoritative + hashed in `Canonical`;
  `Path` shrinks to a service reference; both goldens re-pinned (behaviour-justified).

**Migration (a) is mandatory:** keep per-path `s_mm` arc-length; `Path` keeps a *derived concatenated*
arclen over its segment refs ⇒ `vehicle.rs::advance()`'s integrator is **untouched at every commit**.
Migration (b) (`s_mm → (seg,offset)`) is deferred to L4 where routing needs the segment graph.

## Sub-steps (each = one green commit)

- **A0 — `TrackSegmentId` newtype + serde derives on graph structs.** `ids.rs` + `track_graph.rs`
  derives. NEUTRAL (nothing enters `Canonical`). Test: `TrackGraph` postcard round-trip.
> **A1 OPEN SUBTLETY (found 2026-06-17, must resolve before A1):** a shared segment (one cell-chain
> traversed by ≥2 lines) does **not** have a unique smoothed polyline — Catmull-Rom curvature depends on
> each line's *neighboring* stops/waypoints, so two lines over the same cells can smooth that run
> DIFFERENTLY. So "segment geometry = the owning Path's sub-range, byte-identical" is **false for shared
> segments**. Resolution options for A1: (i) segment owns the **cell-centre** chain (coarse, line-independent,
> matches the L1 graph) and lines keep their own smoothed deviation as a render-only overlay until C1; or
> (ii) define a **canonical** segment smoothing (e.g. clamped to endpoints, neighbor-independent) and have
> lines CONFORM to it at C1 (changes rendered curvature → a behaviour/golden change, must be justified).
> (i) keeps A1 truly additive/neutral; (ii) is the real end-state but defers the curvature decision to C1.
> Pick (i) for A1 (neutral), schedule (ii)'s curvature reconciliation as an explicit C1 sub-decision.

- **A1 — Segments own *derived* geometry tables** (`polyline`/`arclen_mm`/`track_type`/`span_mode`/
  `min_radius_mm`/`speed_cap_mm_s` + segment-local `point_at`/`length_mm`/…), populated by
  `derive_track_graph` slicing the contributing `Path` sub-range. NEUTRAL (non-hashed graph field).
  Test: each segment's concatenated geometry reproduces the owning `Path` sub-range **bit-for-bit** (the
  compatibility invariant the whole flip rests on).
  - **MUST-FIX G4 (segment identity):** id MUST be a **canonical pure function of topology** (sorted
    endpoint cells), **NOT** a first-seen/allocation-order slab. Else when ids become hashed at C1 the
    hash couples to *command order* and breaks the "same network ⇒ same hash" property (today's
    `derive_track_graph` guarantees order-independence). Decide this in A1, not at C1.
- **A2 — `Path` gains `segments: Vec<(TrackSegmentId, bool)>`** via **`#[serde(skip)]`** (NOT
  `serde(default)`) so it's not serialized/hashed yet. Dual-carry: `Path` keeps full geometry. Test:
  `path.segments` concatenates to `path.polyline` after every edit.
- **B1 — Migrate RENDER to read segments** (`render_buf::track_graph_m` + per-segment track_type).
  NEUTRAL (render never hashed).
- **B2 — Re-key P2 single-track meet to `TrackSegmentId`.** ⚠ **NOT a neutral reader-flip as first
  drafted.**
  - **MUST-FIX G1:** re-keying the meet while `track_type` still lives per-`Path` changes occupancy-row
    cardinality for a SHARED single segment (today P2 is skipped there via `cross_span_covered`; B2 makes
    P2 start claiming it) → changes the clamped `ds` → changes hashed `veh_s_mm`. So **EITHER** (a) keep
    `track_type` + the cross/junction skip-guards keyed per-path through B2 and prove byte-identity with a
    SHARED-segment property test before flipping, **OR** (b) fold the `track_type`/meet-authority flip into
    C1's single documented re-pin. Do **not** ship B2 as "neutral" without the byte-identity proof.
  - Test (RED-first never-freeze): two paths over one shared single segment, opposing trains ⇒ no head-on
    + cumulative ridership strictly increases (liveness asserted, never inferred from replay).
- **B3 — Unify cross-line mutex into the per-segment mutex.** ⚠ **liveness lives in a CAP, not the mutex.**
  - **MUST-FIX G2:** `cross_line_ring_never_deadlocks` passes because `dispatch.rs` sets
    `max_lines = cyclic?1:2` and caps the fleet — the cyclic detection is computed *inside*
    `derive_cross_blocks` (union-find). B3 must **keep a derived cyclic-component capacity cap** (derive it
    from the segment graph's component structure) *before* deleting `derive_cross_blocks`. RED-first test
    MUST include an **OVER-PROVISIONED cyclic ring** (trains ≫ 1), not just a meet.
  - **MUST-FIX G3 (PBS):** the atomic-path reservation is **net-new** liveness machinery on the hot path.
    Ship it with BOTH a deadlock-freedom AND a **starvation-freedom** test: a contended **bidirectional
    multi-segment** run asserting cumulative ridership strictly increases for **both** lines (lowest-index
    `occ_claim` is deadlock-free but NOT starvation-free across a multi-segment atomic claim — two trains
    each holding one segment of the other's path is a classic standoff). Specify the cyclic-mutex / aging
    tiebreak that breaks a partial-claim standoff.
- **B4 — Re-express P4 junctions as segment sets** (`group_overlap` → segment-set intersection). NEUTRAL
  (junctions non-hashed). RED-first: junction-segment-set overlap never-freeze.
- **B5 — Migrate routing + stats readers off `Path.polyline`** (pure readers; NEUTRAL). Ensures no reader
  depends on `Path`-owned geometry being authoritative before C drops it.
- **C1 — DROP: segment store authoritative + hashed (the ONE documented re-pin).** Move geometry rebuild
  into the `apply` write-path (intern segments authoritatively + bind `Path.segments`); shrink `Path`
  (geometry fields move onto `TrackSegment`; `Path` keeps `stops`/`loop_line`/`segments` + derived
  concatenated `arclen`/`stop_arclen`); add the segment slab to `Canonical`; flip `Path.segments` to
  `serde(default)`; re-pin BOTH goldens (behaviour-justified: geometry now lives in segment ids; verify
  vehicle `s_mm`/ridership/gauge are bit-identical pre/post — only serialization moved). `SetSegmentTrack`
  re-targets a `TrackSegmentId`; mirror in `types.ts`+`codec.ts`+`contract.rs` SAME commit.
  - **MUST-FIX G5:** migrate every TEST HELPER that reads `Path` geometry (`head_on`, cross-line
    detectors → read `track_type`/`span_of`/`stop_arclen_mm`) in the SAME commit, and RE-RUN the B2–B4
    never-freeze suite against post-C1 segment-authoritative geometry with the helper re-sourced — else the
    liveness gate goes vacuously green (false pass) at the exact commit that matters most.
  - **MUST-FIX G6:** define the **whole-line** `SetSegmentTrack` encoding (today `span == u32::MAX` fans
    out to every path's spans) — a single `TrackSegmentId` drops that affordance; preserve a sentinel/None.
- **C2 — DROP dead code** (`derive_cross_blocks`/`CrossBlock` once fully subsumed). NEUTRAL.

## Review verdict (must address before/while executing)

The naive plan was **NOT safe as-is**: the hash moves at **B2** (G1) and order-independence breaks at
**C1** unless segment identity is topology-pure (G4) — so the "only one re-pin" framing is wrong unless
B2 is proven byte-identical (or folded into C1) and ids are topology-derived. B3 must retain the
cross-line **cap** (G2) + ship PBS with anti-starvation (G3). Phases A0/A1/A2/B1/B5 are genuinely safe
additive/reader-only steps (modulo G4 in A1).

## L4 / L5 attach points
- **L4** (routing + berth choice) attaches at C1's authoritative `Path.segments`: optional migration-(b)
  (`s_mm → (seg,offset)`), a `trait Router` extension traversing the segment graph, berth choice via the
  L2 `berth_key`. The PBS atomic-path reservation from B3 is the substrate L4 routes over.
- **L5** (signals/junctions as objects) attaches at the per-segment `signal state + block identity`
  fields; B4's junction-as-segment-set becomes first-class junction objects; B3's cyclic-component mutex
  is the block-identity foundation L5 makes explicit.
