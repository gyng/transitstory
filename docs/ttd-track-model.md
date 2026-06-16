# TTD track model — first-class track objects + multi-platform stations

**Status:** PLANNING (owner-directed 2026-06-16). Supersedes the "FULL model — the cliff" section of
[p5-shared-track-roadmap.md](p5-shared-track-roadmap.md) with a concrete, green-at-every-commit layer plan.
**Owner decisions (2026-06-16):** go **full track-objects model** (not the pragmatic reskin) **+
multi-platform stations** (parallel berths) to kill the single-cell dwell jam.

## Why now / the product driver

The fork is becoming a transport-builder. The headline interaction the owner wants is the **OpenTTD
flow**: *build track (infrastructure) → create lines (services over track) → assign & customise
trainsets*. Two coupled problems block it today:

1. **Track is welded to the line.** A `Line` owns its `stops → Path.polyline`; "draw line" emits
   `CreateLine` + `AddStop`s in one gesture and rebuilds geometry from the stops. There is no standalone
   track. (Grid lines that snap to the same cells *do* already share rail via the `edge_key` cross-line
   mutex — the seed of the graph, but keyed off line-owned polylines.)
2. **Stations are a single point ⇒ a dwell jam.** A station is one `PointMm`; a train dwells ~700 ms; a
   follower must hold the P1 braking-block gap (~260 m) behind it; on single track they can't pass, so
   they pile up. Real parallel berths need a train to *route* to a free platform — which needs the graph.

So both wants land on the same foundation: **make track first-class**, and **stations graph nodes with
platform berths**. This is the largest model change in the project; it ships in **layers, each green and
testable**, never a half-built cliff (AGENTS).

## The end-state

- **`TrackSegment`** is a first-class object: its own geometry (the grid edge run between two graph
  nodes), per-segment track type (single/double) + signal state, and the physical-block identity.
- **`TrackNode`** is a junction **or a station**; a station node owns **N platform berths** (parallel
  segments through the node).
- **`Line`** references an ordered route — a `Vec<TrackSegmentId>` (+ which berth it calls at per station)
  — it no longer *owns* geometry; any line may use any segment.
- **Vehicles route over the track graph** (YAPF-style segment choice), reserving a **physical block** by
  `TrackSegmentId` (cross-line) with the proven primitive.
- **Signals/junctions/shared depots emerge from the graph**; the player draws track + places signals.

## The one reusable primitive (unchanged across every layer)

A train's per-tick advance is a `min()` over resource limits; a **physical block** is held by ≤1 consist
at a time via the `occ_claim`/`occ_owner`/`try_claim` sorted-Vec machinery (`vehicle.rs`, proven for P2
single-track, P4 junctions, and Phase-2 cross-line). **Only the block's identity KEY graduates** per
layer (`(line,path,span)` → `(line,key_station)` → `edge_key` → **`TrackSegmentId`** → **`(station,berth)`**).
The mechanism is constant; this is why each layer is a small extension, not a rewrite.

## The genuinely hard part (settled by the Phase-2 review, do not relitigate)

Cross-line deadlock-freedom is **NOT** resource-ordering ("acquire segments in global id order") — that
was disproven: opposing trains traverse the same segments in opposite order, so a total order constrains
nothing ⇒ a gate-blind 2-cycle (replays green). The proven discipline, extended from Phase 2:

- **Atomic reservation of the whole path to the next safe waiting point (PBS)** — a train can't enter a
  shared section unless it can reserve a clear run to a place it can wait *outside* the section ⇒ a waiter
  never holds a resource a mover needs ⇒ the wait-for graph is an acyclic depth-1 forest.
- **+ a cross-line capacity cap** per shared block, **+ a global mutex on cyclic shared components**,
  **+ a fair aging/round-robin tiebreak** (else the lowest `LineId` starves the others).
- **Liveness is never inferred from replay-equality** (a deterministic deadlock replays green): **every
  layer ships a RED-first never-freeze property test.**

## Determinism discipline (every layer)

Re-derive blocks per-tick (sorted `Vec` + binary search, integer, **no HashMap iteration**); nothing new
enters `Canonical` until a layer genuinely needs hashed segment state; same-seed-same-log replay stays
green at every commit; new hashed state ⇒ a deliberate, documented golden re-pin (behaviour-justified).
Both goldens: `GOLDEN_TRANSIT_HASH`, `GOLDEN_ARCADIA_HASH`.

## The layer plan (each ships green + testable, in order)

### L1 — TrackGraph as a *derived, additive* structure (foundation, zero re-pins)
Promote what `dispatch::derive_cross_blocks` already computes into a first-class **derived** `TrackGraph`
(nodes = stations + grid junction cells; segments = maximal grid-edge runs between nodes), rebuilt from the
existing line polylines, **never hashed**. Expose it through a render copy-out so the network draws as
shared **infrastructure** (a shared corridor reads as one fat rail, not N stacked line-ribbons). No
Command, no `Canonical` change, no behaviour change ⇒ **goldens byte-identical**. This is the spine every
later layer keys off, landed with zero risk.

