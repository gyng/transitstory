# P5 — shared track: the road to first-class track objects

**Status:** S1v1 landed 2026-06-13 (the cross-path single-track cap). S2 (the physical-block meet
mutex) and the full TrackGraph are scoped below.
**Product driver (2026-06-13):** an impending fork into a transport-builder game where **distinct
lines sharing physical track** (a central tunnel, a shared viaduct, OpenTTD-style track networks) is a
headline feature. The roadmap originally deferred this as "the architectural cliff — do not build
speculatively" ([capacity-roadmap.md](capacity-roadmap.md)); the product driver removes the
*speculative* premise, so we build it — **incrementally**, foundation-first.

## The end-state (owner decision 2026-06-13): full track objects

Tracks become **first-class objects** that lines run *on* (the OpenTTD model): a `TrackSegment` has its
own geometry/signals; a `Line` references an ordered list of segments; any line may use any segment;
junctions/signals/shared depots fall out of the graph. Routing runs on the track graph. This is the
largest model change in the project and is reached in layers — each layer ships green and is testable.

## The one reusable primitive: a physical-block reservation

Every layer is the same operation as P1/P2/P4: a train's per-tick advance is a `min()` over resource
limits, where a **physical track block** is reserved by at most one consist at a time (the
`occ_claim`/`occ_owner`/`try_claim` sorted-Vec machinery, `vehicle.rs:134-178`, already proven for P2
single-track and P4 junctions). What changes per layer is **only the block's identity key**:

