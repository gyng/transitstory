# transitstory — AGENTS.md

Engineering & design conventions for **transitstory** (a 2D top-down transit-builder game:
deterministic Rust→WASM sim + TS/Vite frontend on a real OSM map). This file loads into every
coding session, so it is deliberately high-signal. It encodes the rules that **protect the locked
decisions** — treat them as constraints to satisfy, not options to relitigate. For the build plan,
milestones, and task graph, see [PLAN.md](PLAN.md).

## The spine

The codebase is **concentric rings**: `packages/app` (TS / MapLibre / deck.gl) → `crates/sim-wasm`
(the only wasm-aware crate) → **`crates/sim`** (the pure deterministic core). **Dependencies point
strictly inward.** Determinism and command-sourcing aren't just features — they are the
*enforcement mechanism*: the replay-equality test goes red the instant an outer-ring concern leaks
inward, and the single `apply_command_json` seam is what makes "the frontend never mutates state"
mechanically true.

---

## Non-negotiables (the whole team obeys these; domains below add only their delta)

1. **Inward dependency edge.** `crates/sim` depends only on serde / rand / rand_chacha / indexmap /
   rustc-hash / fnv — **zero** `wasm-bindgen`/`js-sys`/`web-sys`/`serde-wasm-bindgen`. `crates/sim-wasm`
   is the only `cdylib`/`#[wasm_bindgen]` crate; it wraps `Sim{ core: sim::World }` and holds **no game
   logic** (no catchment math, routing, dispatch, scoring, or clamps — those live in the core).

2. **Determinism gate = commit gate.** The core is pure: `i64` ms time, `i64` mm positions, seeded
   `ChaCha8Rng` (`use rand::{RngExt, SeedableRng}`; `.random_range`, never `.gen_range`), index-ordered
   `Vec`/slab/`IndexMap` iteration only. **Banned in `crates/sim`:** `std::HashMap`/`HashSet` iteration,
   `f32`/`f64` in state-affecting math, wall-clock (`SystemTime`/`Instant`), threads/rayon. The
   replay-equality test (same seed + same command log ⇒ identical `state_hash`, twice in one process)
   must be **green at every commit from CP0** — a red gate blocks the commit, no `wip:` escape.

3. **Command-sourcing is the only write path.** Every mutation is a serde `Command` via the single
   `apply_command_json` → `World::apply`. The frontend **never** mutates sim state — it sends Commands
   and reads snapshots; the in-memory log is the save (`seed + log`) and the future-multiplayer seam.
   New capability = a new `Command` variant, **never** a wasm setter. Only the six Commands mutate state;
   pause/play is `SetRunning`, **speed is a GameLoop knob, not a Command**.

4. **`coords/geo.ts` is the one coordinate crossing.** All lng/lat ⇄ metres ⇄ mm and all Web-Mercator
   math live there and **nowhere else**. Commands carry only `i64` mm; the core's `geo_local.rs` is
   `i64` mm planar with no projection. No `Math.*` Mercator constant in `overlay.ts`, `SimBridge.ts`,
   `main.ts`, or any UI file; no `lon_e7`/`lat_e7` field ever enters a Command.

5. **Render hot path.** Per frame: **copy** the SoA `f32` buffers into a *reused* JS `Float32Array`
   (re-acquire the view if `memory.buffer` grew — never hold a long-lived zero-copy view), feed deck.gl
   via `data.attributes`, `setProps` with **stable data identity**, bump `updateTriggers` **only on
   topology change**, and **never rebuild layers per frame**. Any slowdown is the rebuild anti-pattern,
   not a scaling limit.

6. **Two clocks, never merged.** Sim steps at a **fixed 20–30 Hz**; render interpolates to **60 fps**
   via `alpha = accumulator/dt`, lerping **along polyline arc-length** (not raw x/y → no corner-cutting);
   DOM/stats update on a **separate 1–4 Hz** throttle. Never call `stats()` inside rAF; never touch the
   DOM in the render loop. **Build mode does not tick** — its feedback is purely client-side.

