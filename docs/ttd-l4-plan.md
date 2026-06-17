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
