# Capacity & topology roadmap — network physics that's emergent and buildable

**Status:** design locked 2026-06-12. Phase 1 in progress (TDD).
**Owner decision (2026-06-12):** trunk-tree branching · train length derived from spec · service
pattern round-robin-default-but-player-settable · hybrid sequencing **P1 → P3 → P2 → P4** · P5
seam-only with a go/no-go after P4.

## Why
Today a line's capacity is a **flat clamp** — `MIN_HEADWAY_MS`, `MAX_TRAINS_PER_LINE`
(`world.rs:25-27`) — bolted on for SoA buffer sizing, with **no signalling, no train length, no
junctions, and no branching** (a line is a single linear `stops: Vec<StationId>` + `loop_line:
bool`, `line.rs:27-28`). Two consequences:

1. **Capacity is free and fake.** Trains pass through each other at any spacing (`vehicle.rs::advance`
   moves every vehicle independently — no following/occupancy logic anywhere). Throughput is
   `trains × abstract-capacity`, capped by an arbitrary 24.
2. **Real networks can't be drawn.** The Circle Line imports as one linear 28-stop spine
   (HarbourFront→Dhoby Ghaut); its **CE branch (Promenade→Bayfront→Marina Bay) is dropped** — the
   linear model can't branch, and `build_networks.py:166`'s `min_stops=3` stub filter discards a
   standalone short branch. The **Jurong Region Line** is worse: a genuine **multi-branch** service
   (Bahar Junction is a 3-way JS/JW/JE divergence) — the known "Jurong stub" artefact in PROGRESS.

The roadmap replaces both with capacity that **emerges from track physics** and is **fixed by
spending money** — the same identity as build-legality + economy (Surface/Elevated/Tunnel, the water
hard-gate, capital cost). Every capacity limit gets a costed engineering fix:

| Limit | Emergent cause | Costed fix |
|---|---|---|
| Block throughput | trains can't safely close on the leader (length + braking) | more trains *up to* the block limit |
| Single-track meets | opposing trains contend for one track | **double-track** the span ($) |
| Junction conflict | branches diverge/converge, or lines cross, at grade | **grade-separate** — flyover/dive-under (reuses Elevated/Tunnel $) |
| Shared-track merge | two distinct lines over one physical edge | P5 — heavy, deferred |

## The one architectural investment: a movement-authority layer
Every phase is the same operation. Each tick a train computes its desired advance, then asks
**"what's the furthest position I'm authorized to reach?"** — a `min()` over resource limits, from a
**start-of-tick snapshot**, integer, index-ordered (⇒ deterministic and order-independent):

```
authorized_s(train) = min(
   leader_gap_limit,        // P1 same-direction block following (+ train length)
   single_track_limit,      // P2 opposing-direction segment reservation
   junction_conflict_limit, // P4 divergence/convergence/crossing mutex
   shared_block_limit,      // P5 cross-line merged separation (seam only)
)
```
This is a simplified moving-block + interlocking authority model. P1 builds the layer with one
resource; each later phase registers another. **This is the through-line** that makes the roadmap one
system, not four bolt-ons.

## Physical properties woven across all phases
- **Curvature → max speed** — *already exists* (`vehicle.rs:141`, `speed_cap_at(s)`, G0 `v=√(lat·R)`).
  P4 extends it to a **turnout speed cap** at branch divergences (a diverging move is restricted
  independent of track curvature).
- **Train length** — *new*, **derived from the trainset spec** (`length_mm` per spec; a rail consist
  is long, a bus short) so it's **not a new player lever** (the thin loop is guarded). Constant per
  line ⇒ no new per-vehicle state. Threads through P1 (follow is head-to-tail) and P4 (junction
  clearing time ∝ length).

## A key dynamics fact (drives P1's shape)
On a **homogeneous** line, evenly-dispatched trains obey the same autonomous 1-D ODE `ṗ = f(p)`, so
their order is preserved and **they never collide** — adding trains just packs them tighter until the
even spacing drops below a block. So the follow constraint has **two distinct jobs**:

- **(a) Dispatch-time density cap** — the *throughput ceiling*. Don't run more trains than fit at the
  block gap; the effective count/headway floors at the block limit and the UI reads the clamped value
  back (the existing "clamp in the core, UI reflects it" pattern, now physically grounded instead of
  an arbitrary 24).
- **(b) Move-phase dynamic follow clamp** — handles **non-homogeneous** desync: bus self-congestion
  (`tod::congestion_at`), dwell variation, and (the big one) **branches/junctions** where trains run
  different circuits and genuinely converge. Prevents a follower running into a slowed/dwelling
  leader.

