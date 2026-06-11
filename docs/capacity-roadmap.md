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

### P4 — Junction conflict  *(operations at branch points + at-grade crossings)*
- Mutex where branches diverge/converge (P3) and where distinct lines cross at grade (raster-cell
  crossing detection, precomputed on network change). **Train length sets the clearing time**
  (occupied until the tail passes). **Grade-separation = the fix** (reuses Elevated/Tunnel + cost).
  Legible conflict-hotspot indicator. Builds on P1's authority layer + P3's branch points.

### P5 — Shared-track merges between distinct lines  *(track graph + interlocking — GO/NO-GO after P4)*
- The architectural cliff: distinct lines over one physical edge ⇒ a shared `TrackGraph`. Kept as a
  **named seam** (the authority layer's `shared_block_limit` slot). P1–P4 deliver the full
  capacity-as-buildable fantasy without it. **Do not build speculatively.**

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