7. **Units/ids are types; BigInt crosses the boundary.** Positions `i64` `_mm`, time `i64` `_ms`, ids
   are `u32` newtypes (`StationId`/`LineId`/`TrainsetId`/`VehicleId`/`PaxId`). `i64`/`u64` marshal to JS
   as **`BigInt`** — wrap reads in `Number()` at the SimBridge/Stats boundary; never `BigInt === number`.

8. **Guard the thin loop.** The slice is exactly: 2 build tools + Build/Run + 3 speeds + one Headway
   slider + one auto-assist (auto-name + auto-headway). Deferred features (track types, junctions, fares,
   transfers/RAPTOR K>1, extra trainsets, time-of-day, GTFS, more cities, multiplayer) attach **only** as
   `trait Router` / `trait Demand` / `Command` / `CityData` seams — **never half-built**.

9. **Pin & lock; minimal deps.** `rand="=0.10.1"`, `rand_chacha="=0.10.0"`, `wasm-bindgen="=0.2.117"`
   (must match the installed CLI — re-verify at run start), `@deck.gl/core`+`@deck.gl/layers`+`@deck.gl/mapbox`
   all at the **identical** `9.3.x`, `vite-plugin-wasm@^3.6` only (`build.target:'esnext'`, no TLA plugin).
   Commit **`Cargo.lock` + `pnpm-lock.yaml`**. `postcard` stays Rust-only (save artifact); the wire format is JSON.

10. **Attribution is a release gate, not polish.** MapLibre `AttributionControl` (OpenStreetMap; + Protomaps
    once T7 lands) is mounted **from the first map commit**, and the `ATTRIBUTION` file is committed in the
    first scaffold commit. ODbL Produced-Work obligation — never ship the slice without visible credit.

---

## Architecture *(owns: how things attach to the rings)*

- **`sim-wasm` is a translation membrane.** It only marshals: `serde_json` decode Command → call `sim` →
  copy-out SoA pointers → `serde-wasm-bindgen` the stats struct → `i64/u64 → f64/BigInt`. If you're
  tempted to compute, clamp, validate, or branch on game state there, **move it into `crates/sim`** and
  expose the result through the existing ports.
- **The core's only ports** are the `Command` enum (in) and the SoA buffers + `stats()` snapshot (out).
  New player capability → new `Command` variant + `apply` arm. New readout → extend `stats.rs` or add an
  SoA buffer in `render_buf.rs`. Never a new mutator method on the facade.
- **Aspirational systems attach as ring-respecting extensions.** Transfers/bus/HSR → a new `routing/raptor.rs`
  `impl Router` (RAPTOR rounds K>1 + footpaths) behind the existing trait — *not* a change to `apply`'s
  signature. New city → a new committed `*_city.json` + `*_demand.json` consumed by the same
  `Sim::new(seed, city_json)`. Multiplayer → a netcode module in the **outer** ring that ships/receives the
  command log; the core is untouched. ✗ Reworking `World` internals or the facade "to support transfers"; ✗
  hardcoding Singapore constants in the core instead of `CityData`.

---

## Code organization *(owns: file layout, naming, error discipline, pins, commit hygiene)*

- **Files map 1:1 to PLAN §4; modules stay single-responsibility.** `tick.rs` is the strict ordered phase
  loop *only* (clock → spawn+route → dispatch → move → alight/board → accounting) — it is the determinism
  heart, keep it isolated and auditable. Deferred systems sit behind their trait seam
  (`routing/{mod.rs,direct.rs}`, `demand/{mod.rs,gravity.rs}`). ✗ inlining `DirectRide` branching into
  `tick.rs`; ✗ growing `world.rs` into a catch-all.
