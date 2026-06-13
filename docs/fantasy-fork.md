# Fantasy Fork — shared engine, split game (design doc)

> **Status:** design, not yet built. Produced by a survey → design → adversarial-critique →
> synthesis workflow grounded in reads of the live repo. This doc pins the architecture and the
> **determinism guardrails** before any code lands. Where it contradicts an instinct to "just make
> `World` generic" or "spin a second crate," **this doc wins** — read §4 (the one dangerous step)
> first. Sibling to [PLAN.md](../PLAN.md) and the [capacity roadmap](capacity-roadmap.md); obeys
> every non-negotiable in [AGENTS.md](../AGENTS.md).

## 0. The idea

Fork the deterministic transit sim into a **second game** in the *same repo, behind the same
entrypoint*: a TTD/Factorio-flavoured **fantasy logistics** mode (isekai-as-transit-engineer) —
sources (raw materials, people) → sinks (factories, apothecaries, towns), supply chains, organic
town growth where supplied. **Real mode** (today's transit game on OSM) and **fantasy mode** are a
toggle. **Share the engine, split the game.**

## 1. The decision

**Fantasy is the `globe` writ large.** One pure `crates/sim`, one `cdylib`, one entrypoint. Mode is
a **tag on `CityData`** that selects deterministic trait objects inside `World::new`. No second
crate, no second cdylib, no new dependency, no pin change.

This won a scored, adversarially-reviewed comparison against two alternatives:

| Architecture | Verdict | Why |
| --- | --- | --- |
| **Ruleset-at-construction** (chosen) | **8.6** | Lifts the *proven* `Box<dyn Router>` seam one level up; ~zero churn to the locked transit slice; one membrane. |
| Ruleset-as-save-citizen | 8.4 | Same architecture; its save-tuple guard is **merged in** (see §3). |
| Concentric 3-crate split (`engine` + `game-transit` + `game-fantasy`, dual cdylib) | 6.8 | Cleanest boundary (10/10) but the **hard work is identical** (still must carve `coverage_score`/`line_cost_metrics`/`tick_economy` out of `world.rs`); the crate move + generic `World<R>` + dual-cdylib CI matrix + 3-package frontend is pure tax given the locked single-cdylib / single-entrypoint rules. |

The crate split is a *later* promotion path, not the starting point — and only if fantasy ever
diverges radically. We graft its one real win (game state can't leak into the kernel) at trait cost
(§5, decision B).

## 2. The seam

`World` already holds a deterministic trait object that replays bit-for-bit because **its output is
hashed, not the object**: `router: Box<dyn Router>` (`crates/sim/src/world.rs`, defaulted to
`RaptorRouter` in `World::new`). `routing/mod.rs` documents the contract we copy verbatim:
*"Implementations MUST be deterministic (index-ordered iteration only) — the determinism gate."*

Lift that one level up:

```
World {
  router:  Box<dyn Router>,    // EXISTS — trip planning
  demand:  Box<dyn Demand>,    // NEW — gravity | agents | supply-chain   (prepare / spawn / grow)
  ruleset: Box<dyn Ruleset>,   // NEW — score / line_cost / validate / after_apply / id / tick bodies
}
```

- `crates/sim/src/ruleset/mod.rs` (NEW) defines `trait Demand` and `trait Ruleset` with the **same
  index-ordered determinism contract** as `Router`. Lives *in* the pure core, so the inward
  dependency edge and the pin set are unchanged.
- `ruleset/transit.rs` (NEW, **DEFAULT**) = `TransitRuleset` + `GravityDemand` + `AgentDemand`. It
  *moves* (does not rewrite) today's `coverage_score`, `line_cost_metrics`, economy, and
  gravity/agent demand bodies behind the traits. The `agent_demand` bool (`tick.rs`) is absorbed as
  a sub-mode here — de-souping `tick.rs`.
- `ruleset/fantasy.rs` + `demand/supply_chain.rs` (NEW) = `FantasyRuleset` + `SupplyChainDemand`.
  Sibling files the transit path **never imports**. Fantasy hashed state is **ruleset-owned**, not
  bolted onto `World`.

## 3. The mode toggle & save contract

**The mode is frozen at construction — it is NOT a Command and NOT a wasm setter.**

- `ruleset` rides in on `CityData` as `#[serde(default = "…transit")] pub ruleset: String`.
  `CityData` already carries a wall of `#[serde(default)]` fields and `tests/city.rs` proves unknown
  fields are ignored, so every committed manifest parses unchanged.
- The save tuple becomes `SaveGame { seed, ruleset, commands }`. `replay()` (`world.rs`) asserts
  `city.ruleset == save.ruleset`. A fantasy log fed to the transit decoder is **rejected at the
  boundary** (thrown JS error surfaced in the EditorPanel), never silently mis-replayed. **Disjoint
  save universes.**
- Cross-mode Commands are gated by `Ruleset::validate` **before `cmd_log.push`**. Today
  `World::apply` pushes *every* command to `cmd_log` unconditionally — so the gate must come first,
  or a rejected fantasy command would pollute a transit save (deterministic, but unclean).

**Why this is determinism-safe (and why `SetDemandMode` is allowed to be a live command but a
ruleset is not):** the trait object is never hashed — only its mutation output reaches the
fixed-order `Canonical` view (`world.rs`, hashed via `fnv1a(postcard::to_allocvec(&Canonical))`).
The rules are fixed before `command[0]`. `SetDemandMode` (gravity↔agents) *is* a live command **only
because** it flips deliberately-**unhashed** spawn behavior ("isn't hashed; the trips it causes
are"). A whole-game ruleset changes the hashed field *set/shape*, so it **must** be
construction-frozen — a mid-log rules change would invalidate "same seed + same log ⇒ same hash."

## 4. ⚠️ The one dangerous step — the determinism carve

Moving `demand` / `coverage_score` / `line_cost_metrics` behind the traits (Step 2) must keep
transit's hash **byte-for-byte identical**. The trap:

> **`tests/determinism.rs` only proves `run == run` within one process — NOT `run ==
> committed-bytes`.** A uniform hash shift (from reordering `Canonical` fields, or hand-rolling a
> hasher) would break **every saved game** and **pass the existing gate silently.**

Mandatory mitigations, landed **before** any refactor (Step 0):

1. **Golden-hash literal** — run `sample_log()` + 600 ticks once, paste the resulting `u64`
   constant, assert it. The hex must be *unchanged* after Step 2 (diff it).
2. **Committed save artifact** — a real pre-refactor transit save, asserted to replay to that hash.
3. **No hand-rolled hasher** — `state_hash` stays `fnv1a(&postcard::to_allocvec(&Canonical)?)` with
   byte-identical field order. (The "iterate slices into a hasher" idiom is exactly the drift to
   avoid.)

Two more determinism gotchas baked into the plan:

- **Fantasy must draw from a keyed RNG sub-stream** (`seed ^ const`, the `agents.rs` pattern),
  **never `world.rng`** — or it perturbs the shared draw order and breaks *transit* replay.
- **`coords/geo.ts` stays the one coordinate crossing.** The fantasy world is an offline bake into
  the same `i64` mm `DemandGrid`/`BuildabilityGrid` with a *synthetic origin* (the `globe` board
  uses `originLngLat [10,35]`). A bake-time assertion forbids any lng/lat or raw-pixel coord —
  including supply-graph node coords — from reaching a Command. (Matches the
  [[dynamic-city-architecture]] "offline bake frozen into the save" rule.) **For a GRID world (§10)
  the bake additionally snaps every vertex to the `cell_size_mm` lattice and asserts alignment;
  `geo.ts` collapses to a trivial integer scale+offset but does not vanish (MapLibre's camera still
  needs a lng/lat frame).**

## 5. Architecture layout

**`crates/sim`** (pure core, deps unchanged, zero wasm):
- Keeps the engine spine: `tick::step` ORDER, the `apply()` shell (decode → mutate → `cmd_log.push`
  → mark-dirty → return `Event`s), `state_hash`/`Canonical`, `SaveGame`/`replay`, ChaCha8 seeding,
  the `id_type!` macro, `geo_local::PointMm`, `VehicleSoA` + `advance`, the dispatch clamp, the pax
  token-flow (`board_alight`/`renege`), `trait Router`, `render_buf` helpers, `stats` shape.
- **Gains** `demand: Box<dyn Demand>` + `ruleset: Box<dyn Ruleset>` on `World`, selected by
  `CityData.ruleset` in `World::new`. New files: `ruleset/{mod,transit,fantasy}.rs`,
  `demand/supply_chain.rs`. No new external dep, no new crate.

**`crates/sim-wasm`** (the ONE cdylib, mode-blind): the four genre-agnostic verbs
(`new`/`apply_command_json`/`tick`/`state_hash`) are already game-opaque and stay untouched. The
~16 transit-named read accessors (`vehicle_positions`, `peep_*`, `stations_view`, `lines_view`,
`station_od`, … `preview_line_cost`) are the leak risk — they collapse into **mode-agnostic SoA
buffers (`render_entities()`) + one generic `query(kind, args)` dispatched *inside* `crates/sim`**,
so `lib.rs` never branches on mode. A CI grep gate forbids a ruleset/mode string-match in `lib.rs`.
`wasm-bindgen=0.2.117` pin unchanged.

**Frontend — shared spine (untouched):** `main.tsx` (single `createRoot`, no `StrictMode`),
`App.tsx` phase machine, the two-slice/two-cadence `GameContext`, fixed-timestep `GameLoop` with
alpha, `SimBridge` (gains a ruleset-mismatch rejection on load), and `coords/geo.ts`. `boot()` gains
one branch on `entry.kind`.

**Frontend — per-mode:** factor `game.ts` → `GameCore` (overlay `setProps`, `composeAndSet`,
`onChange` fan-out, undo/redo, shared render-lifecycle helpers) + `TransitGame` / `FantasyGame`.
`render.ts` exposes the **layer discipline** (reused `Float32Array` with buffer-growth re-acquire,
stable data identity, `updateTriggers`-on-topology, the binary peep layer) as shared helpers **both**
modes call; `fantasy/renderFantasy.ts` supplies only the fantasy layer *set* (sources/sinks/
supply-routes/town-growth heat) reusing those helpers + the same deck.gl building blocks.

**Registry & map:** `cities.ts` `CityEntry` gains `kind: 'real' | 'fantasy'` — the presentation
mirror of the manifest tag; all scattered `entry.id === 'globe'` compares collapse into this typed
flag in one pass. `basemap.ts` `createMap` gains a **required, fail-safe-ON** `attribution` param
(OSM `AttributionControl` omitted only when `kind === 'fantasy'`).

**Data:** committed manifests under `packages/app/public/data/`. Real cities carry
`ruleset:"transit"` (omittable). Fantasy worlds (e.g. `arcadia_world.json`) are an offline bake
carrying `ruleset:"fantasy"` + a synthetic origin + an additive `supply_graph` field (mm coords,
consumed *only* by `SupplyChainDemand`).

## 6. Migration ladder

Steps 0–4 are **behavior-preserving and independently shippable** — they de-soup `tick.rs` and
formalize the `globe` smell, valuable *even if fantasy never lands*. That is how this honors the
"never half-built" rule (AGENTS non-negotiable #8): transit is feature-complete behind a default
ruleset before any fantasy code exists.

- **Step 0** — Add the `ruleset` tag to `CityData`/`SaveGame`/`CityEntry`/the frontend save blob
  (defaulting to transit/real). **Land the golden-hash + committed-save regression test first**
  (§4). No behavior change. Full native suite + e2e green. Commit.
- **Step 1** — Create `ruleset/mod.rs` (`trait Demand`, `trait Ruleset`, determinism contract copied
  from `routing/mod.rs`). Add the two boxes to `World`, default-constructed beside `router`, **not
  yet called**. Golden-hash + replay-equality byte-identical. Commit.
- **Step 2** ⚠️ — The carve. *Move* (don't rewrite) `demand::{prepare,spawn,grow}`,
  `coverage_score`, `line_cost_metrics` behind the traits; fold the `agent_demand` if/else into the
  `Demand` trait so `tick.rs` becomes `ruleset.demand().grow/prepare/spawn`; trait-ify the `apply()`
  trailer (`demand::prepare(self)` → `ruleset.after_apply(self, cmd)`). Keep `Canonical` field order
  byte-identical. **Hash hex diff must be zero.** Commit only then.
- **Step 3** — `World::new` matches `city.ruleset`; `replay`/`save` carry & assert `ruleset_id`;
  `validate` gates **before** `cmd_log.push`. `tests/ruleset.rs`: round-trip + cross-mode command
  rejected **and** absent from the log. Transit-only still. Commit.
- **Step 4** — Extract the read surface (`render_entities()` SoA + generic `query`) so
  `sim-wasm/lib.rs` has no mode branch (+ CI grep gate). Factor `game.ts` → `GameCore` +
  `TransitGame`; extract `render.ts` lifecycle into shared helpers; replace all `entry.id ===
  'globe'` with `kind`. Vitest wasm smoke + Singapore e2e green. **Transit fully migrated, behavior
  unchanged — safe to stop here.** Commit.
- **Step 5** (fantasy, RED-first, complete-or-nothing) — Tests first: `fantasy_demand.rs`
  (source→sink token flow ≤ source weight, reusing the catchment no-double-count invariant),
  `fantasy_growth.rs` (supplied town grows > starved), `fantasy_score.rs` (its **own** monotonicity
  invariant), a **ticked** replay test (apply log + tick N + `state_hash` twice), and a fantasy
  golden-hash. Extend the `f32`/HashMap-iteration grep ban over the fantasy files. Add fantasy
  Command variants (`PlaceSource`/`PlaceSink`/`BuildRoute`/`SetRecipe`) to `command.rs` +
  `contract.rs` (partitioned tag sets) + `types.ts` + `codec.ts` in **one** commit. Implement
  `SupplyChainDemand` (keyed RNG sub-stream) + `FantasyRuleset` until every RED test passes and the
  fantasy replay gate is green twice-in-one-process.
- **Step 6** — Bake + commit `arcadia_world.json`. `boot()`/`basemap.ts` `kind` branch (procedural
  layer; attribution off only for fantasy). `renderFantasy.ts` + `FantasyGame`. e2e: a cart's
  position changed between two reads **and** an output counter rose **and** the fantasy gauge moved
  **and** layer instance identity is stable across N frames. Register the `kind:'fantasy'` menu
  entry only once the Step-5 tier is green. Log the mode axis + every seam in `PROGRESS.md`.

## 7. First fantasy slice (proves the split end-to-end, writes almost no new code)

One **ore source**, one **smithy/town sink**, one **cart**, one **route**. Bake `arcadia_world.json`
(`ruleset:"fantasy"`, synthetic origin) with exactly two high-weight demand cells — a SOURCE
(high `origin_w`) and a SINK (high `dest_w`). The player places one source node, one sink node, draws
one supply route (a `Line`, mode = a "cart" spec appended append-only), assigns one cart + headway.
On Run:

- `SupplyChainDemand::spawn` emits one commodity token (a `Pax` with `citizen_id` repurposed as
  commodity-type) into `waiting[source]`.
- The **existing** `RaptorRouter` plans source→sink over `serving`; `VehicleSoA::advance` moves the
  cart; `pax::board_alight` consumes the token at the sink. **Zero new motion/routing/token code —
  only a new `Demand` impl.**
- `SupplyChainDemand::grow` (reusing the day-boundary scheduler in `demand::grow`) raises the sink
  cell's weight when fed — **the town grows where supplied**, not by transit catchment, capped like
  `growth_cap_w`.
- `FantasyRuleset::score` returns a monotonic "supply satisfied" gauge (delivered / demanded).

Proves: (1) the tag selects fantasy behavior at construction and replays bit-for-bit; (2) the same
`Pax`/`waiting`/`Router`/`VehicleSoA`/`board_alight` spine carries cargo unchanged; (3) growth is
supply-fed; (4) a fantasy command on a transit world is rejected and unlogged. No multi-commodity,
no recipes, no procedural terrain — just the disjoint-save-universe + behavioral divergence, end to
end through one entrypoint.

> **"Almost no new code" holds for *line-owned* track (§10, A1) only.** If the grid game commits to
> **shared physical rail** (A2 = the deferred P5 `TrackGraph`), that is a deliberate later phase with
> real cost — it is *not* part of this first slice.

## 8. Locked sub-decisions (defaults; revisit only on real need)

- **Reuse transit's `Canonical`** for the first slice (carts = `VehicleSoA`, delivered-tokens = a
  spare `u64`) → Step 5 needs **zero** `state_hash` surgery. Add a per-mode `Canonical` only when a
  genuine multi-commodity inventory can't be expressed by existing hashed fields — and only via an
  explicit migration step that keeps transit's field order byte-identical under the golden-hash test.
- **Fantasy state is ruleset-owned**, not on `World` (which is already a 1321-line catch-all
  AGENTS warns against). Clean promotion path to generic `World<R>` if it ever warrants it.
- **Shared `Command` enum**, `validate`-gated, `contract.rs` tag set partitioned. Split per-game
  only if/when you also adopt the full crate split (deliberately deferred).
- **Fantasy basemap = deck.gl-drawn terrain layer** (no tile pipeline; attribution trivially
  real-mode-only). Baked raster tiles are an art upgrade later — re-mount ODbL attribution if any
  tile derives from OSM.
- **Minimal `GameCore`** in Step 4 (only what `FantasyGame` needs + shared render-lifecycle
  helpers). A full `game.ts` teardown is not required to prove the split and risks the working
  transit e2e.

## 9. Invariants this fork must hold (checklist)

- Mode enters via `CityData`/`SaveGame` at construction; **never** a Command, **never** a wasm
  setter · cross-mode command rejected **before** `cmd_log.push` · `replay` asserts
  `save.ruleset == city.ruleset`.
- Trait objects (`Demand`/`Ruleset`) are index-ordered/deterministic; only their *output* is hashed
  · `state_hash = fnv1a(postcard(Canonical))` with unchanged field order · golden-hash + committed
  save artifact green at every step from Step 0 · hash hex diff zero through the Step-2 carve.
- Fantasy uses a **keyed RNG sub-stream**, never `world.rng` · no `f32/f64` in hashed fantasy state
  · no HashMap iteration · `coords/geo.ts` remains the one coordinate crossing; no lng/lat or pixel
  coord reaches a Command.
- `sim-wasm/lib.rs` has **no** mode branch (reads via `render_entities()` + generic `query`) · the
  cdylib stays single · pins + both lockfiles unchanged.
- Fantasy ships **complete-with-tests or not at all** (determinism + monotonic score + behavioral
  e2e); its menu entry is unregistered until that tier is green · Steps 0–4 leave transit
  byte-identical.
- OSM `AttributionControl` mounted for every real-mode board (`createMap`'s `attribution` param is
  required, fail-safe ON).
- **(Grid mode, §10)** grid `Path`s are built crisp/un-smoothed with `samples=1` (the existing
  `literal` flag's `LITERAL_SAMPLES=2` still rounds corners — not crisp enough) · all grid vertices
  land on the `cell_size_mm` lattice · the grid-geometry mode is **additive behind a new flag**,
  never an edit to the default smoothing path · a fresh `run()==run()` replay test covers a
  grid-built line · `track_type`/signals are **not** relocated to a shared track object without a
  golden-hash re-pin + `types.ts`/`codec.ts` mirror.

## 10. Grid posture (geometry substrate)

> Added after a survey → 4-way code verification → adversarial → synthesis workflow that asked: *is
> the engine reusable if the fantasy game is **grid/tile-based**?* **Verdict: yes, almost
> wholesale.** The grid decision is **orthogonal** to the ruleset/Demand seam in §1–§3 — grid is a
> geometry-**input** + render-substrate concern, *not* a kernel concern. The ruleset-at-construction
> architecture is unchanged; grid is selected by the same `CityData` tag.

**The key fact:** grid is **a lattice constraint over the continuous `i64` mm world**, not a
different engine. Every load-bearing computation keys off 1-D **arc-length (`s_mm`) + integer
span/path indices**, never `(x,y)` shape — verified in code:

- P1 block-follow (`vehicle.rs:398-405`), P2 single/double meet (`seg_key(line,path,span)` +
  `strictly_inside`), P4 junction mutex (`junc_key` + `group_overlap` over per-path arclen spans) —
  **all arclen; zero shape dependency.**
- `(x,y)` is only ever *render output* (`point_at(new_s)`/`heading_at` write the SoA *after* the
  authority layer decides `new_s`) or a buildability-raster lookup. The only geometry-derived motion
  *input* is the curve-speed cap, which reads a precomputed arclen-indexed array.

### Posture A (chosen) vs Posture B (rejected)
- **A — lattice over continuous mm.** Tile = `cell_size_mm`; grid track is a `Path` whose vertices
  land on the lattice (crisp/octilinear); vehicles keep continuous `s_mm` and **glide** via the
  two-clock alpha-interp. P1–P4 transfer because they key off arclen/spans. *This is how
  OpenTTD/Factorio/Transport Fever actually work under the hood.*
- **B — discrete tile-to-tile hops, no continuous position. REJECTED.** It throws away the working,
  deadlock-proof, byte-deterministic authority layer *and* the glide render, and literally cannot
  express headway (P1 spaces trains **sub-tile** by braking distance, `block_gap_mm`).

**Bonus:** the P4 Catmull-Rom shared-prefix arclen-divergence bug (the reason the MIN-gap coalescing
workaround exists, `dispatch.rs:94-120`) **vanishes** on crisp grid track — identical physical
prefixes get identical vertices and identical arclen. Grid makes the in-flight signalling work
*simpler*.

### Reuse map
- **Verbatim (no code change):** the arc-length core (`point_at`/`span_of`/`strictly_inside`/
  `speed_cap_at`/`length_mm`/`heading_at`/`loop_p`), `PointMm` i64-mm + isqrt Euclidean dist, the
  trapezoidal accel/cruise/brake + dwell integrator, the **whole P1–P4 runtime authority layer**
  (re-derived sorted Vecs, never hashed), the two-clock alpha-interp render + `render_buf` copy-out +
  `composeAndSet` stable-identity splice, the metre-radius catchment/walkshed hex renderers (already
  a lattice-over-mm renderer), determinism/replay, and the dispatch capacity clamps.
- **New (additive, behind a flag/ruleset tag):** a crisp grid-geometry mode (`samples=1` +
  lattice-snap; **not** the `literal` flag, whose `LITERAL_SAMPLES=2` still rounds corners); optional
  corner-fillet templates; a frontend tile quantizer on cursor **and** drag-handle mm; a degenerate
  `geo.ts` (integer scale+offset); grid build/editor tooling; the grid `Ruleset`/`Demand` + logistics
  objective.
- **Corner speed is fine, not degenerate:** a 90° lattice corner is concyclic with circumradius
  `cell/√2`, so `cap_from_radius` is finite and scales with `√cell_size` (~71k mm/s at 10 m cells,
  ~225k at 100 m — comparable to STREET/OFF-ROAD caps). Tune with fixed-radius corner-tile fillets if
  small-cell turns feel sluggish.

### The one real cost — track ownership (the genre's defining verb)
A TTD/Factorio grid logistics game's core verb is **shared physical track** (lay rail once, many
lines route over one edge, signals/occupancy track-owned). But the engine is **mechanically
line-owned**: `seg_key` packs `line<<40` (`vehicle.rs:119`), `junc_key` packs `line<<32` (`:165`),
junctions derive from per-line `diverge_at`. Two lines on the same rail get distinct keys and **pass
through each other** (the `#[ignore]`d `shared_trunk_..._is_p5` test proves it). **That gap *is* the
deferred P5 `TrackGraph`** — confirmed unbuilt (a doc-only seam slot).

- **A1 — line-owned (today): free.** A first grid slice reuses P1–P4 unchanged; lines lay their own
  rails, no sharing. Proves glide + authority on tile track with zero core risk.
- **A2 — shared *tile* `TrackGraph` (= P5): the genre verb, a deliberate phase.** Re-key occupancy
  to physical tile-edge/node ids; move `track_type`/signals per-edge (a **hashed-state +
  `SetSegmentTrack` contract change**); add a cross-path interlocking **liveness cap**. A *tile*
  graph is materially **cheaper** than the continuous one the roadmap feared (clean integer edge
  adjacency; the arclen-divergence bug is gone; `roadnav.rs` grid A* + the mm `BuildabilityGrid`
  already exist) — but it is still the single largest net-new subsystem, and **must ship the
  edge-mutex *with* the liveness cap in one change** (the ignored P5 test warns that physical keying
  *without* liveness turns cosmetic pass-through into a *worse* deadlock — never the half-fix).

**Recommendation:** A1 for the first grid slice; A2 as a named second phase only when shared rail is
committed as a core verb. The in-flight track-physics work is an **asset** — the runtime mutex
*semantics* (`group_overlap`, `occ_claim`/`try_claim`, coalescing) are reusable in spirit for A2;
only the *derivation* side and the ownership key get rewritten. Grid may be exactly where the P5
"architectural cliff" finally becomes climbable.

### Replay guardrails (grid must not destabilize the in-flight P-series)
- Grid geometry is **strictly additive behind a new flag**, never an edit to the default smoothing
  path. There are **zero golden-hash constants today** (all determinism tests are `run()==run()`
  self-equality), so an additive flag cannot break an existing hash test; the only way it bites is
  retrofitting grid geometry onto an existing line's build path — which the flag forbids.
- All new vertex math stays `i64` mm, index-ordered, `f64`-then-`.round()` only (the discipline
  `smooth_centripetal`/`circumradius` already follow, proven replayable by `waypoints.rs` over 3000
  ticks). Add a fresh `run()==run()` replay test for a grid-built line, plus RED property tests for
  zero-length spans (adjacent-tile stops) and identical-arclen ties across paths.
- Do **not** relocate `track_type`/signals to a shared `TrackGraph` edge without an explicit
  golden-hash re-pin **and** a `types.ts`/`codec.ts` mirror — that is a hashed-state shape change +
  a command-contract change, and the one place a careless P5 build can silently break replay (keep
  any shared graph index-ordered/integer, never a persistent HashMap-iterated structure).
