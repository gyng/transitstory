# onlytransits — Overnight Build Plan (ultracode)

> **Purpose.** This is a self-contained, execute-without-a-human plan to take this repo
> from empty to a **playable vertical slice** of a NIMBY-Rails-style 2D transit-builder
> on a real OpenStreetMap map of **Singapore**, in one overnight run.
>
> It was produced by a research → design → adversarial-critique workflow (grounded in live
> web/registry/environment checks). Every correction the critic found is **already baked in
> below** — where the plan contradicts your training-data instincts (e.g. `rand` API,
> coordinate handling, postcard), **the plan wins**. Read §0 first.

---

## 0. Pre-flight corrections — READ THIS FIRST

These are verified facts that contradict common pre-2026 examples / training data. An
unattended agent that trusts its instinct here will burn hours. Treat as non-negotiable.

1. **`rand` 0.10 moved the sampling methods to a new trait.** `use rand::Rng;` + `gen_range`
   / `random` **does not compile** on rand 0.10. You **must** `use rand::{RngExt, SeedableRng};`
   to get `.random()`, `.random_range()`, `.random_bool()`. Seeding is
   `ChaCha8Rng::seed_from_u64(seed)` (via `SeedableRng`).
2. **Exact crate pins differ by patch.** Pin `rand = "=0.10.1"` **and** `rand_chacha = "=0.10.0"`
   (there is **no** `rand_chacha` 0.10.1). Mirroring the patch number breaks `cargo`.
3. **`wasm-bindgen` must match the installed CLI.** Pin `wasm-bindgen = "=0.2.117"` in
   `crates/sim-wasm/Cargo.toml` (latest crate is 0.2.122; cargo will pull it and break the
   glue otherwise). Re-verify the installed CLI version at run start and pin to match.
4. **The sim NEVER sees lng/lat.** `Command::PlaceStation` carries **`{ x_mm: i64, y_mm: i64 }`**
   (local planar millimetres), not `lon_e7/lat_e7`. All Web-Mercator / lng↔lat math lives in
   TS `coords/geo.ts`. This is the load-bearing decision the whole determinism + GTFS story
   rests on — do not put geographic coordinates inside the deterministic core.
5. **Command codec = JSON, not hand-rolled postcard.** Expose `Sim.apply_command_json(json: &str)`
   in the wasm wrapper; TS sends `JSON.stringify(cmd)`. Hand-matching `postcard`'s non-self-describing
   wire format in TS is a multi-hour rabbit hole and there is no maintained JS encoder. Keep
   `postcard` **only** for the Rust-side save-file / command-log artifact (both ends Rust).
6. **`Sim::new(seed: u64, city_json: &str)` from the very first wasm build.** Do not build a
   seed-only constructor; you'll have to re-shape the API and every call site mid-run.
   `city_json` may carry an empty/placeholder demand grid until the real one exists.
7. **deck.gl pin discipline:** keep `@deck.gl/core`, `@deck.gl/layers`, `@deck.gl/mapbox`
   **all at the identical `9.3.x`**. `@deck.gl/mapbox` does **not** peer-depend on maplibre-gl,
   so there is no maplibre↔deck version coupling — mismatched deck submodule minors is the real break.
8. **Drop `vite-plugin-top-level-await`.** Set `build.target: 'esnext'` (and esbuild target esnext)
   and use `vite-plugin-wasm@^3.6` only. Two plugins = redundant failure surface.

Secondary gotchas (cheap to handle, expensive to discover at 3am):

- **i64/u64 cross the wasm boundary as JS `BigInt`, not `number`.** Either expose UI-facing
  numerics (clock, ridership, hash) as `f64`/`number` from Rust, or wrap with `Number(x)` in TS
  and Playwright assertions. A naive `=== number` comparison will silently fail.
- **Go straight to the synthetic demand grid.** Do **not** try to `pip install pyrosm` on the
  overnight budget — it pulls geopandas + heavy geo wheels. The sim consumes the committed JSON
  identically either way; real OSM-derived weights are a post-slice nicety.
- **Basemap is OFF the critical path.** Ship on a hosted free style (CARTO Positron / MapLibre
  demo) first. Self-hosted PMTiles (`pmtiles@4.4.x`, v4 API) is the deferred T7 upgrade — and
  the most cuttable large task.

---

## 1. Vision & scope

**The slice (one night):** a genuinely fun, complete 5-step loop on a real Singapore map —
*place stations → draw a line → assign a trainset + headway → press Play → watch trains run,
passengers board/ride/alight, ridership & a coverage score climb → tweak.*

**Locked architectural decisions (do not relitigate):**

| # | Decision |
|---|----------|
| 1 | **2D top-down**, map-centric. Real OSM geography underneath; schematic overlays on top. Not 3D, not isometric. |
| 2 | **Deterministic Rust sim → WASM**; TS/web frontend. Determinism is for future lockstep multiplayer. |
| 3 | Overnight target = **playable vertical slice**, Singapore only. Thin but end-to-end. |
| 4 | **Sandbox on real geography** (player builds everything). Architect so real-life **GTFS import** can be layered in later — but don't build it. |
| 5 | **Architect for multiplayer** (deterministic + command/event sourcing) — but **do not implement** it. |

**Aspirational (keep seams open, build none tonight):** interchanges, transfers, HSR, bus
networks, multiple trainset types, junctions/signaling, time-of-day/rush-hour demand, economy
(fares/costs), the other 8 launch cities (Tokyo, Shanghai, London, NYC, Calgary, SF, Kolkata, KL),
heavy auto-assist, polished UI.

---

## 2. Tech stack (pinned)

**Rust core (`crates/sim`, pure, host-testable):**
- `rand = "=0.10.1"`, `rand_chacha = "=0.10.0"` (seeded `ChaCha8Rng`; `use rand::{RngExt, SeedableRng}`)
- `serde` (derive), `serde_json` (CityData + JSON command path), `postcard = "1.1"` (save-file only)
- `indexmap` and/or `rustc-hash` (`FxHashMap`) — **never `std::HashMap` iteration** in sim logic
- `fnv` (or hand-rolled FNV-1a) for `state_hash()`

**Rust wasm wrapper (`crates/sim-wasm`, `crate-type = ["cdylib"]`, the only wasm-aware crate):**
- `wasm-bindgen = "=0.2.117"` (match installed CLI — re-verify at run start)
- `serde-wasm-bindgen = "0.6.5"` (low-frequency structured `stats()` only)
- `sim` (path dep)

**Toolchain:** stable Rust (verify version at run start; `rust-toolchain.toml` pins it +
`targets = ["wasm32-unknown-unknown"]`); `wasm-pack` (`--target web`); `pnpm`; Node 24.x.

**Frontend (`packages/app`, Vite + TS):**
- `maplibre-gl@5.24.x` (basemap, camera, input, `unproject`, AttributionControl)
- `@deck.gl/core@9.3.x` + `@deck.gl/layers@9.3.x` + `@deck.gl/mapbox@9.3.x` (**identical** minor) — overlay layers + picking
- `vite@8.x`, `vite-plugin-wasm@^3.6` (**no** top-level-await plugin; `build.target:'esnext'`)
- `vitest@4.x` (unit + wasm-in-node smoke), `@playwright/test` (e2e + self-verification)
- **Deferred (T7 only):** `pmtiles@4.4.x` (v4 API) + protomaps basemaps for self-hosted Singapore tiles

Commit `Cargo.lock` **and** `pnpm-lock.yaml`. Gitignore `target/`, `node_modules/`,
`packages/wasm-sim/pkg/`, `*.pmtiles`, `*.pbf`, `data/raw/`.

