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
  `serde(default)`) so it's not serialized/hashed yet. Dual-carry: `Path` keeps full geometry.
  - **RESOLVED WRINKLE (2026-06-17):** the binding is NOT computable at `apply`/`rebuild_line_geometry`
    time — a segment boundary is a junction whose degree depends on *other* lines, so a path can't
    decompose itself into segments without the whole graph. Therefore A2's binding is a **DERIVED
    post-dispatch computation** (filled in right after `derive_track_graph`, like the graph itself), stored
    on `Path.segments` as `#[serde(skip)]` (non-hashed). At C1 this binding moves into the authoritative
    apply-time write-path. Resolve each path's ordered covered-segment refs by walking its polyline→cells
    and matching the derived segments' cell chains (forward/reverse), same machinery A1 uses.
  - **Test:** `path.segments` concatenates to `path.polyline` **for single-line (unshared) paths only** —
    a shared segment carries the lowest-index representative's curve, so the invariant is "concatenates to
    the representative," not "to this path" (the documented curvature subtlety; full reconciliation at C1).
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

## Phase A complete + the Phase B/C entanglement finding (2026-06-17)

**Phase A is DONE, green, golden byte-identical** (A0 `764595b`, A1 `ab48f7d`, A2 `6824f39`): segments own
derived smoothed geometry (lowest-index representative, #29 curves preserved) and each `Path` carries a
derived post-dispatch `segments` binding (`#[serde(skip)]`, non-hashed). The additive staging area is built.

**Finding — Phase B/C does NOT decompose into clean independent reader-flips.** Executing A1/A2 revealed the
remaining concerns INTERLOCK and cannot be shipped as separate golden-neutral commits the optimistic plan
implied:
- The owned segment geometry from A1 lacks `track_type`/`span_mode` (they're per-`Path`, HASHED via
  `Canonical.lines`). So "B1: render per-segment track_type" and "B2: P2 meet reads the segment's track_type"
  both REQUIRE moving `track_type` onto the segment — which is a hashed-state move (re-pin), i.e. C1 work.
- B2's meet re-key changes occupancy-row cardinality for shared single segments → changes clamped `ds` →
  changes hashed `s_mm` (review G1). So B2 is hash-moving, not neutral.
- B3 deletes `derive_cross_blocks`, which computes the `cyclic` capacity CAP that PROVIDES cross-line
  liveness (not the mutex) — the replacement cap must land in the SAME unit (review G2).
⇒ **B2 + B3 + the `track_type`/`span_mode` ownership move + C1 must be designed and executed as ONE focused,
determinism-core unit** (one documented re-pin; RED-first never-freeze tests for every liveness change; PBS
atomic-path reservation + cross-line cap + aging tiebreak per G3). This is the genuine "cliff" — it needs a
fresh, focused context, NOT an incremental grind: a gate-blind deadlock replays green (AGENTS), so a rushed
commit here is a silent determinism-spine break. Phase A (safe) is the right stopping point for one pass; the
B/C unit resumes here with this doc + the 6 review gaps as the spec.

## Cliff attempt #1 (2026-06-17) — REJECTED by review, reverted; learnings

A tests→implement→review Workflow attempted the whole B/C cliff in one shot. The adversarial review
returned **NOT SAFE** and it was **reverted** (nothing unsafe committed; both goldens stay at the Phase-A
pins). What the implement agent actually produced + why it was rejected — these are HARD CONSTRAINTS for
the next attempt:
- **It did NOT implement B3** — `dispatch.rs`/`derive_cross_blocks`/the `vehicle.rs` cross-line mutex were
  untouched. No segment-keyed mutex, no PBS atomic-path reservation, no segment-derived cyclic cap, no
  `SetSegmentTrack`→`TrackSegmentId` re-target, no contract mirror (G6). It only moved `track_type`/
  `span_mode`/`min_radius` onto the segment + re-keyed the meet + re-pinned.
- **Unearned re-pin (the key lesson):** it HASHED `track_type`/`span_mode`/`min_radius` on the segment while
  they had **zero state-affecting consumers** and were still *derived from* the hashed `Path` (geometry NOT
  moved off `Path`). ⇒ a golden re-pin cost with no ownership flip behind it. **DO NOT hash segment geometry
  fields until C1 ACTUALLY moves geometry off `Path` (Path stops authoring it).** Until then keep them
  `#[serde(skip)]` (A-phase style) and pay NO re-pin.
- **Vacuous liveness gates + fabricated evidence:** the never-freeze gates passed against pre-cliff source
  (they test the *existing* cap), and the re-pin comment cited integrator fingerprints that **no test
  computes**. The committed `shared_segment_liveness.rs` regression guards (green at HEAD, RED against a
  stubbed cap) are now the real gate; the re-pin must be justified by a **committed two-build identity
  assertion**, not prose.
- **Conclusion:** the cliff is genuine multi-session expert work, NOT one-shot-able by an agent — B3 (the
  cross-line liveness machinery) must be written + tested RED-first against an *intermediate no-cap build*,
  the ownership flip must be real (Path loses geometry) for the re-pin to be earned, and the contract mirror
  lands same-commit. Throwing more one-shot Workflows at it reproduces overclaiming.

## DE-SCOPING INSIGHT (2026-06-17) — B3 is deferrable; C1 shrinks to the earned geometry flip

The "irreducible cliff" was partly self-imposed. **L4 (routing + berth choice) needs segment-AUTHORITATIVE
GEOMETRY, not a rewritten cross-line cap.** The existing `derive_cross_blocks` union-find cap + the
`vehicle.rs` cross-line mutex ALREADY provide cross-line liveness (proven by the goldens + the committed
ridership never-freeze guards). So the real minimal L3-for-L4 is:
- **C1 (do this):** move geometry ownership onto `TrackSegment` (authoritative + hashed — an EARNED re-pin,
  because `Path` genuinely STOPS authoring `polyline`/`track_type`/etc.; `Path` derives a concatenated
  geometry from its `segments` so `advance()`'s integrator is byte-unchanged, migration-(a)). KEEP the
  working cross-line machinery untouched (the meet reads `track_type` via the segment, decision-identical).
- **B3 (DEFER):** unifying the cross-line cap/mutex into a segment-keyed PBS reservation is a *separate,
  optional* refinement — NOT a prerequisite for L4. It only matters if/when we want richer multi-line
  routing than the current cap allows. The committed `shared_segment_liveness.rs` guards gate it whenever
  it's attempted.

**The earned-re-pin identity gate (fixes review D3's fabrication):** before the C1 flip, COMMIT a
`position_fingerprint_pinned` test — an FNV of `(sorted vehicle s_mm + per-line ridership)` after N ticks
of the transit + arcadia golden logs, pinned at the CURRENT values. The C1 flip then MUST keep that
fingerprint byte-identical (proving "only serialization moved" — the golden *hash* legitimately changes,
but the *positions* must not). That is the real, committed, mechanical proof the first attempt faked.

## C1 scope finalized + the shared-corridor curvature consequence (2026-06-17)

C1 (the earned geometry flip) is now fully gated and ready to execute:
- **Gates in place (committed):** `position_fingerprint.rs` (the flip MUST keep TRANSIT `0xdccb…e54a` /
  ARCADIA `0xbf4d…3236` byte-identical — proves positions unchanged for the golden scenarios while the
  golden *hash* legitimately re-pins); `shared_segment_liveness.rs` (the flip must keep these green — keeps
  the working cross-line cap); replay-equality; full suite.
- **The one genuine behaviour change (document + accept):** making geometry segment-authoritative means a
  SHARED segment carries ONE canonical curve, not each line's independent Catmull-Rom smoothing. For the
  goldens this is invisible (transit = continuous/no segments; arcadia = single line/no sharing) ⇒ the
  position fingerprint is UNCHANGED, "only serialization moved" holds. For a MULTI-LINE shared corridor the
  rendered/integrator curve of the shared run changes to the canonical representative — which is the
  CORRECT "one track, many services" behaviour (#36): two services on one corridor should ride ONE shared
  rail with ONE curve, not two near-identical divergent ribbons. So this is an intended improvement, not a
  regression — but it is a real geometry change on shared track that the (non-sharing) goldens don't
  exercise; note it in the C1 commit so it isn't mistaken for a bug.
- **C1 commit shape:** geometry fields leave `Path` (segments author them, in `Canonical`, hashed = earned
  re-pin); `Path` derives a concatenated geometry from its `segments` so `advance()` is byte-unchanged for
  NON-shared paths (migration-(a)); keep `derive_cross_blocks` (B3 deferred); `SetSegmentTrack` →
  `TrackSegmentId` + whole-line sentinel, mirrored in types.ts/codec.ts/contract.rs same commit (G6).
  Re-pin both goldens (justified); position fingerprint MUST stay byte-identical (the mechanical proof).

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
