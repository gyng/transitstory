# P5 — shared track: the road to first-class track objects

**Status:** S1v1 + S2 landed 2026-06-13 (the within-line caps/mutex). **Cross-line shared track landed
2026-06-14** — grid geometry (Phase 1) + the cross-line shared-block meet mutex (Phase 2), the first
time **two distinct lines** physically share one rail and take turns on it. The full TrackGraph (the
real model-change cliff) is scoped below.
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
| **S2** (done) | the **junction window-block** — single spans are first-class blocks | one line's paths | coalesce single spans with the adjacent switch (P4's trick) |
| **Track objects** | `TrackSegmentId` — a first-class physical edge | **any line's** trains | atomic whole-path reservation to the next safe waiting point (PBS) — Phase 2's mechanism, extended to chosen routes (NOT resource-ordering — see the cliff section) |

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

## S2 — the physical-block meet mutex *(DONE 2026-06-13; the reusable primitive)*

**The bug S1v1 left:** a **single span inside an otherwise-double** shared trunk. Here the line *has*
passing places, so S1v1's cap leaves a real fleet — and the trunk + branch trains still pass through
each other on that one physical span. The fix is a **meet mutex on the physical span** (not a cap):
trains take turns, meeting at the adjacent double spans.

**As built:**
- Single shared-trunk spans become **first-class window-blocks** in the dispatch junction set, alongside
  P4's divergence-point blocks. A block is a per-path arclen window `(path, lo, hi)` (a point has
  lo==hi; a span has lo<hi); the **existing A.1.5/B.4 mutex** (`group_overlap`) serialises them
  unchanged — no new pass, no new key. A span is a block iff it is **shared** (>=2 traversing paths =
  trunk + a branch with `diverge_at > k`) AND physically SINGLE on **any** traversing path
  (single-if-any — closes the asymmetric hole where `SetSegmentTrack{span:k}` edits only the trunk, so
  the branch reads double on a rail that is physically single).
- **Coalescing generalised from points to windows** (`coupled` now uses `q.lo − p.hi`): contiguous
  single spans merge into one section, and a single approach **folds into its adjacent switch** within a
  consist-length — so a consist bridging the single span and the switch holds **one** resource, killing
  the **P5×P4 wait-for cycle** (A holds the single span + waits for the switch; B holds the switch +
  waits for the single span). A **non-contiguous** single span (separated by a double) can't bridge into
  a switch, so it stays a standalone block — both covered (`non_contiguous_single_span_meets`).
- **The cap shares its budget round-robin** (S1v1's trunk-takes-all drain starved the branch to 0): a
  fully-single trunk (capacity 1) is still a trunk shuttle, but a single span between passing places
  (capacity >1) gives the trunk AND the branch trains so they **meet** — the mutex serialises the span.
  The two halves (round-robin cap + block mutex) ship together: round-robin without the mutex head-ons.
- Liveness: the cap bounds population; the mutex provides mutual exclusion; coalescing keeps the
  wait-for graph an acyclic depth-1 forest. RED-first: `single_span_between_passing_places_runs_a_meet`
  (branch runs + meets, no head-on, no freeze) + the non-contiguous case. Parity: inert unless a
  branched line has a phys-single SHARED span ⇒ zero re-pins.

**Four adversarial-review rounds hardened S2** (each found a deterministic, replay-gate-blind failure;
all now have regression tests in `junction.rs`):
1. **Bunched-double over-admit** — the cap counted individual double spans, but coalescing merges a
   contiguous single run into one capacity-1 block; bunched doubles over-admitted → P1×P2 deadlock.
   Fixed: count passing-place **RUNS**, not individual doubles.
2. **Staggered region uncapped** — a single span shared by the trunk + a *late* branch (in
   `[min, max diverge_at)`) was outside the cap's `[0, min)` scope → unbounded → deadlock. Fixed: scope
   the cap to `[0, max diverge_at)` with traversing-path-aware `phys_single`.
3. **P2×junction wait-for cycle** — a returning train resting inside a coalesced run + a train P2-gating
   the same physical span formed a 2-cycle (the double-gating the **skip-guard** was meant to prevent,
   bet against and skipped — the review proved it load-bearing). Fixed: `span_block_covered` makes P2's
   per-path meet SKIP any span a junction block owns (the block is the sole authority).
4. **Branch starvation** — the block's lowest-index `try_claim` is deadlock-free (the occupant always
   advances) but **not starvation-free**: on a coalesced ≥2-span run, two lower-index trunk trains hand
   the block off to each other forever, pinning a higher-index branch at v=0. Fixed **conservatively**:
   a ≥2-span run caps the region to 2 trains (trunk + branch alternate fairly); a fully-single trunk
   (no passing place) is a 1-train shuttle; single-span blocks (confirmed not to starve) keep the full
   passing-place capacity.