---

## 3. Architecture

**Data flow (one full loop):**

1. **Input** — player interacts with the MapLibre map; a small TS drawing state machine reads
   `map.on('click'/'mousemove')` and `map.unproject(point) → lng/lat`.
2. **Coord boundary (`coords/geo.ts`)** — `lng/lat → local planar metres → mm` via a fixed
   Singapore origin (equirectangular linearization). **All** Mercator/float-geo math lives here.
3. **Command** — intent becomes a typed `Command` (`PlaceStation{x_mm,y_mm}`, `CreateLine`,
   `AddStop`, `AssignTrainset`, `SetHeadway`, `SetRunning`). TS **never** mutates sim state;
   it `JSON.stringify`s the command → `sim.apply_command_json(json)` and pushes to an in-memory
   command log (save = seed + log; future multiplayer = exchange commands, not state).
4. **Sim tick** — one `requestAnimationFrame` loop runs a fixed-timestep accumulator (Gaffer
   "Fix Your Timestep"), stepping the Rust sim at a constant `dt` (~20–30 Hz). The pure core
   runs one strict ordered phase per tick: *advance clock → spawn+route pax (seeded RNG) →
   dispatch trains by headway → move trains (arc-length + trapezoidal profile) → alight/board
   (capacity cap) → accounting*. Time = `i64` sim-ms; positions = `i64` mm; iteration is
   index-ordered over `Vec`/slab — bit-reproducible.
5. **State buffer** — sim keeps **pre-sized, never-reallocated** SoA `Vec<f32>` render buffers
   (vehicle x/y/angle, prev + current). **For the slice: copy them into a reused JS `Float32Array`
   each frame** (do NOT use zero-copy views — they detach on heap growth; the copy is negligible).
6. **Render (interpolated 60fps)** — each frame computes `alpha = accumulator/dt` and lerps each
   vehicle between prev/current snapshot **along the polyline arc-length** (not raw x/y — avoids
   corner-cutting), converts metres → lng/lat at the boundary, and feeds deck.gl via the binary
   `data.attributes` path. `overlay.setProps({layers})` with **stable data-object identity**;
   `updateTriggers` bump **only** on topology change. **Never** rebuild layers per frame.
7. **Stats (low-frequency)** — on a ~1–4 Hz throttle, `sim.stats()` returns a structured object
   via serde-wasm-bindgen (ridership, per-station boardings/alightings, load factor, waiting
   counts, 0–100 coverage score). Vanilla-TS UI renders it.

**ASCII diagram:**

```
+=============================================================================+
|                            BROWSER (main thread)                            |
|  +-----------------------------+        +------------------------------+    |
|  |   UI (vanilla TS)           |        |   MapLibre GL JS v5.24        |    |
|  |  LineListPanel (left)       |        |   - Singapore basemap         |    |
|  |  EditorPanel (right)        |        |   - camera / pan / zoom       |    |
|  |  StatsBar + TransportBar    |        |   - pointer events            |    |
|  |  (bottom: play/pause/speed) |        |   - map.unproject(px)->lnglat |    |
|  +--------------+--------------+        +---------------+--------------+    |
|                 | user intent                          | click/move        |
|                 v                                       v                   |
|  +-----------------------------+ picking +------------------------------+   |
|  |  DrawingStateMachine.ts     |<------->|  deck.gl 9.3 MapboxOverlay   |   |
|  |  place-station / draw-line  | onClick |  (overlaid, interleaved:false)|  |
|  |  dragPan disable during draw| onHover |  PathLayer (lines, blueprint) |  |
|  +--------------+--------------+         |  ScatterplotLayer (stations,  |  |
|                 | lng/lat                |    catchment, waiting pax)    |  |
|                 v                        |  IconLayer (vehicles)         |  |
|  +-----------------------------+         +---------------+--------------+   |
|  |  coords/geo.ts  BOUNDARY    |   interp lnglat         ^ data.attributes  |
|  |  lnglat <-> metres <-> mm   +-------------------------+ (Float32Array)   |
|  +--------------+--------------+                                            |
|                 | Command{x_mm,y_mm,...}            ^ lerp(prev,cur,alpha)  |
|                 v JSON.stringify                    | along arc-length      |
|  +=========================================================================+|
|  |  GameLoop.ts  (rAF)  fixed-timestep accumulator                         ||
|  |  while(acc>=dt){ sim.tick(dt); acc-=dt }   alpha=acc/dt ; render(alpha)  ||
|  +=========================================================================+|
|                 | apply_command_json(json)   ^ copy SoA f32 -> reused JS arr|
|                 | tick(dt_ms)                | stats() (serde, low-freq)     |
|                 v                            |                               |
|  +=========================================================================+|
|  |  crates/sim-wasm  (#[wasm_bindgen] Sim facade, cdylib)  [WASM]          ||
|  |  Sim{ core: sim::World }  — NO game logic, only the boundary            ||
|  +=========================================================================+|
|                 | (no wasm types cross here)                                |
|                 v                                                           |
|  +=========================================================================+|
|  |  crates/sim  (PURE deterministic core, host-testable)                   ||
|  |  World{ clock_ms:i64, rng:ChaCha8Rng, stations, lines, trainsets,       ||
|  |         vehicles(SoA), pax(slab), demand_grid, stats, cmd_log }         ||
|  |  apply(Command)  tick(dt_ms)  state_hash()->u64                         ||
|  |  trait Router (DirectRide now; RAPTOR-shaped data) | trait Demand        ||
|  +=========================================================================+|
+=============================================================================+
   committed build-time assets (never fetched at runtime):
   packages/app/public/singapore.pmtiles            (gitignored; T7 only)
   packages/app/public/data/singapore_demand.json   (committed coarse grid)
   packages/app/public/data/singapore_city.json     (committed CityData manifest)
```

**Why this shape:** the pure core compiles natively → determinism tests run as fast `cargo test`
(no browser). The wasm wrapper is the only place JS types / the metres↔lng-lat boundary appear,
so the core stays portable (reusable server-side for future authoritative multiplayer) and
bit-reproducible. Every action being a `Command` applied to a pure `apply()+tick()` buys replay,
undo, save/load, and lockstep at once. RAPTOR-shaped routing data means the single-line direct-ride
stub is literally "RAPTOR round 1" and transfers/bus/HSR are additive. lng/lat at the boundary +
a `CityData` contract means GTFS and the other 8 cities are pure data, not refactors.

---

## 4. Repo layout