### L2 — Multi-platform stations (the jam fix), berth = a block key
A station node gains a **platform count K** (a buildable footprint of cells; default **K = 1 ⇒
byte-identical**, goldens neutral until a player builds K>1). New `Command::BuildPlatforms{station,k}`
(mirrored in `types.ts`/`codec.ts`). A berth is a block keyed `(station, berth)`; an arriving consist
claims a **free** berth via the primitive and dwells there; up to K consists dwell in parallel, so a
follower takes another berth instead of holding the block gap. RED-first never-freeze + a
"K berths ⇒ K parallel dwells, no follow-queue" property test. (Parallel-berth *routing* for one line's
trains arrives with L4 routing; L2 already unblocks multi-line and terminus berthing.)

**Length coherence + exit direction (owner, 2026-06-16).** Platform length and train length live in the
SAME units so the fit is legible:
- **Cabin = cell-step ÷ 4** (≈ 4 cabins per hex), *derived from `grid_cell_mm`* — so a cabin/car is a fixed
  fraction of a cell on any map, never a hardcoded metre count (the render pre-rescale shipped this: see
  the render note below). `cell_step_mm = center_of((1,0), cell).x_mm` (√3·size).
- **Train length = (1 loco + N cars) · cabin**; `train_cells = train_len / cell_step`. Standard (loco+3) ≈
  1 cell; Heavy (loco+5) ≈ 1.5 cells; so a model's consist maps to a platform-length requirement.
- **Platform length is in CELLS** and the buildable constraint is **`platform_cells ≥ ceil(train_cells)`** —
  a train longer than its platform OVERHANGS: it can't fully berth (won't release the approach block / loads
  slowly), the informative pressure to lengthen the platform (a real TTD lever, sits alongside K berths).
- **Exit direction = the platform segment's orientation.** A berth is a *directed* slot along its track
  segment: a through-platform admits from one end and releases at the other (the consist clears forward); a
  terminus platform reverses in place. L4 routing picks a berth whose geometry matches the train's heading.

### Render rescale (pre-L2, shipped 2026-06-16): trains derived from the cell
The 3D train scale stopped being a hardcoded `VEHICLE_SCALE = 150` (≈3 big cabins/hex on the 250 m fantasy
cell, absurd on the 100 m test cells) and is now **derived from the map's cell** (`cabin = cell_step ÷ 4`),
both the frontend mesh scale (`render.ts`) and the sim car-pitch (`render_buf.rs CAR_PITCH`) in step, so a
consist reads as ~4 small cabins per hex on any map — the proportions L2's platform-length constraint needs.

### L3 — `TrackSegment` becomes *authoritative* geometry (the model change)
Flip ownership: segments own geometry; `Line` references `Vec<TrackSegmentId>` (+ berth per call).
`AddStop`/waypoint edits are re-expressed as "lay/extend segments + bind the line to them." The importer +
editor build/edit segments. Cross-line block key graduates to `TrackSegmentId`. This is the re-pin-likely
layer; it ships with the PBS reservation + cross-line cap + cyclic mutex + aging tiebreak and the
multi-line never-freeze test. Geometry rendering moves to per-segment.

### L4 — Vehicle routing on the graph + parallel-berth choice
Vehicles choose a route over segments (YAPF-style, deterministic least-cost with a fair tiebreak) and
pick a **free** platform berth at each station — so one line's trains genuinely overtake into parallel
berths. RAPTOR/pax routing extends to the track graph.

### L5 — Placeable signals + junctions as track objects
Player-placed signals as first-class segment objects; junctions emerge from segment topology. Block
boundaries become signal-defined (PBS = path-block-signal).

### L6 — The TTD interaction model (the player-facing flow)
Two-gesture thin-loop guard (AGENTS): **draw track** (a Track tool laying segments + stations), then
**assign a line to track** (pick endpoints/route over existing segments), then the existing **assign
trainset + headway**. Sharing is **emergent** from co-located segments (no separate "share" mode). The
build HUD splits into Track / Line / Stock steps; Build/Run wall + one-Command-per-edit + undo=replay all
preserved. testids preserved/extended.

## Sequencing note

L1 → L2 deliver the **most-felt value early** (the network reads as infrastructure; the dwell jam eases on
multi-line/termini) with **zero/again-near-zero re-pins**, while laying the keying spine. L3 is the cliff
proper (re-pin, PBS liveness). L4–L6 complete routing + signals + the interaction. Each layer is revertible
and independently green. The interaction reskin (L6) is intentionally LAST so it dresses a working model,
not a moving target.