**Logged follow-up:** the conservative ≥2-span cap of 2 trades throughput for fairness. A
**fairness/aging tiebreak** in the block `try_claim`/`occ_owner` (longest-waiting wins, deterministically
— probably a small derived wait-rank, not new hashed state) would restore the full passing-place
capacity for long single runs. Until then, a multi-span single section runs at most 2 trains.

## Track objects — cross-line shared track *(GO 2026-06-13; the grid path)*

> **Owner decision (2026-06-13):** build the **grid path** (docs/fantasy-fork.md §10 + docs/shared-rail.md),
> not declared-window on continuous geometry — because the cross-line mutex needs **byte-exact**
> physical identity, and continuous Catmull-Rom polylines never share exact vertices (a float-rounded
> vertex lands in different cells ⇒ the mutex silently never engages = a gate-blind false-negative
> head-on). Grid geometry makes identity exact. Built in two phases:

- **Phase 1 — crisp grid geometry (DONE 2026-06-13).** `CityData.grid_cell_mm` (bake property, 0 = off
  ⇒ byte-identical, zero re-pins); `line.rs::grid_walk` builds a dense octilinear lattice polyline
  (cell-centre vertices, canonical so `a→b` reverses `b→a`), integer-exact. **Sharing guarantee (LITE):
  two lines with the same consecutive stop-cells emit byte-identical edges** (the shared-station trunk).
  **Honest limit** (grid review): a corridor shared BETWEEN stops (express/local) needs the FULL laid-
  track model — `#[ignore]`d seam. So Phase 2's cross-line mutex contract is "shared consecutive
  stop-cells". `tests/grid.rs`.
