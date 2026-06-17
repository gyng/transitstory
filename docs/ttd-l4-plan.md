# TTD L4 — vehicle routing on the segment graph + parallel-berth overtake (de-risked plan)

**Status:** PLANNING (2026-06-17). Produced by a map→design→adversarial-review workflow over the post-L3
model. L3 is DONE (geometry segment-authoritative; `Path.segments` binds to the hashed `track_segments`
slab; B3 cross-line PBS deferred but the existing `derive_cross_blocks` cap + cross-line mutex work). This
is the green-at-every-commit execution spec, with the review's confirmed gaps folded in as MUST-FIX.

## The two orthogonal gates (keep separate)
- **State hash** (goldens `0xea39_755c_339d_ab9e` transit / `0xbc3c_87c3_28ba_0d70` arcadia) — moves ONLY
  when a new field enters `Canonical`. The ONE earned L4 re-pin is **L4f** (`vehicle.berth` joins Canonical).
- **Position fingerprint** (`0xdccb_466e_60a6_e54a` / `0xbf4d_cc02_ef2d_3236`) — folds per-vehicle
  `(line,path,dir,s_mm)` + ridership. The pinned transit/arcadia scenarios are **K=1** (no `BuildPlatforms`),
  so they MUST stay byte-identical through ALL of L4 — a move there is silent drift, not a re-pin. The
  overtake's real motion change is asserted by NEW fingerprints on K≥2 test scenarios (L4f/L4g), not by
  tweaking the K=1 constants.

## Sub-steps (smallest independently-green commits)
**Phase 1 — routing primitive (golden- AND fingerprint-NEUTRAL; pure scratch, zero consumers) — SAFE:**
- **L4a — CSR node→incident-segment adjacency** on `TrackGraph` (`inc_start`/`inc_flat`, `incident(node)`),
  built at the tail of `derive_track_graph` (promote the existing step-3 `inc` pattern). Non-hashed. Test:
  `incident(node)` = segments with `a==node||b==node`, sorted by seg_id, count==degree.
- **L4b — deterministic least-cost segment search** (`routing/segment_graph.rs`): integer Dijkstra over the
  segment graph, edge cost = `segment.length_mm()`, tiebreak = canonical seg_id; `Vec`-indexed dist/prev (no
  HashMap iteration, no float). Pure fn, no caller. Test: diamond picks shorter; equal-cost picks lower seg_id.

**Phase 2 — berth geometry + the MOTION-CHANGING overtake (delicate; the blockers live here):**
- **L4c — berth CHOICE as scratch** (computed, not consumed): heading-matched free-berth pick beside the
  existing seed; K=1 degenerates to today. Neutral.
- **L4d — berth-throat as a P4-style HARD mutex** (liveness change). RED-first never-freeze `platforms_
  throat_never_freezes`.
- **L4e — per-berth parallel arclen geometry** (berth b≠0 gets a distinct platform `s_mm`; berth 0 ==
  legacy centerline). Derived from the segment slab; neutral on K=1.
- **L4f — vehicles TAKE the chosen berth** (THE motion commit): `vehicle.berth: Vec<u8>` graduates into
  `Canonical` (the ONE earned re-pin — K=1 goldens are all-zeros ⇒ empty-slice shift, document like C1);
  `move_trains` brakes to the berth-indexed arclen. RED-first behaviour test `k2_two_trains_dwell_in_parallel_
  berths` + a NEW K≥2 position fingerprint. K=1 fingerprint MUST hold.
- **L4g — reorderable follow-stream: the actual OVERTAKE** (`leader[i]` recomputed from berth/lane occupancy;
  re-merge reuses the P4 junction mutex). RED-first overtake + never-freeze.

**Phase 3 — pax RAPTOR on the segment graph (SEPARABLE, golden-neutral):**
- **L4h — RAPTOR edge cost from the shared segment graph** behind the `Router` trait swap; KEEP `Leg`
  station-keyed so pax board/alight is byte-unchanged. Land behind a RED-first leg-equivalence test (segment
  cost == `stop_arclen_mm` sum on the single-corridor goldens) ⇒ golden-neutral; if legs diverge, a separate
  documented routing re-pin. Reuses L4a/L4b only.

## REVIEW VERDICT: NOT safe to execute as-is — close these first
The phase ordering + re-pin discipline are SOUND (L4a/L4b are genuinely safe to start). But:
- **G1 (BLOCKER) — L4g berth-allocation deadlock not proven RED-first.** "Two trains each routed into the
  other's only free berth" is an allocation cycle distinct from the converge mutex. Today's greedy
  lowest-free-index, first-claimant-wins claim is allocation-safe by construction (no hold-and-wait); L4c's
  geometry-matched predicate could break that. FIX: a RED-first never-freeze whose stub is "wait for a
  specific compatible berth" (deadlocks) vs the greedy take-any-free-this-tick (ridership rises).
- **G2 (BLOCKER) — L4g leader-recompute can break the junction mutex's depth-1 acyclicity.** A train holding
  a berth while waiting for the single physical exit, opposite a train holding the exit while waiting for the
  berth, is a 2-resource cycle the switch-coalescing trick does NOT cover. FIX: the L4g never-freeze must stub
  the LEADER RECOMPUTE (not just the converge mutex) and prove the reorder can't create a berth↔converge
  hold-and-wait; document why one exit + index-ordered atomic claim stays depth-1.