| Layer | Block identity (the key) | Who contends | Deadlock discipline |
|---|---|---|---|
| **P2** (done) | `(line, path, span)` — per service path | one path's trains | meet at passing places + per-path cap |
| **P4** (done) | `(line, key_station)` — a switch cluster, per line | one line's paths | coalesce within a consist-length ⇒ one owner ⇒ acyclic |
| **S1v1** (done) | *no mutex* — a **dispatch cap** on the shared trunk | one line's paths | cap the fleet to the section's single-track capacity |
| **S2** (next) | `(line, physical-span)` — a single span shared by a line's paths | one line's paths | coalesce single spans with the adjacent switch (P4's trick) |
| **Track objects** | `TrackSegmentId` — a first-class physical edge | **any line's** trains | **resource ordering** across the segment graph (the new hard part) |

The reservation *mechanism* is constant; only the key derivation graduates from line-scoped to
cross-line. So each layer is a small extension of a proven primitive, not a rewrite.

## S1v1 — the cross-path single-track cap *(DONE 2026-06-13)*

**The bug it fixes** (reachable today via one `SetSegmentTrack{span:u32::MAX, track:SINGLE}` on a
branched line): a branched line's trunk path and a branch path are two independent polylines tracing
the **same physical trunk rail**. P2's meet keys per `(line, PATH, span)`, so a trunk consist and a
branch consist on that one rail get different keys, never mutex, and **pass through each other** —
captured (RED) by `junction.rs::fully_single_shared_trunk_no_headon_and_never_freezes`.

**Why a cap, not a mutex, is the load-bearing fix here.** A *fully-single* shared trunk has no passing
place, so **two trains deadlock on it even with a perfect mutex** (the trunk and branch trains desync
over their different circuits and eventually oppose with nowhere to pass — the same reason P2 caps a
non-branched fully-single line to a one-train shuttle). The only correct fix is to bound the population:
**all paths traverse the shared trunk, so its single-track capacity bounds the *whole* fleet**, not each
path independently. S1v1 caps the total trains across the trunk + every branch path to
(physically-double shared-trunk spans) + 1 — 1 for a fully-single shared trunk (a shuttle); the trunk
(lowest path index) wins the budget; the branch is unserved until the player doubles a span (a passing
place appears ⇒ the cap rises) — the same informative pressure as P2's single-track shuttle.

**Containment:** a pure `dispatch.rs` count clamp; re-derived (never hashed); **inert** unless a
branched line has a physically-single span on its universally-shared prefix `[0, min diverge_at)` ⇒
**zero re-pins** (all 31 sim suites byte-identical). No new Command, no `Canonical` change, no
`types.ts` mirror.

## S2 — the physical-block meet mutex *(next; the reusable primitive)*

**The residual bug S1v1 leaves** (captured `#[ignore]`d as
`junction.rs::single_span_in_double_shared_trunk_no_headon_is_s2`): a **single span inside an
otherwise-double** shared trunk. Here the line *has* passing places, so the cap leaves a real fleet — and
the trunk + branch trains still pass through each other on that one physical span. The fix is a **meet
mutex on the physical span** (not a cap): trains take turns, meeting at the adjacent double spans.

**Design (drawn from the P5v1 design workflow + its corrected analysis):**
- A shared-trunk span is a **physical block** keyed on the physical segment (line-scoped for S1; the
  station-pair / a derived per-line segment index — drift-proof, never a Catmull-Rom arclen scalar).
  A span is physically SINGLE iff SINGLE on **any** contending path (single-if-any — closes the
  asymmetric-track hole where `SetSegmentTrack{span:k}` edits only the trunk path).
- Reuse P4's block-mutex machinery: add single spans as **first-class window-blocks** to the resource
  set and **coalesce them with adjacent switches and with each other within a consist-length** — so a
  consist bridging a single span and the divergence switch holds **one** resource, which is the only
  way to kill the **P5×P4 wait-for cycle** (train A holds the single span + waits for the switch; train
  B holds the switch + waits for the single span). Coalescing is P4's exact trick, generalised from
  points to windows. Non-adjacent single spans (separated by a double) cannot bridge into a switch, so
  a standalone block mutex covers them.
- Liveness: the cap (S1v1) bounds population; the mutex provides mutual exclusion; coalescing keeps the
  wait-for graph an acyclic depth-1 forest. Proven RED-first by un-ignoring the S2 test + never-freeze
  fixtures (single span between passing places running a real meet, not over-throttled).

**Open edges flagged for the S2 design** (found by hand-analysis of the rate-limited P5v1 design break):
the locked "fold-into-junction" design as written misses **non-contiguous** single shared spans and its
cluster-range cap **over-throttles** a mostly-double line — both are why S2 must model single spans as
first-class blocks with a passing-place-counting cap, not a backward-walk widening. Also: the
**staggered partial-sharing** region `[min diverge_at, max diverge_at)` (single spans shared by the
trunk + a *late* branch but not an early one) needs explicit coverage + a test.

## Track objects — the cliff *(after S2; the real model change)*

- **`TrackSegment` becomes first-class** (geometry + per-segment track/signal state); `Line` references
  a `Vec<TrackSegmentId>`; the importer + editor build/edit segments; lines are assigned to segments.
- The block key graduates to `TrackSegmentId` (**cross-line**): two distinct lines on one segment
  contend for one key — the reservation machinery is unchanged.
- **The genuinely new hard part — cross-line deadlock-freedom.** P4's single-owner coalescing collapses
  *one line's* cycles; cross-line wait-for cycles span multiple lines and **cannot** be collapsed that
  way. The discipline is **resource-ordering** (a train acquires shared segments in a global
  segment-id order so no two trains acquire in opposite order — the classic deadlock-free rule), proven
  by a multi-line never-freeze property test. The determinism gate is blind to this deadlock, so the
  liveness test is the only safeguard.
- Routing/RAPTOR extends to the track graph; junctions/signals/shared depots emerge from segments.
- **Thin-loop guard:** the player-facing surface (drawing/sharing track) must stay inside the loop
  budget — likely "draw track, then assign lines to it" as the two gestures, with sharing **emergent**
  from co-located segments where possible rather than a separate mode.

## Cross-cutting (all layers)

- **Determinism:** every block reservation re-derives per-tick (sorted Vec, binary search, no HashMap
  iteration, integer); nothing new enters `Canonical` until a layer genuinely needs hashed segment
  state. Same-seed-same-log replay stays green at every commit.
- **Liveness is never inferred from replay-equality** — each layer ships a RED-first never-freeze test,
  because a deterministic deadlock replays green.
- **Each layer is shippable alone:** S1v1 fixes a real bug standalone; S2 fixes a real bug standalone;
  track objects is the model change. No half-built cliff.
