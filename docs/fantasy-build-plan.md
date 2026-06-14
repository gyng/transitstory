# Fantasy fork — engineering build plan (S0 → S11)

> **Status:** plan, not yet built. The engineering roadmap to construct the hex 4X-logistics fantasy
> game on the deterministic engine (ruleset-at-construction fork; transit stays byte-identical until
> fantasy is complete). Produced by an understand-real-code → design → adversarial-review → synthesis
> workflow; **verdict: sound, approved with conditions.** Companion to the *game* design
> ([fantasy-game-design.md](fantasy-game-design.md)) and the *architecture*
> ([fantasy-fork.md](fantasy-fork.md)). Obeys every [AGENTS.md](../AGENTS.md) non-negotiable.

## The shape

A huge fraction **reuses verbatim** (transit byte-identical) because it's all i64 arc-length + index
keyed and therefore shape-agnostic: `state_hash`/`SaveGame`/`replay`/ChaCha8, the whole **P1–P4 +
cross-line authority layer** (`seg_key`/`junc_key`/`group_overlap`/`occ_claim` — zero (x,y)
dependency, survives the hex port untouched), `VehicleSoA` + `advance`, `RaptorRouter` (the cargo
router *is* the transit router), the economy/growth substrate. New work is **additive sibling files**
transit never imports + the **hex port** (~100–150 lines behind `grid_cell_mm`) + the **one dangerous
MOVE** (S2). The art/UX is a re-skin — so the real cost is in the sim's new subsystems.

## The ordered sequence — with clean truncation points

**Foundation (transit-only refactor; ships fork-ready):**

- **S0 — Golden-pin (RED-first; the safety net).** Paste a `state_hash` u64 literal for
  `sample_log()+600 ticks @ dt=50`, assert it; commit a pre-refactor transit save replayed to that
  literal. Add `ruleset:String` (`#[serde(default="transit")]`) to `CityData` / `SaveGame` /
  `CityEntry.kind`. **No behavior change.** This is the *only* guard against a uniform hash shift —
  `determinism.rs` is pure `run()==run()` and structurally blind to it.
- **S1 — Trait scaffolding (no carve).** `ruleset/{mod,transit}.rs`: `trait Demand` + `trait
  Ruleset`, determinism contract copied verbatim from `routing/mod.rs:40`. Add `demand` + `ruleset`
  boxes to `World` beside `router`, default-constructed, **not yet called**. Canonical untouched.