- **G3 (BLOCKER) — L4d's HARD throat mutex regresses K≥2 behaviour it declares neutral.** Today's berth claim
  is a SOFT A.3 relaxation (only ever relaxes a follower forward, never HOLDS); L4d makes it a hard gate that
  can DENY. Neutral on the K=1 goldens but changes K≥2 motion — and the existing `platforms.rs` K=2 tests
  aren't fingerprint-pinned, so a regression would pass the fingerprint gate. FIX: pin a fingerprint on the
  existing K=2 `bunched_line` scenario BEFORE L4d; L4d holds it neutral or is re-classed motion-changing.
- **G4 (MAJOR) — exit-geometry invariant untested.** A berth is a DIRECTED slot; a train must not pick a
  berth it can enter but not LEAVE in its onward `dir` (through-clear-forward vs terminus-reverse). FIX: L4e/L4f
  unit test asserting the chosen berth's exit admits the onward direction.
- **G5 (discipline)** — (see workflow output) tighten the route-choice determinism / command-order-independence
  assertion at the hash level.

## Sequencing
L4a + L4b are pure, neutral primitives — safe to land first (they also unblock L4h). The delicate motion/
liveness cluster (L4c–L4g) must address G1/G2/G3/G4 with RED-first never-freeze tests that STUB the new guard
(allocation, leader-recompute, throat) — liveness NEVER inferred from replay. L4h is fully separable (Router
seam). The K=1 position fingerprint is the silent-drift tripwire throughout; the ONE earned state-hash re-pin
is L4f.

---

## RESOLUTION (2026-06-17) — design panel + empirical refutation: defer the WHOLE berth-contention cluster

A map→design→adversarial workflow (`ttd-l4-overtake-design`) and a direct empirical probe settled the
overtake question decisively. Two outcomes:

**(1) The hard throat mutex (L4d) is DEFERRABLE — confirmed, high confidence.** A through-station berthed
train owns NOTHING (vehicle.rs passing-place gate); `berth_occ` is per-tick scratch, never a wait-for
resource (A.3 only ever un-clamps forward, never denies). So the G2 berth↔exit 2-cycle and G1 allocation
cycle CANNOT form on a single line — they require a held-berth-while-waiting edge that today's relaxation
never creates. L4d's hard deny WOULD introduce that edge, and belongs only to the cross-line capacity
workstream (where it must re-prove G1/G2 RED-first against *that* code).

**(2) Single-line OVERTAKE is a solution without a reachable problem — the whole O1/O2/L4c–L4g cluster is
DEFERRED to cross-line.** The design panel recommended O1 (recompute `leader[]` by loop-coordinate order) +
O2 (unify the Phase-B exit-claim tiebreak), both K=1-byte-identical, no Canonical re-pin. But a direct probe
(a heavy-demand k=2 loop, bunched and spaced, 2–5 trains, demand cranked 150×) showed:
  - `max_dwell_ms ≈ 1700` even at 150× demand — load-dependent dwell is hard-capped at
    `MAX_EXTRA_DWELL_FACTOR * base_dwell` (pax.rs), far too short for a follower to catch a still-dwelling
    leader across the inter-train gap.
  - **Two trains NEVER dwell simultaneously at one stop** (`ticks_2dwell == 0` in every configuration —
    bunched 5-train *and* spaced 2-train). The A.3 relaxation pulls a follower into berth 1 only TRANSIENTLY
    during approach (`max_berth==1` but no co-dwell); by the time it would dwell, the leader has departed.
  - A "functional overtake" metric was **identical at k=2 and k=1** (565 vs 565) — i.e. the only motion
    observed is ordinary loop traffic; there is no berth-enabled overtake to enable.

  Therefore single-line overtake (O1/O2) would be dead code gated on a co-dwell condition that the dwell
  model never satisfies. The L2 A.3 relaxation already does the useful single-line work (transient parallel
  pull-up = jam relief). Genuine simultaneous multi-train dwell at one station happens only CROSS-LINE (two
  independent lines sharing a station), which is the deferred capacity workstream — there O1/O2/L4d/L4e and
  the `vehicle.berth`→Canonical re-pin become relevant and must be re-derived against cross-line liveness.

  Side-finding (a real but cosmetic pre-existing L2 artifact, NOT introduced here): at k≥2 with heavy
  bunching, a berth-1 train snaps back to the centerline arc-length coincident with the berth-0 leader on
  exit (`min_head_gap ≈ 5 mm` observed) — masked visually by the render lateral offset. A clean fix is the
  deferred L4e per-berth parallel arc-length geometry; it is out of scope until the cross-line workstream.

**What this leaves on the L4 critical path NOW:** L4's PRIMARY goal — *vehicle/pax routing on the segment
graph*. L4a (CSR adjacency) + L4b (segment Dijkstra) landed (golden-neutral). L4h (RAPTOR edge cost from the
shared segment graph, legs station-keyed, golden-neutral, behind the Router trait) is the clean completion
and reuses L4a/L4b. The berth cluster (O1/O2/L4c–L4g) and L4d/L4e/L4f move to the cross-line capacity
workstream, documented above. O0 (the K=2 berth-motion fingerprints, `position_fingerprint.rs`) is landed
and stands as the tripwire for whenever that workstream resumes.