- **Phase 2 — cross-line shared-block mutex (DONE 2026-06-14, shared-rail.md).** Line-independent
  `edge_key` from consecutive grid vertices (`node_of` = `(x.div_euclid(cell), y.div_euclid(cell))`, a
  sorted cell pair); `derive_cross_blocks` (dispatch.rs) collects every grid edge-use, **union-find
  coalesces all shared edges (≥2 distinct lines) that touch a common node into ONE component** (a
  component is a block iff it contains a single edge), and a component is `cyclic` iff
  `edge_count ≥ node_count`. Phase A.1.7 cross-line occupancy + **Phase B.6 atomic whole-block meet
  gate** (reuse `occ_claim`/`group_overlap`): a held consist parks its head AT the near gate with its
  whole tail BEHIND the block (`tail = gate − dir·len`), so a waiter never occupies the block it waits
  for ⇒ the wait-for graph stays an **acyclic depth-1 forest**. The liveness STACK shipped in one
  commit:
  - **atomic whole-block reservation** (no partial entry — the head can't cross the near gate unless the
    whole block is claimable);
  - **cross-LINE dispatch cap** (`cross_cap` in dispatch.rs): `max_lines = 1 if cyclic else 2`; the
    lowest-index served lines get **1 train each**, the rest **0** — so a shared block carries ≤2 lines,
    1 train apiece (a cyclic component is a 1-line shuttle);
  - **single-owner mutex** on the block via the existing `try_claim` (lowest-index-wins).

  Derived/unhashed (zero re-pins — inert unless `grid_cell_mm > 0`), routing untouched, `Canonical`
  unchanged. **Two adversarial-review rounds** (the budget was 4+; round 2 was completely clean across
  ~14 runnable counterexamples, so the conservative design converged early):
  1. **Two gate-blind deadlocks** — (a) a short double shared run (< a consist length) was miscounted as
     a passing place ⇒ a multi-block wait-for cycle; (b) a `passing_places + 2` round-robin handed one
     line 2 trains that met head-on inside the block. **Fixed:** coalesce **all** shared edges into
     components (block iff it has a single edge), cap to **1 train/line, ≤2 lines** acyclic / **1**
     cyclic (commit `5361bf7`).
  2. **Clean.** 3 lenses (residual-deadlock under the new cap, layer-interaction with P4/P2, determinism
     /parity/3-line), ~14 runnable counterexamples — the opposing two-resource cross-block cycle (worst
     sustained simultaneous stall **13 ticks** over 40 seeds × 6 phase offsets — never deadlocks),
     3-line A-B/B-C chains (coalesce to one block through the shared node), dead-end head-on blocks,
     out-and-back rings (correctly cyclic ⇒ 1-train shuttle), P4-junction × B.6, P2-private × B.6
     boundary, single-on-A/double-on-B (single-if-any). **None went red.** Determinism bit-for-bit
     (identical `state_hash` twice); the two transient REDs were test-harness artifacts (a too-loose
     euclidean head-on detector flagging a legal post-block meet; a clamped track-constant), both
     corrected to green.

  **Logged follow-up:** the 1-train-per-line cap is deliberately **conservative** — over-throttle is the
  safe direction (a line sharing any block runs 1 train *globally*). A richer **per-block** capacity
  (cap only the shared-block contention, let a line run more trains on its private sections via a
  passing-place count + a fair aging tiebreak — the S2 fairness follow-up, now shared with this layer)
  restores throughput. Until then, any line touching a shared block runs a shuttle.

## Track objects — the FULL model — the cliff *(after Phase 2; the real model change)*

- **`TrackSegment` becomes first-class** (geometry + per-segment track/signal state); `Line` references
  a `Vec<TrackSegmentId>`; the importer + editor build/edit segments; lines are assigned to segments.
- The block key graduates to `TrackSegmentId` (**cross-line**): two distinct lines on one segment
  contend for one key — the reservation machinery is unchanged.
- **The genuinely new hard part — cross-line deadlock-freedom.** P4's single-owner coalescing collapses
  *one line's* cycles; cross-line wait-for cycles span multiple lines and **cannot** be collapsed that
  way. **The discipline is NOT resource-ordering** — that was this roadmap's original plan, and the
  Phase-2 adversarial review *broke* it: "acquire segments in global segment-id order" only prevents
  cyclic wait-for if every train acquires in that order, but a train acquires in **physical-traversal**
  order, and **opposing trains traverse the same segments in opposite order** (eastbound `s5→s9→s12`,
  westbound `s12→s9→s5`), so the total order constrains nothing → classic 2-cycle deadlock (gate-blind:
  it replays green). The proven discipline is the one Phase 2 shipped, extended to chosen routes:
  **atomic reservation of the whole path to the next safe waiting point** (PBS — a train can't enter a
  shared section unless it can reserve a clear run to a place it can wait *outside* the section, so a
  waiter never holds a resource a mover needs ⇒ acyclic depth-1 forest) **+ a cross-line capacity cap**
  (bound contention per shared block) **+ a global mutex on cyclic shared components** **+ a fair
  (aging/round-robin) tiebreak** (lowest-index starves the higher-`LineId` line). Proven by a multi-line
  never-freeze property test — the determinism gate is blind to this deadlock, so the liveness test is
  the only safeguard.
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