- **S2 — THE CARVE (the one dangerous step; MOVE, don't rewrite).** Relocate
  `demand::{prepare,spawn,grow}` + `coverage_score` + `line_cost_metrics` behind
  `TransitRuleset`/`GravityDemand`/`AgentDemand`; fold the `agent_demand` if/else into `Demand`;
  trait-ify the `apply()` trailer. **Keep Canonical field order byte-identical and preserve the exact
  `world.rng` draw order** (`demand.rs` destructures `ref mut rng` in lockstep — any reorder breaks
  the hash). **Gate: the S0 golden hex diffs to zero.** Commit only then.
- **S3 — Mode toggle + disjoint-save guard.** `World::new` matches `city.ruleset`; `replay()` asserts
  `city.ruleset == save.ruleset`; `ruleset.validate(cmd)` runs **before** `cmd_log.push` (today it's
  unconditional) so a rejected cross-mode command never pollutes a save.
- **S4 — Mode-blind read surface + frontend factor. ✅ TRANSIT FULLY MIGRATED, byte-identical — a
  coherent shippable product; safe to stop.** Collapse the ~16 transit-named wasm accessors into a
  mode-agnostic `render_entities()` + generic `query(kind,args)` (+ a CI grep gate: no mode-string in
  `lib.rs`). Factor `game.ts` → `GameCore` + `TransitGame`.

**Fantasy build:**

- **S5 — Hex geometry port** (additive behind `grid_cell_mm>0`, NOT an edit to the default
  Catmull-Rom path): `grid_walk`→hex axial cube-round line-draw (**must sort the endpoint pair first**,
  replicating `line.rs:474`, or the cross-line mutex silently disengages); `node_of`→axial cube-round
  (one `f64`-then-`.round()`-to-i64 site — the one float hazard, pinned by a `run()==run()` test);
  `roadnav` 8→6-neighbor. Callers unchanged. Port the `grid.rs` symmetry/shared-edge tests onto hex.
- **S6 — First fantasy slice (RED-first; near-zero new motion code). ✅ Ships.** One source→sink→cart:
  `SupplyChainDemand::spawn` emits a commodity token (a `Pax`, commodity id in the unhashed
  `citizen_id`) reusing `RaptorRouter`+`advance`+`board_alight` **unchanged**; `SupplyChainDemand::grow`
  feeds a town. Grow `SaveGame` to **tick-stamps** `{seed, ruleset, [(tick,command)]}` + make `replay()`
  tick-aware. New `PlaceNode`/`BuildRoute` Commands (+ `contract.rs` + `types.ts`/`codec.ts`, one
  commit). Bake `arcadia_world.json` (2 cells). A separate fantasy golden-hash + ticked-replay.
- **S7 — Forge-Line chains.** ≤8 node types incl. BARRACKS in one place-node tool; 3-stage 2-input
  recipes over ~8 commodities; per-input i64 **buffers** as new hashed Canonical (appended after
  transit fields, empty when `ruleset==transit`). New consume→fire→push tick phase (Liebig: output =
  min input rates). *The buffer cap is the one non-derivable knob — budget playtest.* **Golden re-pin.**
- **S8 — War machine + AI** (budget as two sub-steps): **S8a** a **SEPARATE army SoA** (NOT an in-SoA
  `kind` byte — `dispatch`'s `v.clear()` rebuilds the shared SoA on every `SetHeadway` and would
  teleport a legion) that **owns its position** and admits to single-track via the **existing**
  `occ_claim`. **S8b** the `war_step` sub-phase (locked order accrue→launch→retarget→move-walk→grind→
  flip), supply-gated siege, `PlaceBarracks`/`PostBounty` (Majesty), quantize `town_value` to i64.
  Keyed RNG (`seed ^ WAR_CONST`).
- **S9 — Decadence raiders** (WALK `roadnav`, never `VehicleSoA` — the deliberate occ-cap-deadlock sidestep)
  that sever supply edges.
- **S10 — Core area-control CA (the largest subsystem; the perf cliff).** Dense hex contested-cell
  field (owner/decadence/contest) as a frontier-sparse **hashed** Vec (index-ordered, never HashMap; it
  *can't* reconstruct-from-seed because it's RNG-driven gameplay-causal). **Double-buffered** weighted-
  integer diffusion (read prev, write next — `demand::grow`'s in-place mutation is safe only because
  it's per-cell-independent; a neighbor-reading CA in place reads half-updated cells). **PURGE strictly
  dominates DIFFUSE.** Hard cell cap + a per-tick bench gate.
- **S11 — Economy / tech / endless+prestige / rival seam.** Tribute accrues like opex (exact-integer,
  hashed); soft money declines instead of restoring; the split gauge (each channel its own
  monotonicity invariant); the `UnlockTech` bitset; the per-block cross-line capacity + aging tiebreak;
  the rival kingdom as `war_step(owner != PLAYER)`.

## The three binding conditions (before merging the relevant step)
1. **S0's golden literal lands RED-first** and is **re-pinned as a reviewed single commit at every
   Canonical shape change** (S7/S8/S9/S10). The only guard against the uniform hash shift `run()==run()`
   can't see.
2. **S8's army is a separate SoA** (not a shared-SoA `kind` byte — `v.clear()` would teleport it),
   admitting through the **existing** `occ_claim`, never reinvented occupancy.
3. **S10's sparse Vec gets a hard cap + a per-tick bench gate** (a decadence bloom is a perf cliff no hash
   test catches).

## Gate-blind risk battery (assert reaches-zero / bounded / exactly-once / structural-equality — never `run()==run()`)
transit golden hex == baseline (every step) · hex `grid_walk` symmetric+canonical + two-lines-share-
byte-identical-edges · armies-self-position (s_mm unchanged across a `SetHeadway`) · army↔train share a
single-track block via `occ_claim` (no pass-through) · PURGE>DIFFUSE (a fed frontier reaches decadence==0) ·
launch-vacuum-gate (no manpower bonfire) · raider-frontier-steady-state (bounded system-wide, no
sawtooth) · never-livelock (a supplied army's distance-to-target monotone non-increasing) ·
bounty-exactly-once across grind→flip · `town_value` i64-quantize + candidate-order permutation
(tied-score → same TownId) · CA identical-field-after-K-days-twice + directional-symmetry + the
per-tick bench within the 20–30 Hz budget · split-gauge monotonicity (one proof per channel).

## Fast iteration, telemetry & balancing — the determinism dividend

The deterministic, command-sourced, pure core isn't just a correctness gate — it's a **balancing
superpower**, because the sim is headless and reproducible. The whole iteration loop falls out nearly free:

- **Headless max-speed harness** (a new `crates/balance` bin, or extend the native test infra — AGENTS
  already mandates "sim logic is native cargo, not the browser"): just `World::new(seed, city) → apply(log)
  → tick(dt)` in a tight loop to a horizon, with **no render, no rAF, no 20–30 Hz throttle** — millions of
  ticks/sec, bit-for-bit reproducible.
- **Mass parallelism** (the "background max-speed sims in parallel"): run **N seeds × M parameter sets
  concurrently across threads**. Determinism is *per-instance*, so the no-threads-in-`crates/sim` rule
  isn't violated — the harness owns N independent `World`s on N threads, each deterministic alone.
  Thousands of full playthroughs in seconds.
- **Parameter sweeps**: externalize the tunable knobs (buffer caps, tribute floor/ceil, the five brake
  constants, `launch_cost`, decadence spread rate, `growth_bp`, tech costs) into `CityData`/a config struct
  so a sweep **varies them without recompiling**. Grid / random / bisection over the param space.
- **The AI-as-playtester (the big multiplier):** the player is *already* mostly a logistician steering an
  autonomous AI, so write a **scripted logistician-bot** (place nodes near sources, rail toward the
  best-scored town, post bounties, double-track the throbbing block) and the **whole game self-plays
  headless** across seeds — reproducibly, no human. This is how you balance without hand-playing every
  change.

**Telemetry to collect** (per-tick or periodic → CSV/structured logs):
- **Pacing / does-it-bite** (the #1 unknown): time-to-first-buffer-overflow (the throb), -to-first-capture,
  -to-tier-2, -to-first-tech-unlock. Target: the loop bites in ~60–120 s at default speed.
- **Flywheel / economy**: tribute/turn, **net-income/turn vs town-count** (catches both snowball *and*
  stall/autopilot), gold/mana/manpower balances, debt/bankruptcy events.
- **Bottleneck**: per-node buffer occupancy + throughput, and **where the bottleneck sits over time** (does
  it *move* or pin?).
- **War / front**: legions launched, sieges won vs stalled, towns flipped vs lost, **decadence-front
  distance-to-capital over time**, over-extension events.
- **Liveness** (gate-blind): max sustained zero-progress streak, oscillation/sawtooth detection, flip-count
  bounds.
- **Gauges**: the monotonic-progress + volatile-front trajectories.
- **Determinism guard**: `state_hash` at checkpoints across the sweep → catches any non-determinism a
  balance change introduced.

**Balance automation:** the **four RED property tests are the hard gate**, run across the whole sweep — a
parameter set that violates them is auto-rejected. Layer the soft metrics (compounds-without-autopilot,
bites-fast, bottleneck-moves) on top and **search for the param region that satisfies the invariants + hits
the pacing targets**. The harness *is* the balance tool.

**The live debug overlay:** a dev-mode headless shadow-sim run ahead of / alongside the displayed game,
surfacing the telemetry live — watch the flywheel/brake/front metrics while you tune, fast-forward to see a
build play out.

**Where it lands:** a dev-tooling workstream alongside the build, but **land the harness EARLY** — the
moment the fantasy sim is runnable (≈ after S6) — because it's how you cheaply de-risk the two things paper
can't answer (*does it bite fast*, *is it compelling*) and tune the non-derivable knobs (the buffer cap, the
brake rates). The throb/pacing experiment we keep flagging is just the **first run** of this harness.

## Performance — 90 fps (mid) / up to 144 fps (strong), native Windows

**Verdict (code-grounded):** 90 fps comfortable for both games on a mid machine; **144 fps yes for transit, yes-with-
conditions for the fantasy load on strong GPUs.** The render loop is **already decoupled + uncapped** — `GameLoop.frame`
is a bare `requestAnimationFrame` self-reschedule (no fps cap, no `setTimeout`, no `1000/60`; the "60fps" mentions are
comments). The sim is a separate fixed 20 Hz accumulator on wall-clock `dt`; `alpha = acc/TICK_MS` recomputes per
frame, so a 144 Hz monitor just yields finer interpolation (smoother glide), sim still 20 Hz. Nothing to remove.

CPU per frame is trivial (O(moving entities): SoA copy-out + arc-length lerp + a capped binary-attribute peep/legion
sweep + one `setProps`). **The binding term for fantasy is GPU fill-rate/overdraw** on the dense stacked honeycombs —
Singapore is **62,234 cells**, so ground+decadence+territory ≈ **~180k hex instances** re-rasterized each frame
(buffers cached, not re-uploaded). A mid dGPU clears that inside 11.1 ms; the squeeze toward 6.9 ms is overdraw at
overview.

**Conditions to hold 144 (build requirements):** (1) keep the dense honeycombs **topology-cached** — key
`updateTriggers` on 1–4 Hz content, NEVER a per-frame value (a per-frame bump = a full ~180k-instance re-upload =
blows the budget); (2) **write the LOD pass** — it does NOT exist yet (today's LOD only drops peeps/halos/arrows);
below `DETAIL_ZOOM` drop/merge the dense fields + node-bars/countdowns/glyphs/arcs/tethers — the fill-rate relief that
makes 144 comfortable; (3) **legion dots on the peep binary-attribute path + a hard cap** (not an object-array
`ScatterplotLayer` rebuilt per frame); (4) benchmark on **native Windows + a real dGPU** (NOT WSL — software GL there
is misleading). The one unmeasured corner — 62k × 3 honeycombs + max legions at full overview — deserves one real GPU
frame capture.

## Scope: completable, degrades gracefully
The two large subsystems (S8 war, S10 CA) are sequenced last; **S10 is the scope risk to watch** — it
*is* the "core area-control" identity, so don't start it until S6–S8 are green-and-stable, lest a
half-built CA strand the fork between two products. **Clean truncation points: S4** (transit migrated,
fork-ready) · **S6** (one fantasy slice ships) · **S8** (war loop without the CA endgame). Each is a
coherent shippable product.

## The first step
**S0 — the golden-hash pin.** Small, safe, behavior-preserving; the foundation everything stands on,
and it hardens the existing transit game's determinism on its own. Land it whenever design turns to code.