- **`tick()` never panics; `Result` lives at the `apply()`/decode boundary.** A panic inside `tick` aborts
  the wasm module mid-frame *and* breaks replay (the log can't be re-applied). Use saturating/clamped
  arithmetic and bounds-checked slab access on the hot path. Fallible work (command/JSON/CityData validation)
  returns `Result`, confined to `apply()`/`city.rs`; the facade maps it to a thrown JS error so an invalid
  Command surfaces in the EditorPanel, never aborts the instance.
- **`types.ts` is a hand-mirrored contract — drift is a bug.** When you change a serde shape in `command.rs`/
  `stats.rs`/`city.rs`, update `types.ts` + `codec.ts` in the **same commit**, and match serde's enum tagging
  exactly. A round-trip Vitest + the Rust `serde_json` round-trip test pin it.
- **Keep the bans greppable.** A CI/pre-commit grep over `crates/sim/src` should fail on
  `std::collections::HashMap`, `: f64`/`: f32` in state fields, `SystemTime`, `Instant`, `std::thread`,
  `use rand::Rng;`, `\.gen_range`. Floats are allowed only in `render_buf.rs` (copy-out) and in `tick(dt_ms:f64)`
  cast immediately to `i64`.
- **Commit only green trees; `PROGRESS.md` is the flight recorder.** Before committing, run the relevant tier(s);
  log every time-boxed fallback (deck native fallback, hosted basemap, synthetic demand) with its reason and the
  resolved tool versions. Branch: scaffold on `main`, then `slice/singapore-vertical`; don't push unless asked;
  end commit messages with the required `Co-Authored-By` trailer.

**Checklist:** no wasm/JS crate or float/HashMap-iteration/wall-clock/thread in `crates/sim` · every action is a
serde Command mirrored in `types.ts`+`codec.ts` · units suffixed `_mm`/`_ms`, ids are newtypes · `tick()` has no
unwrap/index-panic and clamps to SoA capacity · BigInt reads `Number()`-wrapped · pins unchanged + both lockfiles
committed + `PROGRESS.md` updated.

---

## Testing / TDD *(owns: HOW determinism & invariants are proven)*

- **Write the load-bearing tests RED, first.** Before any sim algorithm exists, write and fail
  `determinism.rs` (apply a command log, tick N, snapshot `state_hash`, do it twice, assert equal) and
  `catchment.rs` (two overlapping catchments over one high-weight cell → captured demand across all stations
  ≤ cell weight; a far station captures 0). `state_hash()` (FNV-1a over canonical ordered serialization) exists
  in T2 precisely so the replay test has something to assert on day one.
- **Sim logic is native cargo, not the browser.** All of `crates/sim/tests/` is fast native `cargo test`. The
  *only* wasm test is the single Vitest wasm-in-node smoke (create `Sim`, apply one command, tick, assert buffer
  length > 0 + equal `state_hash` across two runs). No `wasm-bindgen-test`/headless-chrome for sim behavior.
- **Property-test the invariants, with seeded sequences** (so any counterexample replays bit-for-bit): captured
  demand ≤ cell weight; boarded ≤ trainset capacity; vehicle count never exceeds pre-sized SoA capacity for any
  allowed headway; cumulative ridership is monotonic non-decreasing; two runs of the same generated `Vec<Command>`
  ⇒ equal `state_hash`.
- **Test through Commands + `state_hash`, never by poking `World` internals.** Build scenarios via the same
  `apply` path the frontend uses; assert on `state_hash` (equality/determinism) and `Stats` (behavior).
  ✗ `world.vehicles.x[0] = 5000` to "set up" a state the command log could never produce.
- **e2e asserts gameplay facts, never "page loaded."** End on: a vehicle's position changed between two reads **and**
  ridership > 0 **and** the coverage gauge moved. Wait on window flags (`__APP_READY`/`__MAP_READY`/a sim-tick flag),
  never sleeps. **Pin a fixed viewport + fixed map center/zoom**, and place stations via the camera-independent hook
  `window.__ot_test.placeStationLngLat(lng,lat)` — which **routes through `coords/geo.ts`**, so e2e exercises the
  production coordinate boundary, not a second one.
- **Screenshots are corroboration, not the gate.** After each UI checkpoint, drive the page (Playwright MCP),
  perform the gesture, read the console, screenshot, and *look* (catches "trains in the ocean" that a `ridership>0`
  assertion passes through). No pixel-diff baselines for the slice.

**Checklist:** determinism replay green this commit · new sim logic landed test-first asserting the invariant ·
sim logic in native cargo (only the wasm-in-node smoke touches wasm) · every e2e asserts a behavioral fact + waits on
flags · e2e camera-independent via the geo.ts-routed hook · all three tiers green + a clean self-verification screenshot.

---

## Frontend / atomic design *(React 19 chrome over an imperative sim/map core)*

> **Decision change (2026-06-05):** the UI chrome migrated from vanilla-TS `createX` factories to **React 19**
> (`@vitejs/plugin-react`, new JSX transform). The *load-bearing* rules below are unchanged — only the rendering
> substrate is. React owns the **DOM chrome**; the map, deck.gl overlay, and rAF loop stay **imperative and outside
> React**. The old `createX(deps): XHandle` convention is retired; the React equivalent is a function component that
> reads hook slices and calls `Game` methods.

Tree: **root** (`main.tsx` → `ui/react/App.tsx` phase machine) → **provider** (`GameContext` — the one React⇄sim
seam) → **components** (`Menu`, `StatsBar`, `Panels` = LineList + Editor, `Toolbar` = chorded bar + popover,
`Settings`). Shared constants/formatters live in `ui/react/shared.ts` (`MODES`, `modeIcon`, `hex`, `fmtMoney`,
`PANEL_STYLE`). The `#map` div, deck overlay, `GameLoop` rAF, and `coords/geo.ts` are untouched by React.

- **`GameContext` is the only seam, with TWO slices on TWO cadences** (the "two clocks" rule, enforced):
  `stats` (the `Stats` snapshot) is **pushed from the existing ~3 Hz interval**; `ui` (mode/tool/transport/
  selection/enabledModes/showDemand) is updated on `game.onChange` with a shallow-compare so the 3 Hz churn
  doesn't cause redundant renders. Hooks: `useGame()`/`useLoop()` (stable instances), `useStats()`, `useGameUI()`.
  ✗ React touching deck.gl/the map/`requestAnimationFrame`; ✗ a component re-rendering per animation frame.
- **UI emits Commands by calling `Game` methods; it reads snapshots via hooks.** A component's only outputs are the
  DOM it owns and `Game` calls (which funnel to `SimBridge`). **`SetHeadway` fires once on drag-end** — bind the
  slider's native `change` to commit and native `input` to a local preview `useState`; React's synthetic `onChange`
  maps to the DOM `input` event and would commit per drag-tick, so it's banned for the headway slider. **Speed is a
  `GameLoop` knob (local state → `loop.setSpeed`), not a Command.** ✗ optimistic self-rendered sim state.
- **Reconcile lists with React keys (`key={id}`) — never `innerHTML`.** Keep data-bound inputs **uncontrolled +
  keyed** to the committed value so they resync on the next snapshot without clobbering an in-progress drag.
- **Styles are token-driven** (`styles.css` custom props: `--ot-space-*`, `--ot-color-surface`, `--ot-gauge-*`). The
  one exception is **per-line color**, sim-owned (`Line.color:u32`), which must reach the list swatch and the deck
  `PathLayer` by the *same* path (`hex()` in `shared.ts`) so they never drift. ✗ a `'#3388ff'` literal in a panel.
- **Simulation geometry is deck.gl layers, never DOM.** React panels are absolutely-positioned chrome inside `#ui`
  (pointer-events scoped) over the full-screen `#map`. Selection/hover flows through the snapshot + command path,
  not DOM coupling. ✗ an HTML div positioned by lng/lat as a station.
- **Every `data-testid` is a contract** — the e2e suite asserts on them; preserve them across any component refactor.

**Checklist:** chrome is React, map/deck/rAF stay imperative · all sim reads via `useStats`/`useGameUI`, all writes via
`Game` methods (zero direct mutation) · headway commits on native `change` only, speed is a loop knob · DOM updates
throttled to the ~3 Hz `stats` slice, not per frame · colors/spacing from tokens, per-line color via `hex()` (same
path as the PathLayer) · lists keyed (no `innerHTML`), data inputs uncontrolled+keyed · geometry is deck layers, all
projection in `coords/geo.ts` · testids preserved.

---

## UX / cognitive science

- **Forgiving map targets (Fitts).** Snap/hit radius is a generous **screen-pixel** constant (in `config.ts`), not
  metres — so it stays tappable when zoomed out. Highlight the snap candidate *before* the click commits.
- **Sub-100 ms optimistic feedback.** Hover highlight, the dashed blueprint following the cursor, and the snap ring
  are drawn client-side every frame on `mousemove` — never gated on the 20–30 Hz tick or 1–4 Hz stats. Build is
  paused, so the acknowledgement *is* the visual.
- **Recognition over recall.** Every queryable state has a persistent on-screen channel: catchment circle, line-color
  swatch reused everywhere, accumulating waiting-pax dots, load factor, the 0–100 gauge. No memorized keystrokes, no
  hidden modes.
- **Progressive disclosure.** The right EditorPanel is empty until selection, then shows only the selected object's
  controls (name, color, trainset, headway, pre-filled with the auto-suggested default). Build controls live in the
  tools; run controls in the bottom bar.
- **Reversible by construction; Build/Run is a hard wall.** Blueprint-then-commit emits one Command; **undo = rebuild
  `World` from `seed + log[..-1]`** (the frontend never splices state). You cannot draw on a live network — switching
  to Run commits blueprints.

**Checklist:** doesn't add a tool/mode/slider beyond the budget · snap radius is screen-pixels with pre-commit
highlight · pointer gestures get sub-100 ms client-side feedback · new state is a persistent on-screen channel +
right panel only on selection · edits are one undoable Command respecting the Build/Run wall.

---

## Information architecture / visual *(owns: z-order, palette, catchment cap, attribution placement)*

- **Mute the basemap** (CARTO Positron / grayscale Protomaps flavor); reserve hue entirely for line identity. The
  player reads only their own network — if the base competes, figure-ground collapses.
- **Fixed overlay z-order (back→front), stable across frames:** basemap < catchment < line PathLayers < station dots
  < vehicles < waiting-pax dots < selection/hover highlight. Array order *is* the z-buffer here. ✗ pushing catchment
  last so its fill greys out the stations you're trying to click.
- **Constant pixel widths for the network; real metres only for space.** Lines: `widthUnits:'pixels'` + `widthMinPixels`
  + rounded caps. Stations/vehicles/pax: pixel (clamped) radius. **Catchment: `radiusUnits:'meters'`** — it's a genuine
  ~500 m spatial fact that feeds the coverage gauge, so it must scale with the map.
- **Cap visible catchment to selected/hovered stations** (low fill alpha + stroked outline); bump `updateTriggers` only
  when that id set changes. Stacked translucent discs both turn to mud *and* misrepresent overlap (the no-double-count
  invariant means stacked discs ≠ stacked demand).
- **Color is line identity: deterministic & color-blind-safe.** Auto-assign from a fixed ~8-entry ordered CB-safe
  palette (Okabe-Ito-style) in `config.ts` on `CreateLine` — keeps color reproducible alongside the command log.
  Always pair with the name/swatch (never hue alone; ~8% of male players can't separate red/green). An optional picker
  only **re-selects within that palette** so lines never collide into an indistinguishable pair.
- **Map carries spatial truth; panels carry abstract state.** Left = line roster (swatch, auto-name, per-line ridership);
  right = the one selected object's properties; bottom = global controls + **one headline ridership counter + one 0–100
  gauge** (details on station/line hover or in the EditorPanel, never crammed into the bar or floated on the map).
- **Distinguish provisional from committed:** blueprint = dashed PathLayer; committed = solid full-color; ghost gesture
  = translucent. Mirrors the Build(editable)/Run(ticking) split.

**Checklist:** overlay z-order correct & stable · lines pixel-width, catchment metre-radius · catchment shown for
selected/hovered only · line color from the ordered CB-safe palette + always paired with text · spatial-on-map /
abstract-in-panels, StatsBar = 1 number + 1 gauge · `AttributionControl` mounted & unobstructed.

---

## Game design *(owns: the loop, juice, the gauge, deferral discipline, onboarding)*

- **The 5-step loop is the product and the unit of scope.** place → draw → assign+headway → Play → read → tweak. A
  feature ships only if it *sharpens* one of those steps; no step may leave the map or open more than the one contextual
  panel. ✗ a per-stop timetable editor, a station-config modal, a build wizard.
- **Cause→effect legible within one loop turn.** Every Command has an immediate on-map echo (place → catchment appears;
  draw → colored line commits with a flash; raise headway → vehicle spacing + waiting dots move) **and** a readable
  post-Play stats delta. An action with no visible echo isn't a legible lever — make it visible or cut it.
- **The coverage gauge is MONOTONIC.** A strictly-better network never lowers it. The formula lives in **`stats.rs`** as
  a blend of (% catchment demand served) and a (wait-vs-headway) penalty **bounded below coverage gains**, with a
  property test (superset network + shorter headway ⇒ score not lower) alongside `catchment.rs`/`ridership.rs`.
- **Left-behind passengers are informative pressure, not a bug** — the only difficulty source in a money-free slice.
  Surface `left_behind` + per-station waiting; scale waiting-dot size with the queue so a starved station visibly throbs,
  pointing at the fix (more capacity or shorter headway).
- **Capacity and Headway are the two orthogonal levers** — capacity moves the per-vehicle ceiling, headway moves wait
  (~headway/2) and throughput. Derive train-count ↔ headway automatically. The **SoA-capacity clamp (min headway /
  max trains-per-line) lives in `dispatch.rs`**, unit-tested in `crates/sim`, never reimplemented in `sim-wasm` or the
  UI — the UI reads the clamped value back from a snapshot.
- **Game feel is P0, on the locked render path.** Connect flash + chime, animated vehicle/pax dots, a reacting gauge are
  deliverables — but ride the alpha-interpolation + stable-data-identity path. Dot growth and gauge motion are CSS/canvas
  tweens off the 1–4 Hz `stats()`, not new sim ticks. ✗ a "pulse" via per-frame layer rebuild; ✗ a zero-copy view hack
  to "save" a copy the plan calls negligible.
- **Onboard by one ghost gesture + one-line objective** (cut-first; issues no Commands, touches no sim state). **Defend
  the thin loop against NIMBY depth** — track types, junctions, fares, transfers, extra trainsets, time-of-day are
  trait/Command/CityData seams only. A "nearly free" depth addition that can't complete is a net loss versus a finished
  thin loop; log the deferral in `PROGRESS.md`.

**Checklist:** keeps the loop to 2 gestures + 1 panel, Build→Run replay in one click · every Command has an immediate
on-map visual + readable post-Play delta · `stats.rs` monotonicity property test passes · capacity + Headway are the
only two levers, one Command per committed slider value, clamp in `dispatch.rs` · juice rides alpha-interp + stable
identity, no per-frame rebuild · any deferred-list feature is a seam only, not half-built.