```
onlytransits/
├── idea.md                         # existing seed (do not delete)
├── PLAN.md                         # this file
├── PROGRESS.md                     # flight recorder — created in T1, updated every checkpoint
├── README.md                       # quickstart: pnpm i && pnpm build:wasm && pnpm dev
├── ATTRIBUTION                     # MANDATORY ODbL: OpenStreetMap + Protomaps credit (commit 1)
├── .gitignore                      # target/, node_modules/, packages/wasm-sim/pkg/, *.pmtiles, *.pbf
├── Cargo.toml                      # [workspace] members = ["crates/sim","crates/sim-wasm"]
├── Cargo.lock                      # COMMITTED (pins rand/rand_chacha/wasm-bindgen for determinism)
├── rust-toolchain.toml             # channel + targets=["wasm32-unknown-unknown"]
├── pnpm-workspace.yaml             # packages: ["packages/*"]
├── pnpm-lock.yaml                  # COMMITTED
├── package.json                    # root scripts: build:wasm, dev, build, test, e2e
├── .github/workflows/ci.yml        # job A: cargo test + wasm-pack; job B: vitest + playwright
│
├── crates/
│   ├── sim/                        # PURE deterministic core — NO wasm-bindgen
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # re-exports World, Command, Event, ids
│   │       ├── world.rs            # World + apply(Command) + tick(dt) + state_hash()
│   │       ├── command.rs          # enum Command (serde) + enum Event (serde)
│   │       ├── ids.rs              # newtype StationId/LineId/TrainsetId/VehicleId/PaxId (u32)
│   │       ├── geo_local.rs        # i64 mm planar math (NO lng/lat, NO Mercator)
│   │       ├── station.rs          # Station{ id, name, pos_mm }
│   │       ├── line.rs             # Line{ id, color, stops, polyline, arclen[] }
│   │       ├── trainset.rs         # Trainset spec (capacity, v_max, accel, decel, dwell)
│   │       ├── vehicle.rs          # SoA vehicle buffers + trapezoidal motion integrator
│   │       ├── dispatch.rs         # headway-based dispatcher (timetable seam)
│   │       ├── demand/{mod.rs,gravity.rs}   # trait Demand + grid capture + gravity dest pick
│   │       ├── routing/{mod.rs,direct.rs}   # trait Router (RAPTOR-shaped) + DirectRide stub
│   │       ├── pax.rs              # passenger slab + per-station FIFO board/alight queues
│   │       ├── stats.rs            # Stats + coverage/satisfaction (0-100)
│   │       ├── tick.rs             # the strict ordered phase loop (determinism heart)
│   │       ├── city.rs             # CityData + DemandGrid deserialization (serde_json)
│   │       └── render_buf.rs       # pre-sized SoA f32 buffers for copy-out
│   │   └── tests/
│   │       ├── determinism.rs      # replay cmd log twice -> equal state_hash
│   │       ├── vehicle.rs          # motion advances + deterministic
│   │       ├── catchment.rs        # no-double-count invariant
│   │       └── ridership.rs        # board/ride/alight -> ridership>0 + deterministic
│   │
│   └── sim-wasm/                   # THIN wasm-bindgen wrapper — ONLY wasm-aware crate
│       ├── Cargo.toml              # crate-type=["cdylib"]; wasm-bindgen="=0.2.117"
│       └── src/lib.rs              # #[wasm_bindgen] Sim; new/apply_command_json/tick/ptr getters/stats
│
├── packages/
│   ├── wasm-sim/pkg/               # wasm-pack OUTPUT (gitignored), consumed as workspace dep
│   └── app/                        # Vite TS frontend
│       ├── package.json            # deps: maplibre-gl, @deck.gl/*, wasm-sim, (pmtiles T7)
│       ├── vite.config.ts          # vite-plugin-wasm; build.target='esnext'
│       ├── tsconfig.json
│       ├── index.html              # full-screen #map; sets window flags
│       ├── public/data/
│       │   ├── singapore_demand.json   # committed coarse demand grid
│       │   └── singapore_city.json     # committed CityData manifest
│       └── src/
│           ├── main.ts             # bootstrap: init map + overlay + loop; set window.__APP_READY
│           ├── config.ts           # SG origin lng/lat, tick rate, catchment defaults
│           ├── map/{basemap.ts,overlay.ts}    # MapLibre init + deck layer factories
│           ├── sim/{SimBridge.ts,GameLoop.ts,interpolate.ts}
│           ├── coords/geo.ts       # THE coord boundary: lnglat<->metres<->mm
│           ├── commands/{codec.ts,log.ts}     # JSON command encode + in-memory log
│           ├── tools/{DrawingStateMachine.ts,selection.ts}
│           ├── ui/{LineListPanel.ts,EditorPanel.ts,StatsBar.ts,TransportBar.ts,onboarding.ts}
│           ├── testhooks.ts        # window.__ot_test.placeStationLngLat(...) for deterministic e2e
│           ├── types.ts            # TS mirror of Command/Event/Stats/CityData
│           └── styles.css
│       ├── test/{geo.test.ts,sim.test.ts}     # Vitest
│       └── e2e/{load.spec.ts,map.spec.ts,deck.spec.ts,vehicle-move.spec.ts,slice.spec.ts}
│
├── scripts/{build_data.sh,build_demand.py}    # offline asset build (T7/T13, off critical path)
└── playwright.config.ts            # webServer = vite preview on CI / dev locally; fixed viewport
```

---

## 5. Rust ↔ TS data contracts (corrected)

- **`Sim::new(seed: u64, city_json: &str)`** — `city_json` = `JSON.stringify(CityData)` with the
  demand grid embedded (placeholder/empty until T13). Rust `serde_json`-parses → `CityData` +
  `DemandGrid`, seeds `ChaCha8Rng::seed_from_u64(seed)`, pre-sizes all render/vehicle buffers.
- **`Sim::apply_command_json(json: &str)`** — TS sends `JSON.stringify(cmd)`. Rust
  `serde_json`-deserializes `enum Command` → `World::apply`. **Positions are local mm (`i64`),
  never lng/lat.** Variants (slice): `PlaceStation{x_mm,y_mm,name?}`, `CreateLine{color:u32}`,
  `AddStop{line:u32,station:u32,after:Option<u32>}`, `AssignTrainset{line:u32,spec:u8,count:u16}`,
  `SetHeadway{line:u32,headway_ms:i64}`, `SetRunning{running:bool}`.
- **`Sim::tick(dt_ms: f64)`** — called by the accumulator at constant dt (e.g. 50.0 → 20 Hz).
  Cast to `i64` internally; advances exactly one ordered phase pass.
- **`Sim::refresh_render(&mut self)`** — recomputes the SoA f32 buffers (prev+current x/y/angle)
  from `World`. Called once per step before TS reads. No allocation, never grows memory.
- **Vehicle SoA getters (copy-out hot path):** `veh_count()->usize`, `veh_x_ptr()->*const f32`,
  `veh_y_ptr()`, `veh_prev_x_ptr()`, `veh_prev_y_ptr()`, `veh_angle_ptr()`, `veh_line_ptr()->*const u32`.
  TS builds a `Float32Array(memory.buffer, ptr, count)` and **copies into a reused JS array** each
  frame (re-acquire if `memory.buffer` changed). Buffers are fixed-capacity, never realloc'd mid-frame.
- **`Sim::stats() -> JsValue`** (serde-wasm-bindgen, LOW FREQUENCY): `{ sim_clock_ms, ridership_total,
  per_station:[{station_id,boardings,alightings,waiting}], avg_load_factor, waiting_total, left_behind,
  coverage_score }`. **Expose numerics as `f64`/`number`** (or `Number()` in TS) — `i64/u64` marshal
  as `BigInt`.
- **`Sim::state_hash() -> ...`** — for determinism test. If returning `u64`, remember it's `BigInt` in JS.
- **Save artifact (Rust-side):** `postcard` bytes of `{ seed:u64, commands:[Command] }`. Replay =
  `Sim::new(seed, city)` then `apply_command_json` per logged command. Determinism test asserts two
  replays yield equal `state_hash`.
- **`CityData` (`singapore_city.json`)** — WGS84 everywhere: `{ id, name, originLngLat:[103.8198,1.3521],
  bbox:[103.55,1.13,104.15,1.50], center, zoom:11, pmtilesPath, demandGridPath, seed }`. First-class
  committed artifact with a `cargo test` that `serde_json`-parses it into `CityData`.