---

## Phase ladder (sequence: P1 → P3 → P2 → P4 → [P5 go/no-go])

### P1 — Block following + train length  *(authority-layer foundation; contained)*
- `p`-space loop-arclength (`round = total` loop, `2*total` out-and-back; `p=s` if `dir>0` else
  `2*total−s`) — unifies loop/out-and-back; leader = next vehicle in the per-line SoA run (dispatch
  creates them in increasing-`p` order, the constraint preserves it ⇒ no sorting).
- Trains are **segments** `[head−L, head]`; follower's head holds `min_gap = brake_dist +
  SAFETY_MARGIN + leader_length` behind the leader's tail, in `p`. `L` from spec ⇒ no new state.
- **(a)** dispatch caps placed trains at block density; **(b)** move-phase clamp for dynamic desync.
- Curvature speed already applies. No Command/schema/save change.
- **Files:** `vehicle.rs` (clamp + `p` helpers), `dispatch.rs` (density cap), `trainset.rs`
  (`length_mm`). **Tests** (`tests/following.rs`, red-first): over-provisioned line caps running
  trains; consecutive separation ≥ a physical gap; replay bit-for-bit.
- **Risk:** default-parity tuning — a normal build must behave ~as today (constraint rarely binds);
  only over-provisioned/congested/branched lines show it. Measure live before commit.

### P3 — Branching lines  *(TOPOLOGY — the Circle Line + JRL fix; pulled forward)*
The data-model phase, and the only one with real **contract-mirror surface**.
- **Model: a route *tree*** (trunk = root; branches are children that diverge at a `(parent, index)`;
  branches may diverge off branches — covers JRL's Bahar 3-way and nested fingers). A tree, **not a
  general graph** (no cycles within a line ⇒ dodges the NIMBY trap; a cyclic single line is just a
  `loop_line`).
- **Service pattern lever:** which leaf-terminal each departure serves. **Default round-robin** across
  leaves; **player-settable** via `SetServicePattern` (weights/sequence) — disclosed only for
  branched lines, so the thin loop is intact for the 99% non-branched case.
- **Contract surface:** `line.rs` (tree), a Command to build a branch (`AddBranch` / `AddStop` with a
  branch target) + `SetServicePattern`, mirrored in `types.ts`/`codec.ts`; the importer
  (`build_networks.py`) captures branches instead of filtering them; `network.json`/`applyNetwork`.
- **Renderer + RAPTOR + dispatch** handle branches; dispatch assigns each train a branch per the
  service pattern; **curvature-speed extends to a turnout cap** at the divergence.
- **Interaction with P1:** branching makes the follow stream **piecewise** — one stream on the trunk,
  splitting per branch; the junction (P4) stitches the segments. That's the authority layer composing,
  not a special case.
- New hashed state: per-vehicle branch/leaf id + the service pattern.
- **Test fixtures:** the **Circle Line** (HarbourFront↔Marina Bay through Promenade) and a
  **JRL-shaped 3-way** must load, route a passenger onto a branch, and replay.

### P2 — Single vs double track  *(per-span enum + Command; the cost/capacity lever)*
- Mirrors `span_mode`/`SetSegmentMode` exactly. `track_type` per span, **default Double** (preserves
  P1). Single spans: opposing-direction mutex + **meets** at loops/stations; **cheaper to build, lower
  capacity** (plugs into the capital model).
- Hashed reservation state. **Key property test: deadlock-freedom** (deterministic occupant-wins +
  lower-index tiebreak). `SetSegmentTrack` Command, `types.ts`/`codec.ts` mirror.
- **DONE 2026-06-13** — built via understand→design→review workflows. Two corrections from the
  adversarial passes: (1) occupancy is **re-derived each tick** (sorted Vecs, no HashMap, integer),
  NOT persisted/hashed — only `Path.track_type` is hashed, so double-track replays byte-identical;
  (2) the meet protocol alone is NOT deadlock-free — a P1×P2 cycle gridlocks once trains exceed the
  line's passing capacity, so **liveness is guaranteed upstream by a dispatch single-track capacity
  cap** (doubles+1 trains; a fully-single out-and-back is a one-train shuttle), mirroring P1's
  `max_fit`. Identity-based block working (one train per single span) + terminus reservation; loops
  exempt (one-way ⇒ no meets ⇒ pure cost discount).

### P4 — Junction conflict  *(operations at branch points + at-grade crossings)*
- Mutex where branches diverge/converge (P3) and where distinct lines cross at grade (raster-cell
  crossing detection, precomputed on network change). **Train length sets the clearing time**
  (occupied until the tail passes). **Grade-separation = the fix** (reuses Elevated/Tunnel + cost).
  Legible conflict-hotspot indicator. Builds on P1's authority layer + P3's branch points.
- **DONE 2026-06-13** (P4v1 = same-line branch divergence/convergence switches; at-grade *crossings*
  between distinct lines + the shared-trunk *section* mutex stay P5 seams). Built via the
  understand → adversarial design → adversarial review workflow pipeline. The authority layer's 4th
  `min()`: Phase **B.4** in `vehicle.rs::advance` clamps a train's `ds` so it cannot cross a switch
  cluster another consist occupies. A consist occupies the cluster while its `[head−dir·len, head]`
  segment overlaps the cluster's per-path `[lo,hi]` span (half-open `group_overlap`), re-derived each
  tick (sorted Vecs, `occ_claim`/`occ_owner`/`try_claim`, no HashMap, integer) — **never hashed**, so
  a non-branched network is **byte-identical (zero re-pins)**. `world.junctions` (also unhashed) is
  derived in `dispatch.rs` on `dispatch_dirty`. Clearing time ∝ `length_mm` falls out for free
  (longer consist straddles the switch for more arclen). **Grade-separating a branch does NOT dissolve
  the mutex** — a same-line switch is a switch at any level (tested).
- **Coalescing is the load-bearing liveness fix** (both design adversaries found the same break): two
  switches within one consist-length on the trunk form a 2-cycle deadlock under a naive point-mutex
  (A holds J1 + gated at J2, B holds J2 + gated at J1 — and the denial arm is index-independent, so
  the replay gate is silent). Merging them into **one atomic group** (key = `min` member StationId,
  command-order-independent) collapses the cycle: one consist straddles ≤1 group, every contender for
  any member point contends for the one key ⇒ the wait-for graph is an **acyclic depth-1 forest**.
- **Two corrections from the build/review vs the locked design** (both validated by RED-first tests):
  (1) the design's **§4.3 dispatch cap was dropped** — a branch switch is a POINT crossing occupied
  only ~`length_mm` of travel, so its throughput dwarfs P1's per-path block density (`max_fit` always
  binds first), and the mutex is deadlock-free by coalescing, so over-provisioning merely *queues* at
  the gate; the block-sized cap the design implied would cripple every branched line to ~2 trains
  (a `dense_*` test pins a real fleet running). (2) The design's **Phase B.5 junction no-rest extension
  was unnecessary** — B.4's gate-crossing test uses `s <= gate` (a train almost always *departs* the
  junction station, which sits ON the gate, so `s == gate` is the dominant case; strict `<` would
  never bind), and with start-of-tick occupancy denying entry while occupied, a non-owner can never
  enter — let alone rest — inside a cluster. A **dispatch snap** (added, not in the design) keeps the
  switch collision-free from tick 0 (a placement straddling a cluster snaps to the gate; verified
  load-bearing by sweep — dense early junctions straddle without it).
- **Adversarial review caught two real bugs (the determinism gate is blind to both — they replay
  green):** (a) **CRITICAL — coalescing axis** (Residual Risk #2, realised): the original rule
  coalesced on the **trunk** gap, but the runtime mutex keys on **per-path** spans, and a branch
  path's smoothed shared-prefix arclen can be *shorter* than the trunk's (Catmull-Rom pulls the branch
  straight while the trunk bows toward its post-junction stop). Two switches >`len` apart on the trunk
  but <`len` on a branch stayed split → a branch consist straddled both → the very 2-cycle gridlock
  coalescing exists to kill (reproducible with ordinary smoothed geometry). **Fixed:** coalesce on the
  **MIN gap over shared paths** (the tightest mutual-reach bound, matching the mutex). (b) **P5 seam,
  deferred — single-track on a branched line's SHARED TRUNK:** P2's meet keys per `(line, path, span)`,
  so the trunk path and a branch path get different keys for the *same physical trunk rail* and don't
  mutex (opposing consists pass through). Pre-existing P2×P3 (P4 guards the divergence point, not the
  single-track span into it); a correct fix is the P5 shared-track phase — key the meet on the physical
  trunk span **plus** a cross-path liveness cap (a half-fix turns the pass-through into a *worse*
  deadlock). Captured as `#[ignore]`d `shared_trunk_single_track_no_headon_is_p5`.