- **`DemandGrid` (`singapore_demand.json`)** — `{ cell_m:300, bbox, cells:[{lon,lat,origin_weight,dest_weight}] }`.
  TS converts each lon/lat → local mm at load and embeds into `Sim::new`. Consumed only by
  `sim::demand` — never fetched at runtime.

---

## 6. Simulation design

All maps to recognized, extensible algorithms; each collapses to a near-trivial slice form.

- **Catchment** — fixed-radius circular buffer (default ~500m, configurable) over the coarse
  demand grid. Assign each cell's weight to in-range stations via **normalized** inverse-distance /
  negative-exponential decay (gravity deterrence). **Normalize so total captured per cell ≤ cell
  weight** (NIMBY's shared-overlap model). Min-population floor → tiny catchments generate ~0 pax.
- **Demand / OD** — gravity model. Slice form: spawn pax at each station at a deterministic
  (seeded `ChaCha8Rng`) rate ∝ `captured_origin_weight × demand_factor` (~0.25); each pax picks a
  destination among the **other stations on its line** weighted by `dest_coverage × (1 + lines_serving) ×
  distance_decay`. (Full O(n²) OD matrix + time-of-day = multi-line generalization, deferred.)
- **Routing** — `trait Router` over **RAPTOR-shaped** data (`route → ordered trips → stop_times`,
  per-stop route list, footpaths). Ship only `DirectRide` (= RAPTOR round 1): board next train,
  ride to stop. Transfers/bus/HSR = enabling rounds K>1 + footpaths, additive.
- **Vehicle movement** — line = polyline with precomputed cumulative arc-length; vehicle state =
  scalar distance `s`. Advance per tick with a trapezoidal accel/cruise/brake profile (one `v_max`,
  one `accel`, one `decel`) + fixed dwell at stops; reverse at ends (out-and-back). **Dispatch by
  headway** (frequency-based; waiting ≈ headway/2). Timetables = generalization.
- **Ridership** — per-station FIFO queue keyed by destination. On arrival: alight pax whose
  dest == stop, then board FIFO up to **capacity** (load factor); leftover pax keep waiting and
  accumulate. Log boardings/alightings/load/waiting/left-behind. **Clamp trains-per-line / min
  headway so vehicle count never exceeds the pre-sized SoA capacity** (unit-tested).
- **Tick order (the determinism heart)** — one strict ordered pass per integer tick:
  `clock → spawn+route → dispatch → move (detect arrivals) → alight then board (cap) → despawn → accounting`.
  `i64` ms time, `i64` mm positions, index-ordered `Vec`/slab iteration, **no `std::HashMap`
  iteration, no wall-clock, no float-Mercator** in the core. Render decoupled via accumulator + alpha.
- **Determinism contract** — same seed + same ordered command log ⇒ identical `state_hash`, proven
  by replaying twice in one native process. `ChaCha8Rng` state is serde-serializable so RNG resumes
  on load. (Cross-machine float lockstep would need fixed-point later — isolate state-affecting math
  so that swap stays local; f64 in one wasm binary is deterministic enough for the slice.)

---

## 7. Game design & UX

**The 5-step fun loop** (intersection of NIMBY / OpenTTD / A-Train / Mini Metro):
*place station (catchment circle appears) → draw line clicking station-to-station →
assign trainset + set one Headway slider → press Play (animated trains + waiting-pax dots) →
read live Ridership + a 0–100 Coverage/Satisfaction gauge → tweak.* Each step gives instant feedback.

- **Build tools (2):** click-to-place-station; click-station-to-station draw-line with **snap-to-station**
  (generous pixel radius), rendered as a dashed **blueprint** PathLayer following the cursor, committed
  on a Build action (every commit = one deterministic command). Disable `map.dragPan` during a draw
  gesture. Single **out-and-back** topology only. Line == implicit right-of-way (Mini-Metro fusion);
  **no separate track-laying, junctions, or signals.**
- **Frequency:** one **Headway slider** (e.g. 2–20 min), auto-suggested default on assign; derive
  train count ↔ headway. No manual per-stop timetables.
- **Scoring:** live **Ridership** counter + a single prominent **0–100 Coverage/Satisfaction** gauge
  = blend of (% of catchment demand served) and (wait-vs-headway penalty), **monotonic** (improvements
  always raise it). **No money/fares/costs.**
- **UI layout:** map fills the screen; **left** = collapsible line list (swatch, name, ridership);
  **right** = contextual editor (name, color, trainset, headway slider); **bottom** = Build/Run toggle,
  pause/play, 3 speeds (1×/10×/max), the headline ridership counter + coverage gauge.
- **Build vs Run mode:** Build = paused + editable; Run = sim ticking. Build→Run commits blueprints
  as ordered commands (the command-sourcing seam).
- **One auto-assist:** auto-name stations (`Station N` for the slice; nearest-OSM-name is deferred)
  + auto-suggest a sensible default headway. Nothing heavier.
- **Onboarding:** one animated ghost gesture on first load + a one-line objective + tooltips. No
  scripted tutorial. *(Ghost-gesture / connect-chime are "cut first" polish — see T17.)*
- **Feel (P0, not polish):** translucent catchment circles, color-coded line polylines, animated
  vehicle dots, accumulating waiting-pax dots at stations, a connect flash on commit. Cap visible
  catchments (selected/hovered only) to avoid alpha over-saturation.

---

## 8. Data pipeline

- **Basemap (off critical path):** ship on a **hosted free style** (CARTO Positron / MapLibre demo)
  first — zero data tooling, network-stable Playwright. **T7 (deferred upgrade):** self-host a single
  Singapore-bbox `singapore.pmtiles` built by the prebuilt `go-pmtiles` CLI (`pmtiles extract <dated
  Protomaps daily build> --bbox=103.55,1.13,104.15,1.50 --maxzoom=14`), wired via **pmtiles v4 API**
  (`import { Protocol } from 'pmtiles'; const p = new Protocol(); maplibregl.addProtocol('pmtiles', p.tile)`
  — call once). The daily-build URL is dated and has no index → resolve a valid recent date by walking
  back ~10 days with `HEAD` checks. Gitignore the `.pmtiles` (fetched artifact) and the 233MB pbf.
- **Demand grid (committed):** go **straight to a deterministic seeded synthetic grid** —
  `scripts/build_demand.py` writes `singapore_demand.json` as ~250–500m cells over the Singapore bbox
  with pseudo-random origin/dest weights biased toward the central/southern urban band, zero over
  obvious sea cells. The sim consumes only this committed JSON. Real `pyrosm`/OSM-derived weights are
  a post-slice nicety (do **not** spend overnight budget installing geopandas).
- **Licensing:** visible **OpenStreetMap (+ Protomaps when T7)** attribution via MapLibre
  `AttributionControl` from commit 1, plus an `ATTRIBUTION` file (ODbL Produced Work obligation).

---

## 9. Milestones

| ID | Title | Demoable outcome |
|----|-------|------------------|
| **M0** | Walking skeleton: repo + green tests in all 3 tiers | `cargo test --workspace` + `pnpm -w test` green; `pnpm --filter app dev` serves a mounted blank app; Playwright loads + screenshots. |
| **M1** | Singapore map renders & is interactive | Pan/zoom a real Singapore basemap; a deck.gl test marker anchored at the origin; OSM attribution visible. |
| **M2** | WASM sim bridge proven | `Sim::new(seed,city_json)` + `apply_command_json` + `tick` + copy-out vehicle buffers work in the app; native determinism test + Vitest wasm-in-node smoke pass. |
| **M3** | Build tools | Click to place stations (catchment circle), draw a line (blueprint→commit), assign a trainset + headway — all via the command log; network persists & renders. |
| **M4** | Live sim | Press Play: animated trains glide on a headway; waiting-pax counts grow at stations and drop on boarding; ridership climbs; pause/speed work. |
| **M5** | Stats readout + full slice verified e2e | Bottom stats bar (ridership + 0–100 coverage gauge); committed Playwright spec proves the whole loop against the production bundle. |

---

## 10. Task graph

> **Reordered per critique:** the critical path makes a vehicle **visible on screen (T15)** *before*
> sinking hours into demand math (T16), so the earliest "is the sim even working" checkpoint isn't
> gated behind the hardest task. **Corrected critical path:**
> `T1 → T2 → T8 → T9 → T5 → T6 → T10 → T11 → T14 → T15 → T16a → T16b → T17 → T18`.

Each task lists **deps**, **acceptance**, and a **runnable verify** command. `(S/M/L/XL)` = size;
`∥` = parallelizable with siblings at the same dependency level.

### M0 — Walking skeleton

**T1 — Scaffold pnpm + Cargo monorepo skeleton + PROGRESS.md + lockfiles** *(M)* — deps: none
- Create the full layout (§4): root `pnpm-workspace.yaml`, root `Cargo.toml` workspace
  (`crates/sim`, `crates/sim-wasm`), `crates/sim` (pure lib, edition 2021), `crates/sim-wasm`
  (`crate-type=["cdylib"]`, `wasm-bindgen="=0.2.117"`), `packages/app` (Vite + TS, vite 8.x).
  `.gitignore`, `ATTRIBUTION` (OSM + Protomaps), `README`, `rust-toolchain.toml`.
- **Create `PROGRESS.md`** with: a checklist mirroring M0–M5; a timestamped running log; a
  **resolved-versions block** (fill in actual installed rust/wasm-bindgen/node versions); a
  known-gaps/deferred section.
- Pin npm deps: `maplibre-gl@5.24.x`, `@deck.gl/core@9.3.x`+`@deck.gl/layers@9.3.x`+`@deck.gl/mapbox@9.3.x`
  (**identical** minor), `vite-plugin-wasm@^3.6` (**no** top-level-await), `vitest@4.x`, `@playwright/test`.
  Set `build.target:'esnext'`.
- **First commit on `main`** (scaffold + license + .gitignore + ATTRIBUTION + PROGRESS.md), then
  branch `slice/singapore-vertical` for all further work. **Commit `Cargo.lock` + `pnpm-lock.yaml`.**
- *Acceptance:* workspace resolves; both crates are members; deps pinned as above; both lockfiles
  committed; PROGRESS.md exists with the resolved-versions block.
- *Verify:* `pnpm install && cargo metadata --format-version 1 --no-deps | grep -q sim-wasm && echo OK`

**T2 — Sim core: World, Command, state_hash, determinism replay test** *(M)* — deps: T1
- `crates/sim`: `World{ clock_ms:i64, rng:ChaCha8Rng, ... , cmd_log:Vec<Command> }`; seeded
  `ChaCha8Rng::seed_from_u64` (**`use rand::{RngExt, SeedableRng}`**; `rand="=0.10.1"`,
  `rand_chacha="=0.10.0"`). `Vec`-based ordered containers only — **no `std::HashMap`** in sim logic.
- `enum Command { PlaceStation{x_mm:i64,y_mm:i64,name:Option<String>}, CreateLine{color:u32},
  AddStop{...}, AssignTrainset{...}, SetHeadway{line:u32,headway_ms:i64}, SetRunning{running:bool} }`
  with serde derive (**mm, not lng/lat**). `fn apply(&mut self, &Command)`, `fn tick(&mut self, i64)`,
  `fn state_hash(&self)->u64` (FNV-1a over canonical ordered serialization).
- Determinism test: two Worlds, same seed + same command log + N ticks ⇒ equal `state_hash`. JSON
  round-trip test for `Command` (`serde_json`); postcard round-trip for the save artifact.
- *Verify:* `cargo test -p sim --release`

**T3 — Vite app shell + Vitest smoke + Playwright load spec** *(M, ∥ with T2)* — deps: T1
- `index.html` (full-screen `#map`, sets `window.__APP_READY`). `vite.config.ts` with
  `vite-plugin-wasm` + `build.target:'esnext'` (document the fallback in a comment). Vitest config +
  one smoke test (`coords/geo.ts` round-trip). `playwright.config.ts`: webServer = `dev` locally /
  `preview` on CI (`process.env.CI`), `reuseExistingServer:!CI`, **fixed viewport**, `screenshot:'only-on-failure'`.
  `e2e/load.spec.ts`: navigate, wait for `__APP_READY`, assert title, screenshot.
- *Verify:* `pnpm --filter app build && pnpm --filter app exec vitest run && pnpm --filter app exec playwright install --with-deps chromium && pnpm --filter app exec playwright test e2e/load.spec.ts`

**T4 — CI workflow (cached cargo + wasm feeding JS tests)** *(S, ∥)* — deps: T2, T3
- `.github/workflows/ci.yml`: job A (Swatinem/rust-cache → `cargo test --workspace` + `build:wasm`,
  upload `packages/wasm-sim/pkg` artifact); job B (pnpm + node 24 → download artifact, `vitest run`,
  build app, cache Playwright browsers, `playwright test` vs preview). Root script
  `build:wasm = wasm-pack build crates/sim-wasm --target web --out-dir packages/wasm-sim/pkg`.
- **Not required to pass overnight** (per defer list) — must not block the loop.
- *Verify:* YAML parses (`npx --yes js-yaml .github/workflows/ci.yml`); `build:wasm` produces `pkg/*.wasm`.

### M2 — WASM bridge *(built before the map so the bridge is proven early)*

**T8 — sim-wasm wrapper: `Sim::new(seed, city_json)` + `apply_command_json` + tick + SoA + wasm smoke** *(M)* — deps: T2
- `#[wasm_bindgen] Sim{ core: sim::World }`: **`new(seed:u64, city_json:&str)` from the start**
  (parses CityData + possibly-empty demand grid). `apply_command_json(json:&str)` (serde_json →
  `Command` → `World::apply`). `tick(dt_ms:f64)`. `state_hash()`. SoA getters
  (`veh_*_ptr`/`veh_count`), pre-allocated fixed capacity, never realloc'd mid-tick.
  `stats()->JsValue` via serde-wasm-bindgen (numerics as `f64`/`number`).
- `wasm-pack build crates/sim-wasm --target web --out-dir packages/wasm-sim/pkg`.
- **Fail-fast smoke (acceptance):** a node one-liner imports `pkg` and calls `tick()`; the build
  checkpoint **fails if instantiation throws** (guards wasm-bindgen version skew).
- *Verify:* `cargo build -p sim-wasm --target wasm32-unknown-unknown && wasm-pack build crates/sim-wasm --target web --out-dir packages/wasm-sim/pkg && ls packages/wasm-sim/pkg/*.wasm`

**T9 — TS SimBridge: import wasm, copy-out Float32Array, JSON command encode, Vitest node smoke** *(L)* — deps: T8, T3
- `sim/SimBridge.ts`: import the `wasm-sim` workspace package, instantiate `Sim`. `applyCommand(cmd)`
  = `apply_command_json(JSON.stringify(cmd))` + push to log. `tick(dt)`. `readVehicleBuffers()` builds
  a `Float32Array` view on `wasm.memory.buffer` from the SoA ptrs and **copies into a reused JS array**
  (re-acquire view each call; never read a detached buffer). `stats()` (handle `BigInt` → `Number`).
- Vitest node smoke (vite-plugin-wasm enables wasm in node): create `Sim` with a minimal valid
  CityData+demand JSON, apply PlaceStation+CreateLine+AddStop+AssignTrainset, tick, assert vehicle
  buffer length > 0 and identical `state_hash` across two identical runs.
- *Verify:* `pnpm --filter app exec vitest run test/sim.test.ts`

**T13 — Demand grid asset: deterministic synthetic Singapore grid (+ singapore_city.json)** *(M, ∥)* — deps: T1
- `scripts/build_demand.py` writes `packages/app/public/data/singapore_demand.json`: ~250–500m cells
  over the Singapore bbox, **deterministic seeded** origin/dest weights (urban-band biased, zero over
  sea). **Do not attempt pyrosm overnight.**
- Author `singapore_city.json` as a first-class committed artifact with exact fields (§5) + a `cargo
  test` that `serde_json`-parses a committed copy into `CityData`.
- *Verify:* `python3 scripts/build_demand.py && node -e "const g=require('./packages/app/public/data/singapore_demand.json'); if(!Array.isArray(g.cells)||!g.cells.length) process.exit(1); console.log('cells',g.cells.length)"`

### M1 — Map

**T5 — MapLibre Singapore basemap (hosted style) + attribution + geo.ts** *(M)* — deps: T3
- Full-screen MapLibre v5 centered on Singapore (`[103.8198,1.3521]`, zoom 11), CSS imported,
  `AttributionControl` (OSM credit). **Hosted free style first** (CARTO Positron / MapLibre demo).
  On `idle` set `window.__MAP_READY=true`. `coords/geo.ts` exports `lngLatToMeters`/`metersToLngLat`
  (equirectangular around the fixed SG origin) + `metersToMm`/`mmToMeters`, with a Vitest unit test.
- *Verify:* `pnpm --filter app exec vitest run test/geo.test.ts && pnpm --filter app build && pnpm --filter app exec playwright test e2e/map.spec.ts`

**T6 — deck.gl MapboxOverlay (overlaid mode) + test layer** *(M)* — deps: T5
- Add deck.gl 9.3 via `@deck.gl/mapbox` `MapboxOverlay` (`interleaved:false`) with `map.addControl`.
  One static `ScatterplotLayer` at the SG origin (`radiusUnits:'meters'`, `pickable:true`). `overlay.setProps`
  with **stable data identity** (no per-frame rebuild). e2e: deck canvas exists + the marker pixel is
  non-background after `__MAP_READY`, and stays anchored on pan/zoom.
- *Fallback (45-min time-box):* MapLibre-native GeoJSON `line`/`circle` layers + a synced 2D `<canvas>`
  for vehicles. Keeps the loop; loses GPU picking. Do **not** touch PixiJS/regl.
- *Verify:* `pnpm --filter app build && pnpm --filter app exec playwright test e2e/deck.spec.ts`

### M3 — Build tools

**T10 — Place-station tool: click → unproject → PlaceStation{x_mm,y_mm} → station + catchment layer** *(L)* — deps: T6, T9
- `map.on('click')` → `unproject` → `geo.ts` lng/lat → metres → **mm** → `PlaceStation` command via
  SimBridge. Sim assigns an auto-incrementing id; auto-name `Station N`. Render stations as a pickable
  `ScatterplotLayer`; each catchment as a translucent `ScatterplotLayer` (`radiusUnits:'meters'`, ~500m,
  low alpha, stroked) — show only selected/hovered to avoid alpha stacking. Build/Run toggle (Build = paused).
- *Verify:* `pnpm --filter app build && pnpm --filter app exec playwright test e2e/place-station.spec.ts`

**T11 — Draw-line tool: snap-to-station, blueprint polyline, commit (CreateLine + AddStop)** *(L)* — deps: T10
- Click an existing station to start; click subsequent stations to append (snap within a generous
  pixel radius). Render in-progress as a dashed blueprint `PathLayer` following the cursor; commit on
  double-click/Enter/button → emit `CreateLine` + ordered `AddStop` commands. Disable/re-enable
  `map.dragPan` around the gesture. Committed lines render as a `PathLayer` (`widthUnits:'pixels'`,
  `widthMinPixels`, rounded caps, per-line color). **Out-and-back only.** Sim stores ordered station
  positions + precomputes cumulative arc-length.
- *Verify:* `pnpm --filter app build && pnpm --filter app exec playwright test e2e/draw-line.spec.ts`

**T12 — Assign trainset + headway slider (auto-suggested default) + left line list** *(M)* — deps: T11
- Contextual right panel on line select: "Assign Trainset" (one fixed type, fixed capacity) + one
  Headway slider (2–20 min → ticks). Assign → `AssignTrainset` + auto-suggested default headway;
  slider → `SetHeadway`. **Clamp min headway / max trains-per-line so vehicle count never exceeds the
  pre-sized SoA capacity** (Rust unit test). Left panel lists lines (swatch, name). No movement yet (T14).
- *Verify:* `pnpm --filter app build && pnpm --filter app exec playwright test e2e/assign-trainset.spec.ts`

### M4 — Live sim

**T14 — Vehicle movement: arc-length + trapezoidal profile + headway dispatch (Rust)** *(L)* — deps: T8, T11
- Dispatch vehicles per headway; advance scalar arc-length `s` per tick with a trapezoidal
  accel/cruise/brake profile + fixed dwell; reverse at line ends. Write x/y (interpolated along
  polyline vertices) + heading into the SoA `Vec<f32>` buffers. Integer/fixed-tick math, stable
  iteration. `cargo test`: 2-station line, assign trainset, tick → vehicle advances monotonically;
  identical across two replays (`state_hash`).
- *Verify:* `cargo test -p sim --release vehicle_movement`

**T15 — Fixed-timestep animation loop: tick sim, interpolate, render moving vehicles at 60fps** *(L)* — deps: T14, T9, T6
- Gaffer fixed-timestep accumulator in one rAF loop: step `sim.tick(dt)` at ~20–30 Hz; each frame
  `alpha=acc/dt`, interpolate vehicle positions **along the path** (arc-length param, not raw x/y),
  metres → lng/lat at the boundary into a persistent `Float32Array`, feed deck.gl via `data.attributes`
  (IconLayer/ScatterplotLayer), `setProps` with stable identity (bump `updateTriggers` only on topology
  change). Pause/Play + 1×/10×/max. Vehicles pickable.
- **This is the early "vehicles visibly move" checkpoint (CP6) — before demand math.** Add an interim
  `e2e/vehicle-move.spec.ts` asserting a vehicle's lng/lat changed between two samples (no ridership yet).
- *Verify:* `pnpm --filter app build && pnpm --filter app exec playwright test e2e/vehicle-move.spec.ts`

**T16a — Catchment capture + seeded passenger spawn (Rust)** *(L)* — deps: T14, T13
- Load the committed demand grid (passed via `city_json` at init). Assign each cell's origin/dest
  weight to in-range stations by **normalized** distance-decay within the catchment radius.
- Per tick: spawn pax at stations at a deterministic seeded rate ∝ `captured_origin × demand_factor`;
  each pax picks a destination among other stations on its line via `dest_coverage × (1+lines_serving) ×
  distance_decay`. Routing behind `trait Router` with a `DirectRide` impl over RAPTOR-shaped data.
- **Concrete no-double-count unit test:** two stations whose catchments overlap one high-weight cell —
  assert the captured weight across both **sums to the cell weight** (±ε), and a far station captures 0.
- *Verify:* `cargo test -p sim --release catchment`

**T16b — Board/ride/alight + capacity + stats (Rust)** *(L)* — deps: T16a
- Per-station FIFO queues keyed by destination. On arrival: alight `dest==stop`, then board up to
  capacity (load factor); leftover keeps waiting. Log boardings/alightings/load/waiting/left-behind +
  total ridership; compute the 0–100 coverage/satisfaction score (monotonic). `cargo test`: 3-station
  serviced line ⇒ ridership > 0; identical across replays (`state_hash`).
- *Verify:* `cargo test -p sim --release ridership`

### M5 — Readout + e2e

**T17 — Stats bar + coverage gauge + waiting-pax dots (must-have) | ghost-gesture/chime (cut-first)** *(L)* — deps: T16b, T15, T12
- **Must-have:** bottom stats bar — live ridership counter + prominent 0–100 coverage gauge, read from
  `stats()` at ~1–4 Hz (handle `BigInt`). Accumulating waiting-pax dots at stations (deck layer from
  per-station waiting). Per-line ridership in the left list.
- **Cut-first polish (drop under time pressure, do not let it block the loop):** connect flash + chime
  on commit; first-load ghost-gesture hint + one-line objective.
- *Verify:* `pnpm --filter app build && pnpm --filter app exec playwright test e2e/stats.spec.ts`

**T18 — End-to-end vertical-slice Playwright spec vs the production bundle** *(M)* — deps: T17
- `e2e/slice.spec.ts` drives the full loop against `vite preview` with a **fixed viewport + fixed
  map center/zoom**, and places stations via the **deterministic test hook**
  `window.__ot_test.placeStationLngLat(lng,lat)` (camera-independent) rather than raw pixel clicks
  (raw-click path covered by a separate non-flagship spec). Place ≥3 stations → draw a line → assign
  trainset + headway → Play → run a few seconds. **Assert concrete facts:** (a) a vehicle's lng/lat
  changed between two samples, (b) DOM ridership > 0, (c) coverage gauge changed from initial. Wait on
  `window` flags / `data-testid`, never fixed sleeps. Capture `page.screenshot()`.
- *Verify:* `pnpm --filter app build && CI=1 pnpm --filter app exec playwright test e2e/slice.spec.ts`

### Deferred / off critical path

**T7 — Self-host Singapore PMTiles (pmtiles v4) — DEFER unless M0–M5 are green with time to spare** *(L, ∥)* — deps: T5
- `scripts/build_data.sh`: download `go-pmtiles`, resolve a valid recent Protomaps daily-build date
  (HEAD-walk-back ~10 days), `pmtiles extract --bbox=103.55,1.13,104.15,1.50 --maxzoom=14`. Wire the
  **v4 API** (`new Protocol()`, `addProtocol('pmtiles', p.tile)` once) + a protomaps light/grayscale
  flavor. Hosted style remains the documented fallback. Ensure **nothing in M2–M5 depends on the
  PMTiles path** (it's a leaf). Most cuttable large task.
- *Verify:* `bash scripts/build_data.sh && test -f packages/app/public/singapore.pmtiles && pnpm --filter app build && pnpm --filter app exec playwright test e2e/map.spec.ts`

---

## 11. Checkpoints (= commit gates; determinism re-gates every commit)

- **CP0 (Walking skeleton)** — workspaces exist; `cargo test` determinism passes; Vitest wasm-in-node
  smoke passes; `pnpm --filter app build` succeeds; Playwright loads + screenshots a mounted app.
  **COMMIT**; PROGRESS.md resolved-versions filled. *(T1–T4)*
- **CP1 (Sim core)** — cargo tests cover catchment no-double-count, gravity dest pick, arc-length motion,
  FIFO board/alight w/ capacity, fixed-tick phase order. Determinism green. **COMMIT.** *(T2, T14, T16a/b)*
- **CP2 (WASM boundary)** — `Sim::new(seed,city_json)`/`apply_command_json`/`tick`/SoA + copy-out; node
  smoke asserts a vehicle advances across ticks; wasm instantiation smoke passes. **COMMIT.** *(T8, T9)*
- **CP3 (Basemap)** — MapLibre renders Singapore (hosted style ok) + visible OSM attribution; screenshot
  shows the island. **COMMIT.** *(T5)*
- **CP4 (Static overlays)** — deck.gl (or fallback) renders a test line + stations + catchment over
  geography; screenshot confirms positioning. **COMMIT.** *(T6)*
- **CP5 (Build tools)** — place-station + draw-line via `apply_command_json`, re-emitting authoritative
  entities; screenshot shows a player-built line. **COMMIT.** *(T10, T11, T12)*
- **CP6 (Animation — EARLY VISUAL WIN)** — fixed-timestep accumulator runs the sim; vehicles animate
  smoothly ~60fps; no per-frame layer rebuild (verified); interim e2e asserts a vehicle moved.
  Screenshot + console FPS log. **COMMIT.** *(T15)*
- **CP7 (Ridership loop closed)** — pax spawn from catchment, board/ride/alight; ridership counter + 0–100
  coverage gauge update live in the DOM. **COMMIT.** *(T16a/b, T17 must-have)*
- **CP8 (Slice + e2e)** — headway slider + auto-name + pause/play/speed; committed slice spec asserts
  *load → place → draw → assign → run → vehicle moved AND ridership>0 AND gauge changed*, with a
  screenshot. Final determinism green. **COMMIT.** *(T18)*
- **DETERMINISM RE-GATE:** the `cargo` replay-equality test must be green at **every** checkpoint commit
  from CP0 on. A red determinism test blocks the commit.

---

## 12. Execution protocol (unattended discipline)

- **Branch:** first commit (scaffold + license + .gitignore + ATTRIBUTION + PROGRESS.md) on `main`,
  then work on `slice/singapore-vertical`. **Do not push** unless told. End every commit message with
  the required `Co-Authored-By` trailer.
- **Commit cadence:** commit after every checkpoint that leaves the tree green (builds + all existing
  tests pass). **Never commit a red tree.** Prefix `wip:` only to checkpoint a building-but-partial state.
- **Walking skeleton first:** T1 is not gameplay — prove every integration seam (cargo determinism,
  Vitest wasm-in-node, Playwright load) before depending on it.
- **Pin everything; commit `Cargo.lock` + `pnpm-lock.yaml`.** Version skew is the top silent-failure
  source. Re-verify installed wasm-bindgen/rust/node versions at run start and record them in PROGRESS.md.
- **Command-sourcing seam from commit 1:** every mutation is a serializable `Command` via the single
  `apply_command_json` path; frontend never mutates sim state; keep the in-memory command log.
- **Self-verification:** after each feature checkpoint run the relevant tier (`cargo test`, `pnpm vitest
  run`); for any UI-visible change, drive the page with the **Playwright MCP** — navigate to preview,
  perform the gesture, screenshot, read console for errors. The bar is a concrete DOM assertion
  (ridership>0, vehicle moved) + a clean screenshot — **never "page loaded."**
- **Time-box risky steps, then take the documented fallback:** deck.gl MapboxOverlay 45 min;
  determinism debug 20 min (then bisect by hashing every tick to find the first divergent tick — almost
  always a HashMap iteration or a float); basemap data / pyrosm — skip to synthetic/hosted immediately.
  Never iterate past two failed attempts on the same wall without falling back. Log the fallback + reason
  in PROGRESS.md.
- **Rabbit-hole guard:** before any deep dive ask "is this on the critical path to the 5-step loop?"
  If no → stub it and move on. Basemap cartography, real demand data, GPU picking, interpolation
  smoothness, CI, and all of T7 are non-blocking — degrade gracefully.
- **PROGRESS.md is the flight recorder:** keep the checklist, timestamped log, resolved-versions block,
  and known-gaps/deferred section current at every checkpoint and every fallback. This is how the morning
  human reconstructs the night.
- **Hard rules:** no float-Mercator or wall-clock in the sim (all projection in TS); single-threaded
  main-thread sim on stable Rust — **no Web Workers / SharedArrayBuffer / COOP-COEP / rayon / nightly**
  (perf is microseconds-per-tick; any slowdown is the deck.gl per-frame-rebuild anti-pattern, fix that, not threads).
- **Checkpoint gate = commit gate:** don't advance until acceptance is met and committed. If a checkpoint
  can't be met even after its fallback, record the gap, commit what's green, and proceed to the next
  independent layer rather than blocking the whole run.

---

## 13. Risk register (top risks; mitigation → fallback)

| Risk (L/I) | Mitigation | Fallback |
|---|---|---|
| **wasm-bindgen crate/CLI skew** (H/H) | Pin `=0.2.117`; commit Cargo.lock; node smoke after build (fail checkpoint if instantiation throws). | `cargo install -f wasm-bindgen-cli --version <pinned>`, or let wasm-pack pin its pair. Record resolved versions; move on. |
| **Detached Float32Array view on heap growth** (H/H) | **Don't use zero-copy views** — pre-size fixed-capacity SoA buffers, copy into a reused JS array each frame; assert byteLength stable. | If perf ever needs it, re-acquire the view every frame and check byteLength. Copy path is the default — never a rabbit hole. |
| **Protomaps daily-build extract fails** (H/M) | Basemap **off** the critical path; resolve a valid recent build date (HEAD-walk-back); verify output opens. | Ship on hosted CARTO/MapLibre demo style; swap PMTiles in later. If extract keeps failing, keep hosted style + note as deferred. |
| **pyrosm install eats the night** (H/M) | **Don't install it.** Go straight to the seeded synthetic grid. | Synthetic grid is the primary path; pyrosm is a post-slice nicety. |
| **Determinism leaks** (M/H) | Pure core, integer time/mm, seeded ChaCha8Rng (pinned), Vec/IndexMap only; replay test in the first task; re-gate every commit. | Bisect by per-tick hashing to the first divergent tick (usually HashMap/float). If stuck, quarantine the subsystem behind a flag, keep the green core, log it — never ship a silently-nondeterministic core. |
| **deck.gl + MapLibre overlay won't render** (M/H) | Pin deck submodules identical 9.3.x; overlaid mode only; verify a trivial MapboxOverlay dot before building features (CP gate). | After 45 min: MapLibre-native GeoJSON line/circle layers + a synced `<canvas>` for vehicles. Keeps the loop; loses GPU picking. No PixiJS/regl. |
| **Corner-cutting / dragPan conflict** (M/L) | Interpolate along arc-length; run sim ≥10 Hz; explicit `dragPan.disable()/enable()` around draws. | Raise tick rate to 30 Hz and lerp x/y — imperceptible at metro speeds. |
| **Playwright false-green / flake** (M/H) | Assert concrete facts (vehicle moved AND ridership>0); wait on window flags/testids; fixed viewport + center/zoom + deterministic placement hook; serve the built bundle. | Drive the page via the Playwright MCP interactively + screenshot + console as the verification of record; commit the spec for CI anyway. Never accept a load-only green. |
| **COOP/COEP/threads rabbit hole** (L/H) | Hard rule: single-threaded, no SAB, no threads. | If a CPU bottleneck is perceived, confirm it's the deck.gl rebuild anti-pattern, not a thread need. |
| **deck.gl per-frame-rebuild perf collapse** (M/M) | Mutate persistent typed arrays; `setProps` stable identity; `updateTriggers` only on topology change; verify FPS before declaring CP6 done. | Confirm no layer rebuild; cap visible catchments to selected/hovered. Entity counts are 2–3 orders under deck's ceiling — any slowdown is a bug, not scale. |
| **Git history bloat (pmtiles)** (M/L) | Gitignore `*.pmtiles` + pbf from commit 1; commit only the small demand JSON + scripts. | If the asset must be reproducible for Playwright, run `build_data.sh` as a fixture step; keep it gitignored. |
| **vite-plugin-wasm + TLA friction** (M/M) | `build.target:'esnext'` + `vite-plugin-wasm@^3.6` only; verify wasm-in-node Vitest smoke early. | Import the `--target web` output by relative path / inline-instantiate via generated `init()`. Record what worked. |

---

## 14. Definition of Done

- End-to-end **playable loop in the browser against the production bundle** (`vite preview`): pan/zoom a
  real OSM Singapore map; place ≥2 auto-named stations (catchment circle visible); draw one line; assign
  a trainset + set a headway via one slider (auto-suggested default); press Play; an animated train runs
  the line on the headway; passengers spawn from catchment, board/ride/alight; a live ridership counter
  (>0) and a 0–100 coverage/satisfaction gauge update.
- The deterministic Rust core is **pure** (no clock/thread/HashMap-iteration/float-Mercator) and its
  **replay-equality test passes** (same seed + ordered command log ⇒ identical `state_hash`, twice in one process).
- Every mutation flows through a single `apply_command_json` path with an in-memory command log retained
  (save = seed + log) — the command/event-sourcing seam is real.
- **All three test tiers green:** `cargo test` (sim + determinism), `pnpm vitest run` (TS + wasm-in-node
  smoke), the committed Playwright slice spec (vehicle moved AND ridership>0 AND gauge changed + screenshot).
- Singapore basemap loads with **no API key / no runtime tile-API dependency** (hosted free style is an
  acceptable documented fallback; PMTiles preferred), with visible OSM attribution + `ATTRIBUTION` file.
- Sim consumes **only** a committed deterministic demand grid JSON — never the network at runtime.
- Clean **pnpm + Cargo workspace**, pinned + locked deps (`Cargo.lock` + `pnpm-lock.yaml` committed),
  large assets gitignored, reproducible scripts. **PROGRESS.md** documents resolved versions, every
  fallback taken, and known gaps.
- **Unimplemented-but-real seams:** `CityData` contract (WGS84), `trait Router` (DirectRide now,
  RAPTOR-shaped data), `enum Command`, `trait Demand` — so GTFS, transfers, more lines, more cities,
  and multiplayer are additive, not rewrites.

---

## 15. Explicitly deferred (do NOT build tonight)

Multiplayer netcode/lockstep/rollback (only the command-sourced seam) · GTFS import (only the lng/lat
contract) · the other 8 cities · transfers/interchanges/multi-line routing/RAPTOR K>1 · junctions/
signaling/separate track/branching · multiple trainset types/carriages/manual timetables · economy
(fares/costs/money) · time-of-day demand · Web Workers/SAB/COOP-COEP/rayon/nightly/fixed-point/
deck interleaved/METER_OFFSETS · zero-copy wasm views (use the copy path) · GPU-picking polish/PixiJS/
regl · vector-tile cartography polish · real census-grade demand · heavy auto-assist · follow-camera/
minimap/photo-mode · scripted tutorial · audio/music (at most one chime, cut-first) · CI passing
(scaffold only; must not block the loop) · all of T7 unless M0–M5 are green with time to spare.

---

*Generated by an ultracode research→design→critique workflow. Readiness after baking in the §0
corrections: launch-ready for an unattended overnight run.*