- **Tests** (`tests/junction.rs`, red-first): mutual exclusion (Y + JRL 3-way); coupled-junction
  no-deadlock (RED without coalescing); **branch-coupled junctions coalesce + run** (RED with
  trunk-only coalescing — bug a); dense early junction + dense loop+spur clean from dispatch (the
  snap); single train not self-gated; grade-sep invariant; determinism + command-order-stable keys;
  non-branched parity; `#[ignore]`d P5 shared-trunk seam (bug b). **Deferred** (seams, logged):
  at-grade line *crossings* + shared-trunk *section* mutex incl. single-track-on-shared-trunk (P5
  go/no-go); a **turnout speed cap** at divergences (`speed_cap_at` seam, add only if a junction
  visibly binds); the optional `LineView.junction_points` amber-dot readout.

### P5 — Shared track: the road to first-class track objects  *(GO 2026-06-13 — a product driver)*
**Full roadmap: [p5-shared-track-roadmap.md](p5-shared-track-roadmap.md).** The original "do not build
speculatively" deferral was lifted by an impending fork into a transport-builder game where distinct
lines sharing physical track (a central tunnel, OpenTTD-style networks) is a **headline feature** — so
it is no longer speculative. Built **incrementally, foundation-first**, around one reusable primitive:
a **physical-block reservation** (the proven `occ_claim`/`group_overlap` machinery), where only the
block's *identity key* graduates from line-scoped to cross-line per layer.
- **S1v1 — cross-path single-track cap (DONE 2026-06-13).** Fixes the reachable shared-trunk head-on (a
  branched line single-tracked on its shared trunk — the trunk + a branch path are two polylines on one
  physical rail, and P2's per-`(line,path,span)` meet never mutexes them). A *fully-single* shared
  trunk deadlocks 2 trains even with a perfect mutex (no passing place), so the load-bearing fix is the
  **cap**: all paths traverse the shared trunk, so its single-track capacity bounds the WHOLE fleet — cap
  total trains across trunk + branches to (physically-double shared-prefix spans)+1 (1 ⇒ a shuttle, the
  trunk wins). Pure `dispatch.rs` clamp, re-derived, **inert unless** the universally-shared prefix
  `[0, min diverge_at)` has a physically-single span ⇒ **zero re-pins**. Un-ignored the captured head-on
  test; the single-span-in-a-double-trunk case is `#[ignore]`d → S2.
- **S2 — physical-block meet mutex (DONE 2026-06-13, the reusable primitive).** A single span *between
  passing places* needs a meet MUTEX, not the cap. Single shared-trunk spans become first-class
  **window-blocks** in the dispatch junction set (per-path `(path, lo<hi)`; single-if-any track read);
  the existing A.1.5/B.4 `group_overlap` mutex serialises them unchanged. Coalescing is generalised
  points→windows (`q.lo − p.hi`), so a single approach **folds into its adjacent switch** (kills the
  P5×P4 cycle) and contiguous singles merge; non-contiguous singles stay standalone. The cap drain
  became **round-robin** so the branch runs and MEETS (trunk-takes-all starved it to 0) — the two
  halves ship together. RED-first meet + non-contiguous tests; zero re-pins. This is the kernel
  cross-line track sharing reuses (only the block key graduates line-scoped → `TrackSegmentId`).
- **Track objects — the cliff (after S2).** `TrackSegment` first-class; `Line` references segments; the
  block key becomes a cross-line `TrackSegmentId`; the genuinely new hard part is **cross-line
  deadlock-freedom via resource-ordering** (single-owner coalescing can't collapse multi-line cycles).
  Routing extends to the track graph. **Each layer ships green and standalone — no half-built cliff.**

---

## Cross-cutting
- **Determinism:** P1(+length) adds **no** state; P2/P4 add hashed occupancy/reservations
  (index-ordered, integer); P3 adds the branch tree + per-vehicle branch id + the **only real
  contract mirror** (`command.rs`↔`types.ts`/`codec.ts`). All keep same-seed-same-log replay green.
  Expect numeric **re-pins** in `vehicle.rs`/`ridership.rs`/`pressure_buckets.rs` as motion timing
  shifts; routing tests (`raptor.rs`/`access.rs`) are unaffected (RAPTOR costs use headway + arc-length,
  not live positions, `raptor.rs:76`).
- **Tests are red-first per phase** (AGENTS TDD): the invariant lands failing before the code.
- **The clamps demote** from capacity model to safety backstops (buffer sizing) as phases land.
- **Cost:** P1/P2 contained; **P3 is the data-model investment** (and fixes a visible bug); P4
  moderate; **P5 is the cliff** (separate decision).
</content>
</invoke>
