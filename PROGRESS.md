# transitstory — Build Progress (flight recorder)

Live log of the autonomous overnight build of the Singapore vertical slice (per [PLAN.md](PLAN.md)).
Maintained at every checkpoint and every fallback taken.

## Milestone checklist

- [x] **M0** Walking skeleton — repo + green tests in all 3 tiers (T1–T4, CP0) ✅
- [x] **M1** Singapore map renders & is interactive (T5, T6, CP3/CP4) ✅
- [x] **M2** WASM sim bridge proven (T8, T9, T13, CP2) ✅
- [x] **M3** Build tools — place stations, draw a line, assign a trainset (T10–T12, CP5) ✅
- [x] **M4** Live sim — vehicles run, passengers flow, ridership accrues (T14–T16b, CP6/CP7) ✅
- [x] **M5** Stats readout + full slice verified e2e (T17, T18, CP8) ✅ — **SLICE COMPLETE**

### Task status
| Task | Title | Status |
|------|-------|--------|
| T1 | Scaffold monorepo + PROGRESS + lockfiles | ✅ done |
| T2 | Sim core + determinism replay test | ✅ done (cargo 6/6) |
| T3 | Vite shell + coords/geo.ts + Vitest + Playwright load | ✅ done (vitest 3/3, e2e 1/1) |
| T4 | CI workflow | ✅ done (YAML valid; not run locally) |
| T8 | sim-wasm wrapper (Sim::new/applyCommandJson/tick/SoA) | ✅ done (node smoke) |
| T9 | TS SimBridge + wasm-in-node smoke | ✅ done (vitest 6/6) |
| T13 | Synthetic demand grid + singapore_city.json + loader | ✅ done (cargo 8/8, vitest 9/9) |
| T5 | MapLibre Singapore basemap + geo.ts | ✅ done (CP3, e2e) |
| T6 | deck.gl MapboxOverlay + test layer | ✅ done (CP4) |
| T10 | Place-station tool + catchment | ✅ done (CP5) |
| T11 | Draw-line tool (snap/blueprint/commit) | ✅ done (CP5, e2e) |
| T12 | Assign trainset + headway slider + left line list | ✅ done (CP5, e2e) |
| T14 | Vehicle movement (Rust) | ✅ done (cargo, replay) |
| T15 | Fixed-timestep animation loop | ✅ done (CP6, e2e) |
| T16a | Catchment capture + spawn | ✅ done (no-double-count test) |
| T16b | Board/ride/alight + stats + coverage | ✅ done (ridership + monotonic coverage) |
| T17 | Stats bar + coverage gauge + waiting dots | ✅ done (CP7) |
| T18 | End-to-end slice Playwright spec | ✅ done (CP8, vs preview bundle) |
| T7 | Self-host PMTiles (DEFERRED unless time) | deferred (hosted CARTO style ships) |

## Resolved tool versions (verified at run start, 2026-06-04)

| Tool | Required (PLAN §0/§2) | Installed | OK |
|------|----------------------|-----------|----|
| rustc / cargo | 1.94.0 | 1.94.0 | ✅ |
| wasm32-unknown-unknown target | present | present | ✅ |
| wasm-bindgen CLI | =0.2.117 | 0.2.117 | ✅ (crate pinned to match) |
| wasm-pack | any recent | 0.14.0 | ✅ |
| node | 24.x | 24.14.1 | ✅ |
| pnpm | 10.x | 10.33.2 | ✅ |
| npm | — | 11.11.0 | ✅ |
| git | — | 2.43.0 | ✅ |
| network (crates.io/npm/github) | reachable | 200 | ✅ |

Dependency pins in use: `rand =0.10.1`, `rand_chacha =0.10.0`, `wasm-bindgen =0.2.117`,
`@deck.gl/{core,layers,mapbox}` identical `9.3.3`, `maplibre-gl ~5.24.0`, `vite-plugin-wasm ^3.6`
(no top-level-await plugin; `build.target:'esnext'`). Command wire format = JSON; postcard = save artifact only.

## Running log

- **2026-06-04** — Run start. Verified toolchain (table above): all green, wasm-bindgen CLI already at the
  pinned 0.2.117, network reaches all registries. Began T1 scaffold.
- **2026-06-04** — T1: scaffolded monorepo. **Found the git repo root was `/home/g` (the home dir, an empty
  unrelated repo) — gave the project its own isolated repo via `git init` to avoid ever committing home-dir
  secrets.** First commit on `main`, then branch `slice/singapore-vertical`. All 25 project files, 0 leakage.
- **2026-06-04** — T2: deterministic sim core (`World`/`Command`/`Event`/`state_hash`), externally-tagged
  commands (round-trip JSON + postcard), integer mm/ms, headway/count clamps. `cargo test -p sim`: 6/6 incl.
  replay-equality. Committed.
- **2026-06-04** — T3: app shell + `coords/geo.ts` boundary + styles; Vitest geo round-trip (3/3); Playwright
  load spec (1/1, Chromium launched without `--with-deps` — system libs present); screenshot
  `docs/progress/cp0-app-shell.png`. T4: CI workflow (`.github/workflows/ci.yml`, YAML valid; not run locally
  — CI not gated overnight per PLAN §15).
- **2026-06-04** — **CP0 reached: walking skeleton green in all three tiers** (cargo 6/6, vitest 3/3, e2e 1/1,
  app build ok). Committing.
- **2026-06-04** — T8: `Sim` facade (applyCommandJson/tick/stateHash/vehicle copy-out/stats/views).
  **Fallback taken:** dropped `rand` default features — getrandom 0.3 has no wasm32-unknown-unknown backend;
  the sim is seeded-only so OsRng was never needed (also reinforces determinism). Node instantiation smoke green.
- **2026-06-04** — T9: TS SimBridge + types/codec/log + wasm-in-node Vitest smoke. **Decision:** switched
  wasm-pack `--target web` → `--target bundler` so vite-plugin-wasm auto-instantiates the module in both the
  Vite browser build and Vitest(node) via top-level await — removes the manual `init()`/fetch friction (the
  documented T9 risk). Vitest 6/6 (geo 3 + bridge 3, incl. determinism across the wire). Bridge proven; the
  "vehicle advances across ticks" assertion lands with T14/T15.
- **2026-06-04 → 06-05 (overnight)** — T13: synthetic demand grid (3050 cells, no pyrosm) + city manifest +
  loader (cargo 8/8, vitest 9/9). **M2 done.** T5: CARTO Positron basemap + OSM attribution (CP3, screenshot).
  T6: deck.gl MapboxOverlay overlaid + marker (CP4, screenshot). **M1 done.** T10/T11: place-station +
  interactive draw-line (snap/blueprint/commit, dragPan toggle) + overlay render from authoritative views;
  e2e places 3 stations + draws line [0,1,2] (CP5 screenshot cp5-stations-and-line.png). Now T12.
- **2026-06-05 (overnight)** — T12: trainset + headway slider + line list panels (M3 done, CP5). T14:
  deterministic vehicle motion (arc-length + trapezoidal + dwell + reverse), vehicle integer state folded
  into state_hash so the replay gate now covers movement. **Fallback:** raw-node wasm smoke can't load the
  bundler-target module (no manual init) — verify vehicles via the Vitest bridge instead. T15: fixed-timestep
  accumulator loop + 60fps interpolation + speed controls (CP6, cp6-vehicles-running.png).
- **2026-06-05 (overnight)** — T16a/b: catchment capture (normalized, no-double-count test), seeded spawn,
  gravity destination pick (RngExt — RngCore not exported with rand default-features off), board/ride/alight
  with capacity, stats + monotonic coverage. **Fallback:** dropped rand default features earlier already
  covers the RNG path. T17: stats HUD (ridership + coverage gauge + waiting halos) on a 3 Hz throttle (CP7).
  T18: flagship slice e2e vs the production preview bundle (CP8, cp8-slice-running.png).
- **2026-06-05 — SLICE COMPLETE. All three tiers green together:** cargo 14, vitest 10, playwright 7.
  The Singapore vertical slice is playable end-to-end (place → draw → assign → run → ridership + coverage).

## Definition of Done — verification (PLAN §14)

- ✅ **End-to-end playable loop vs the production bundle** — `e2e/slice.spec.ts` against `vite preview`:
  place stations → draw a line → assign trainset + auto headway → Run → animated trains, passengers
  board/ride/alight, live ridership + 0–100 coverage gauge. Screenshots in `docs/progress/`.
- ✅ **Deterministic pure core** — `crates/sim` has no clock/thread/HashMap-iteration/float-Mercator; the
  replay-equality test (incl. vehicle + ridership state) is green and re-gated at every commit.
- ✅ **Command-sourcing** — every mutation flows through `applyCommandJson`; in-memory log retained (save =
  seed + log). Frontend never mutates sim state.
- ✅ **All three tiers green** — `cargo test --workspace` (14), `pnpm --filter app run test` (10, incl.
  wasm-in-node smoke), committed Playwright e2e (7) asserting concrete facts + screenshots.
- ✅ **Basemap no API key / no runtime tile-API dep beyond the hosted style** — CARTO Positron (OSM-derived),
  visible OSM attribution, `ATTRIBUTION` committed. (Self-hosted PMTiles is the deferred T7 upgrade.)
- ✅ **Sim consumes only a committed deterministic demand grid** (synthetic; pyrosm deferred).
- ✅ **Clean pnpm + Cargo workspace**, pinned + locked deps (`Cargo.lock` + `pnpm-lock.yaml` committed), large
  assets gitignored, reproducible `scripts/build_demand.py`. PROGRESS documents versions + fallbacks.
- ✅ **Unimplemented-but-real seams** — `CityData` (WGS84), routing (DirectRide now, RAPTOR-shaped data via the
  dispatch/demand structure), `Command` enum, demand grid — so GTFS / transfers / more lines / multiplayer
  are additive. (Note: a formal `trait Router`/`trait Demand` abstraction is implied by the module layout but
  not yet extracted into named traits — a clean next step.)

## Post-slice features (F1–F6, 2026-06-05)

Built on top of the slice (cargo 20, vitest 10, playwright 10 — all green):

- **F1 Curved track** — lines follow a centripetal Catmull-Rom curve through stops (soft min
  radius, no cusps); dense polyline drives render + arc-length motion, `stop_arclen_mm` marks stations.
- **F2 Time-of-day** — `tod.rs` 24h clock (opens 06:00), twin-peak rush demand multiplier, AM(home→work)/
  PM(work→home) directionality; clock + period in the HUD. Deterministic (pure fn of clock).
- **F3 Transfers** — `routing.rs` BFS over the line graph → minimum-transfer multi-leg routes; passengers
  re-queue at interchanges. `route_cache` (lookup-only FxHashMap) keeps it fast at scale.
- **F4 Existing networks** — `network.ts` + `Game.applyNetwork` pre-seed real lines via the Command path
  (shared station indices = interchanges).
- **F5 Menu + cities** — start menu (city + sandbox/real-network), `cities.ts` registry, per-city manifests +
  demand grids, settable per-session coordinate origin; deep-link `?city=<id>&network=0|1`.
- **F6 OSM data source** — `scripts/build_networks.py` pulls **real networks from OpenStreetMap (Overpass)**,
  re-runnable to update: Singapore 12 lines/177 stations, Tokyo 32 lines/440 stations, Calgary 2 lines/42
  stations. (Demand model from F1-era T16 confirmed in: gravity catchment + spawn + board/ride/alight.)

Demand-data note: the demand grid is still synthetic (gravity bumps); OSM/GTFS-derived demand is the next
data upgrade. The network geometry is now real OSM.

## Rail vs the built environment (2026-06-05)

Researched how NIMBY Rails handles it (answer: it doesn't — buildings are absent from its data;
it prototyped eminent-domain and CUT it because OSM building coverage is too uneven). Adopted the
recommended **middle path**: never hard-block on buildings; make the built environment a SOFT cost.
cargo 24, vitest 10, playwright 12 — all green.

- **G0 Curve radius + speed caps** — per-vertex curve radius on the Catmull-Rom polyline; trains slow
  through tight curves (v=√(lat_accel·R)); `min_radius` flagged in the editor.
- **G1 Buildability** — `scripts/build_buildability.py` rasterizes OSM (water / road-ROW / rail-ROW /
  built / park) into a coarse 120 m grid per city (committed `<id>_buildability.json`). Per-span build
  mode (Surface/Elevated/Tunnel via `SetSegmentMode`); per-line **disruption** = Σ weight(class)×metres×
  mode-factor; **one hard gate = water at surface** (flagged). Elevated/Tunnel cut the penalty + clear water.
- **G2 UI** — live hazard dots (amber built/park, red water) along the blueprint as you draw; water-crossing
  lines render red; EditorPanel Track-mode toggle + water warning + tight-curve flag; HUD **Build-impact** meter.
- **Tier-2** — street-running: Surface track over built-up land caps speed (~43 km/h). The coherent play
  emerges naturally — surface the open suburbs (0 impact, fast), grade-separate the dense core (low impact, fast).

## Economy, sea, modes & layers (2026-06-05)

Batch closing the buildability seams and adding the multi-mode play. cargo 28, vitest 10,
playwright 14 — all green (one full-suite e2e is GL-context-flaky at the tail of the single-worker
run; green standalone).

- **#1 Sea gap closed** — `build_buildability.py` now floods OSM **coastline** into open SEA (seed
  right-of-coastline, BFS over Open cells → WATER), so the water hard-gate covers the strait/harbour,
  not just tagged inland water. The hazard dots + red surface-water lines now fire over the sea.
- **#2 Economy (Tier 3)** — `Line.capital_cost` = per-km by mode + land-taking + trains; tunnelling
  costs far more than surface; fares = ridership×FARE accrue to `balance` = START_BUDGET + fares − capital.
  Optional `economy_enabled` toggle (HUD money box dims when off). Reuses the disruption basis from G1.
- **6 cities** — Istanbul, New York (Manhattan), Dublin added to `cities.ts` + `city_demand_config.json`
  + per-city committed manifests/demand/buildability (now Singapore, Tokyo, Calgary, Istanbul, NYC, Dublin).
- **#5 OSM data inventory** — `docs/osm-data.md`: what's extractable from OSM for modes/demand/coastline/heatmap.
- **#3 Transport modes** — `trainset::tmode` (RAIL/BUS/FERRY/AIR) + `spec_for_mode` presets; `Line.mode`
  carried through the Command path (`CreateLine.mode`). Per-mode placement gate in `recompute_line_buildability`:
  rail (built/water/park weighted, water-flagged), bus (rides roads, cheap), ferry (water IS its road — no
  flag, land penalised), air (flies over anything). Per-km capital scales by mode.
- **#3 Chorded mode UI** — bottom bar with four big mode buttons (1 Rail / 2 Bus / 3 Ferry / 4 Plane);
  selecting one opens its build-controls popover above (Stations / Draw line / Select + a mode hint) and
  arms construction in that mode. Keyboard 1–4 chord the modes. Line list + editor show a mode badge;
  the Surface/Elevated/Tunnel Track control is rail-only.
- **#4 Demand map layer** — toggleable travel-demand heat overlay (deck.gl ScatterplotLayer, blue→red by
  grid weight) sourced from the committed demand cells; sits under the network.
- **#8 Settings panel** — ⚙ panel toggles each transport mode on/off (greys out the bar) and the economy.
- **#6/#7 (prior in this batch)** — real line names from OSM `name`/`ref`; loop lines; importer filters
  construction/<3-stop stubs (fixed the Jurong stub + LRT branch artefacts). Ferries now pulled too (2-stop OK).

## React UI migration (2026-06-05)

Reversed the locked "vanilla TS — no framework" decision (AGENTS.md updated to record it): the UI chrome
is now **React 19** (`react`/`react-dom` 19.2.7, `@vitejs/plugin-react` 6.0.2 pinned; `jsx: react-jsx`). The
sim/map/deck.gl core is untouched — React renders only the floating chrome inside `#ui`; the map, the deck
overlay, and the `GameLoop` rAF loop stay imperative and entirely outside React (two-clocks rule held).

- **State seam = `ui/react/GameContext.tsx`** with two slices on two cadences: `stats` pushed from the existing
  ~3 Hz interval (the chosen "Context + interval setState"), `ui` (selection/mode/tool/transport/…) updated on
  `game.onChange` with shallow-compare so selection stays sub-100 ms without per-frame renders. Hooks:
  `useGame`/`useLoop`/`useStats`/`useGameUI`.
- **`ui/react/App.tsx`** = the menu→boot→playing phase machine; `boot()` builds the imperative world exactly as
  the old `main.ts` did (deep-link `?city=` preserved). `main.tsx` mounts it via `createRoot` (no StrictMode —
  it would double-boot the imperative map/loop).
- **Components** ported 1:1 (testids + behaviour preserved): `StatsBar`, `Panels` (LineList + Editor), `Toolbar`
  (chorded bar + popover), `Settings`, `Menu`. Authored in parallel by a 4-agent workflow against a frozen
  hook/shared contract; integrated + drift-fixed by hand. Headway slider keeps the drag-end-commit rule via
  native `change`/`input` (React's synthetic `onChange` would fire per drag-tick); data inputs are
  uncontrolled+keyed so they resync to the committed snapshot.
- Deleted the vanilla `ui/{menu,panels,statsbar,toolbar,settings}.ts` + `main.ts`. Verified: tsc clean, vite
  build green, vitest 10, **all 14 e2e green** (incl. the menu/modes/slice/assign/build-tools flows) + visual
  parity screenshots (`docs/progress/react-menu.png`, `react-ingame.png`).

## Time-dependent routing — RAPTOR (2026-06-05)

Replaced the min-transfer `BfsRouter` as the **default** with `RaptorRouter` (frequency-based
RAPTOR), the long-planned drop-in behind `trait Router`. BFS counted legs; RAPTOR minimises
**expected travel time** = per-leg wait (~headway/2) + in-vehicle time (arc-length ÷ a
mode-effective speed, ~0.75·v_max, + per-stop dwell), bounded to `max_legs` rounds (= K). Riders
now route over fast/frequent corridors instead of merely fewest-transfer ones. cargo 49 (+5),
vitest 20, tsc clean — determinism gate green with RAPTOR in the loop.

- **`routing/raptor.rs`** — pure integer / deterministic (i64 ms throughout, index-ordered
  iteration, no HashMap iteration, no floats in the cost). RAPTOR route-scan per round: mark lines
  serving any improved stop, scan each directed route once carrying the earliest boarding, relax
  downstream arrivals; a round-start arrival snapshot guarantees each round adds ≤1 leg (so legs ≤
  `max_legs`). Out-and-back lines yield forward+backward routes; loops a doubled cyclic route.
- **Drop-in, no core rewrite** — `World::apply` signature, the demand call shape, and the
  `Vec<Leg>{line,board,alight}` output are all unchanged, so `Pax`/`board_alight`/`route_cache`
  are untouched. `BfsRouter` stays as the simple reference + comparison baseline.
- **Tests** (`tests/raptor.rs`, written RED first) pin what BFS *cannot* do: prefer the faster of
  two equal-leg direct lines; accept a transfer when frequency+speed make it faster than a slow
  direct; honour the `max_legs` bound; reach everything BFS can; and stay bit-identical across a
  6000-tick replay. Browser-corroborated on the live Singapore network (ridership 0→520, trips
  complete avg ~2.3 min, coverage 94, 0 console errors, no geometry artifacts).
- **Still deferred** behind the same seam: inter-station **footpaths** (transfers remain at shared
  station ids only) and a real **departure timetable** (the model is frequency-based by design —
  vehicles are headway-dispatched with emergent positions, so there is no timetable to read).
- **Accessibility-weighted demand** (the closing of the loop) — `Router::reachable` exposes RAPTOR's
  one-to-all travel-time labels (near-free; it computes the whole vector anyway). `demand::pick_dest`
  now weights a trip's destination by **how fast transit reaches it** (wait + ride) instead of
  crow-flies metres, so a better network *induces* demand toward the places it connects well. Cached
  per origin (`access_cache`, cleared with `route_cache` on network change); `BfsRouter` opts out
  (empty vec → geometric fallback). `tests/access.rs`: ordered one-to-all labels, demand skews to the
  better-connected of two equal-job destinations, and the whole path stays deterministic. cargo 53.

## Network dashboard — surfacing the service telemetry (2026-06-05)

The sim produces rich service-quality telemetry (journey/wait time, full-train denials, renege,
demand multiplier, per-line/-mode ridership) that mostly lived in a single hover-tooltip. Added a
**collapsible "📊 Network" dashboard** (`ui/react/ServiceReport.tsx`) in the empty bottom-left
corner — the abstract-state-in-panels half of the IA, so the top StatsBar stays "one number + one
gauge". Default-open so the info is a visible channel, not a buried hover. Pure chrome: reads only
the ~3 Hz `stats` slice, issues no Commands, new testids only (existing contract untouched).

- Surfaces: **Avg wait / Avg trip** (were tooltip-only), **Demand served** gauge + the time-of-day
  **rush multiplier**, the **pressure** trio (waiting / passed-by-full-trains / gave-up, colour-coded),
  and a **Riders-by-mode** bar chart aggregated from `per_line` (no new sim field). A footer line
  names the new accessibility loop ("trips favour destinations your network reaches fastest").
- Browser-corroborated on the live Singapore network: 0 console errors, live values (avg trip 2.2 min,
  served 94, 1022 waiting), no layout collisions (Toolbar is bottom-centre, this is bottom-left), and
  the e2e suite is unaffected (specs are camera-independent — hook + testids, no raw canvas clicks).

## Inspector: trains + lines become hoverable (2026-06-05)

Generalised the station-only hover tooltip into a unified **inspector** over the same deck
`getTooltip` seam — now stations, **trains**, and **lines** all raise a readout, dispatched by
which pickable layer was hit. Z-order makes the hierarchy natural (a station on top, else a train
between stops, else the line's own track). Pointer/snapping is untouched — it uses `nearestStation`,
not deck picking, so making layers pickable only adds tooltip hits.

- **Trains** (new — were not inspectable at all): hover a moving vehicle → its line (swatch + mode
  icon) + live **load factor** (○/◐/● healthy/busy/crush, the same pip as the roster) + `N/cap aboard`.
  Needs per-vehicle load: added one sanctioned copy-out buffer `vehicle_loads` = interleaved
  `[onboard, capacity]` (capacity from the core's `spec_for_mode`, single source — the UI never
  re-derives it). `render_buf.rs` → `sim-wasm` `vehicleLoads()` → `SimBridge` → `game.vehicleTip(index)`.
- **Lines** (new — previously only via selecting → Editor): hover the track → name swatch, mode,
  ridership, load, stops, trains, headway. Built from the same `Stats` snapshot the panels read
  (a `perLineById` index, mirroring `perStationById`), so the readout agrees with the roster.
- Tip HTML builders (`vehicleTipHtml`/`lineTipHtml`) live in `shared.ts` beside `stationTipHtml`,
  reusing `loadPip`/`hex`/`esc`. New `__ot_test.lineTip`/`vehicleTip` hooks + `global.d.ts` types.
- Verified: tsc clean, vitest **21** (added a wasm-in-node assertion that `vehicleLoads` marshals
  `[onboard,cap]` with cap=200 and onboard≤cap), sim 53. `lineTip` browser-corroborated on the live
  Singapore network (real names, load, headway). NOTE: a dev server started before the wasm rebuild
  serves a stale `.wasm` (lacks `vehicleLoads`) until restarted — the production build / a dev restart
  is correct (the committed source + regenerated pkg include it).

## Demand & traffic visibility — the sim made legible (2026-06-05)

Audited what the core *computes* vs what the player *sees* (4 parallel readers over sim/buffers/
deck/UI) and closed the gap across five tracks. Guiding rule: hue stays line identity (load rides
brightness/outline, never hue); flows are on-demand (selected station only → no mud); everything
rides the existing copy-out / ~3 Hz / per-frame-vehicle paths — no new sim tick, no per-frame
rebuild. New player-facing **readouts only** — zero new Commands, zero behaviour change.

- **Sim core readouts** (`StationStat` += `demandOrigin/demandDest` (captured gravity weight),
  `serving` (operational line count; 0 = orphaned), `denied/abandoned` bucketed PER STATION). New
  `World.denied_at/abandoned_at` counters incremented at the existing renege + full-train-pass-by
  sites, **folded into `state_hash`** → `pressure_buckets.rs` asserts Σ per-station == the global
  totals AND replays bit-for-bit. Determinism gate green.
- **Traffic** — trains were uniform dots. Now: a white triangle rotated to the sim heading (the
  `vehicleAngles` buffer was exported and thrown away; an `IconLayer` consumes it — white so it
  reads on same-coloured track), and a crowding outline white→amber→red + radius ∝ load (the
  `loadPip`/waiting-ring language) via the existing `vehicleLoads`. Interp consolidated into
  `Game.vehicleDotsAt(alpha)` (one source for the loop + on-refresh recompose). Arrow angle
  screenshot-verified (points along travel).
- **Peeps** — station dot radius grows with boardings (a usage heatmap); orphaned stations render
  muted and brighten when served; the waiting ring gained a 3rd band (faint → amber BUSY → red
  STARVED); the inspect tooltip gained a per-platform "N passed by · N gave up" loss line.
- **Demand** — the selected station's catchment fill alpha scales with its captured demand
  (self-calibrated vs the busiest station, so city-scale-independent) + a "serves ~N demand" /
  "⚠ no service yet" tooltip line.
- **Sim legibility** — StatsBar gained a compact network-load pip (`avgLoadFactor` + `vehicleCount`
  in tooltip, run-only so never dead chrome); the roster gained a "N stops · N trains · every N min"
  subline. Global `buildDifficulty` **deliberately NOT** surfaced — the StatsBar's locked decision
  keeps build impact a build-time per-line concern (EditorPanel `line-impact`); respected, not relitigated.
- **Flows** — OD "desire lines": selecting a station draws curved `ArcLayer` arcs to where its riders
  are drawn (gravity attractiveness × accessibility). `demand::od_weights` factors the SAME weights
  `pick_dest` samples (minus the RNG baseline); `World::station_od` sorts/top-10/normalizes →
  `OdLink`; facade `stationOd`. **Pure read** — solves accessibility fresh, mutates nothing (`od.rs`
  asserts `state_hash` unchanged across calls; orphaned origins → `[]`). On-selection only.
- Verified: `cargo test --workspace` green (determinism + new `pressure_buckets.rs`/`od.rs`), tsc
  clean, vitest **21**, e2e render-path specs (slice/network/modes/assign/deck) green; every track
  screenshot-corroborated on the live Singapore network, zero console errors. (`vehicle-move.spec`
  checks `vehicleCount()` with no tick guarantee — passes on slower CI/dev, races the first tick on a
  fast local preview; confirmed dispatch is intact: 0 immediate → 3 after 120 ms. Pre-existing
  fragility, not touched.) NOTE the same stale-`.wasm`-on-an-unrestarted-dev-server caveat applies.
- **Deferred (clean follow-on):** accessibility **isochrone** shading — reuses the very
  `Router::reachable` port `od_weights` now calls, so it slots in behind the same seam.

## Accessibility isochrone + inter-station footpaths (2026-06-05)

Continued from the visibility pass. Two more behind-the-seam extensions:

- **Reach isochrone** — the deferred follow-on to the OD lines. `World::station_access(origin)`
  returns every reachable station + transit travel time via the same `Router::reachable` port;
  an opt-in "🕐 Reach" toggle shades them green→amber→red, mutually exclusive with the OD arcs
  (toggling swaps the lens). Pure read (`od.rs` asserts `state_hash` unchanged + monotone).
- **Footpaths** — the routing extension routing/mod.rs explicitly named. Lines that share no stop
  but sit within `FOOTPATH_MM` (~400 m) interchange on FOOT. `World.footpaths` (per-station walk
  edges + integer walk time, rebuilt with the catchment) feeds a new `footpaths` input on the
  Router trait; RAPTOR relaxes them between rounds (transit-reached stations only, so no
  origin/dest walk), legs stay ride-only with a walk GAP (`walk_src` reconstruction), and
  `board_alight` re-queues the transferer at the next leg's board with a walk delay (skipping
  "still-walking" riders). `footpaths.rs` proves a 0→3 trip completes ONLY via the walk and
  replays bit-for-bit; determinism gate + all existing routing tests stay green. The Reach/OD
  overlays reflect walk-reachable destinations for free. Integer ms throughout; BfsRouter stays
  transit-only (the baseline).

## Buses become the road mode (2026-06-05)

The bake already classified `class::ROAD` (motorway/trunk/primary; Singapore 3400 cells,
Tokyo 3748) but NOTHING used it — buses were just cheap rail on straight track. Now buses
are the road-bound mode, all sim-side + deterministic, no new data (chosen A+B+C+E; D/BRT skipped):

- **A — road-aware speed** (`vehicle.rs`): a bus runs full spec speed on a `ROAD` cell, crawls
  (`OFF_ROAD_BUS_MM_S` ~25 km/h) off-road. Same raster-lookup as the rail street cap, mode-gated.
- **B — free roads** (`world.rs line_cost_metrics`): an on-road bus segment lays NO track capital
  (rides the existing road); off-road it builds a busway (3M/km).
- **C — road-following geometry** (`roadnav.rs` + `rebuild_with_span_points`): a bus's inter-stop
  span is routed by an integer **grid A\*** over the `ROAD` raster (cheap on road, dear off), fed
  to the Catmull-Rom smoother as pass-through bends. The search box scales with the stop gap so a
  road can detour off-straight; over-long spans exceed the cell budget → straight (graceful).
  Composes with A+B for free (the road-followed polyline rides ROAD cells → cheap + fast).
- **E — congestion** (`tod::congestion_pct`): an INTEGER step over the in-game hour scales on-road
  bus speed down at the AM/PM peaks — hash-safe (never the f64 `demand_multiplier`).
- Verified: `buses.rs` (4 tests) proves on-road is cheaper+faster, peak slows it, a bus detours a
  U-shaped road, all replay bit-for-bit; full workspace + determinism gate green. Browser-corroborated
  on live Singapore — a bus drew **161 polyline vertices bending ~900 m to hug roads** (rail: 11
  verts, straight) at **10M vs 55M** cost, zero console errors.
- Deferred: **road-graph bake** (true street centrelines vs the 120 m raster — finer follow), **D/BRT**
  (dedicated-busway tier via the build-mode toggle), and a **🛣 Roads overlay** (show the sim's ROAD
  corridors so buses build where they're cheap — the CARTO basemap is a proxy today). Frontend overlay
  left out to avoid colliding with in-flight frontend edits.

## Ferries become the water mode (2026-06-05)

The water twin of road-bound buses, reusing the same machinery (the bus's corridor A* was
generalized, not duplicated). All sim-side + deterministic, no new data:

- **A — water-aware speed** (`vehicle.rs`): a ferry runs full spec speed on a `class::WATER`
  cell and barely moves (`OFF_WATER_FERRY_MM_S`) if forced onto land.
- **B — free water** (`world.rs line_cost_metrics`): an open-water ferry leg lays NO capital
  (just terminals); off-water it pays 5M/km.
- **C — water-following geometry**: `roadnav::road_route` → `class_route(prefer)` — the same
  integer grid A* now follows ANY preferred class. `rebuild_line_geometry` routes BUS spans over
  ROAD and FERRY spans over WATER, so a ferry navigates channels + around islands instead of
  cutting straight over land.
- No congestion (water has no traffic), no overlay (water's obvious on the basemap).
- Verified: `ferries.rs` (3 tests) — on-water cheaper+faster, follows a U-shaped channel, replays
  bit-for-bit; determinism gate green. Browser-corroborated on Singapore — a ferry drew **21
  polyline vertices bending ~400 m around the islands at 0M** cost (rail: 11 verts, straight, 44M).

## Air game — the globe + an aircraft roster (2026-06-05)

The globe is a SECOND `World` of the SAME engine: cities are `Station`s, air routes are `Line`s of
`mode=AIR`, demand is one cell per metro (origin/dest weight = population). Zero core changes beyond
the globe-scale AIR vehicle preset.

- **Globe content** (`scripts/build_globe.py`, pure data): 29 world hubs + 12 flagship air routes.
  Re-runnable + deterministic; one demand cell per metro; stations carry lng/lat only (you don't
  drill into air destinations).
- **Air builds no right-of-way** (`world.rs line_cost_metrics`): AIR per-km cost = 0 — you buy
  aircraft (capital) and burn fuel (opex), not track. The old metro-scale 1M/km was astronomical at
  globe distances (cities thousands of km apart), so any air route was unaffordable.
- **Aircraft roster** (`trainset.rs AIR_ROSTER` + `spec_for(mode, spec_id)`, `Line::vehicle_spec`):
  the `AssignTrainset{spec}` seam made real. Four aircraft — Narrowbody (the locked **index-0
  default**, byte-identical to the historical preset so the determinism hash never moves), Regional,
  Widebody, Jumbo — a **non-dominated** capacity-vs-turnaround ladder (capacity 88<250<410<525,
  dwell 45s<60s<90s<120s, so a bigger jet fills more per departure but sits longer, widening
  effective headway). The one in-sim lever that bites today is dwell (the economy/price dimension
  is the blocked follow-up). Resolved at all four clean reader sites (`pax`/`vehicle`/`render_buf`/
  `raptor`) via `Line::vehicle_spec()`; `spec_id == 0` is always the mode default for every mode, so
  every existing save replays bit-for-bit.
- Verified: `aircraft.rs` (7 tests) — index-0 lock, spec-0==mode-default for all modes, non-dominated
  ladder, OOB clamp, non-air modes ignore spec, a non-default aircraft (widebody) replays bit-for-bit,
  assigned aircraft drives vehicle capacity. Full sim suite + determinism gate green.
- **Deferred (blocked on contested files):** the player-facing **aircraft picker** (a roster mirror
  in `shared.ts` + a selector in the Editor `Panels.tsx` + a `spec` arg on `game.assignTrainset` in
  `game.ts`) and the load-factor display calc at `world.rs:465` (still on `spec_for_mode` — correct
  for the default aircraft, needs `vehicle_spec()` once a non-default can be chosen). These live in
  files under active rewrite (`game.ts`, `shared.ts`, `world.rs`); ready-to-apply, held to avoid
  clobbering WIP. **Air economy depth (per-route P&L, fares, AI competitors)** remains a
  Command/trait/CityData seam, blocked on the same core rewrite — not half-built.

## Audit-driven legibility + build-UX pass (2026-06-10)

A full game audit (rail build UX / visibility / core loop, plus a live play-through) surfaced
three load-bearing design bugs and a tail of feedback gaps; all fixed in one pass, all three test
tiers green (cargo 27 suites / vitest 21 / playwright 15).

- **Coverage gauge re-denominated** (`world.rs coverage_score`): was `served / CAPTURED demand`,
  which read **91/100 seconds after the first 2-station line** (self-referential denominator) and
  *dropped* when you placed a not-yet-served station. Now `sqrt(served_quality / WHOLE-CITY origin
  demand)` — a true progression dial: starts ~0, one good first line ≈ 7, **measured anchors**:
  the full real Singapore MRT ≈ **41**, the globe flagship air board ≈ **54** (measured in-browser
  via the hooks). Monotonic by construction (fixed denominator + 0.5 quality floor + monotone
  sqrt); property tests unchanged-green. Scenario targets retuned to the anchors (Sprint 55→25,
  Metropolis 80→45 "beat the real MRT", Global Airline stays 50); StatsBar/dashboard copy says
  "of the whole city"; the low band renders neutral, not failure-red (a fresh map isn't failing).
- **Time scale made coherent** (`tod.rs HOUR_MS` 60k→120k, day = 48 sim-min): a rush period
  (~3 in-game hours ≈ 6 sim-min) now spans multiple default headways, so ToD demand is tunable
  rather than a flicker between two trains. **Default patience 30→10 sim-min** (`city.rs`): renege
  ("gave up waiting") actually fires — it was the designed difficulty source and effectively never
  triggered (verified live: abandoned stayed 0 over a whole session pre-fix; fires in minutes now).
  `agent_population_target` scales with day length (trips/sim-min constant); congestion test
  re-derived from HOUR_MS instead of pinned ms.
- **Build legality moved into the core** (`dispatch.rs`): a land line with surface track over open
  water is **parked** — no vehicles, no serving entry, no coverage — until elevated/tunnelled.
  Was UI-only (`draftInvalid`), so the hooks/save/replay path accepted a rail line drawn across
  the open sea that ran normally (reproduced live in the audit). Loader sequencing (AddStop →
  tunnel) still works — `legalizeWaterCrossings` now also *un-parks* loaded networks; new
  `buildability.rs` test covers park → legalize → dispatch + replay determinism. Editor warning
  copy says "parked" explicitly. The canonical slice e2e drew exactly such an illegal line
  (clipped Marina Bay) — rerouted through real home/job clusters (Tiong Bahru/Holland ↔ CBD/
  Orchard), and its vehicle-moved predicate de-flaked (sampled positions before dispatch → NaN).
- **Build UX** (audit findings, all verified live): station tool is **sticky** (the onboarding
  literally says "place 2 stations" — the tool used to disarm after one, eating the second click);
  **pre-commit snap ring** (AGENTS rule): the station the next click would chain (selection blue)
  or bulldoze (red) is ringed before the click; **affordability pre-flight** — an unaffordable
  draft shows "$XM short" in the pill, disables ✓ Place, and `commitDraft` gates instead of
  committing; **all-or-nothing commit** — any mid-sequence AddStop rejection rolls the line back
  (RemoveLine on the same log), so the committed network never silently differs from the blueprint.
- **Visibility**: inspector tooltips are now **game-owned DOM refreshed on the 3 Hz stats slice**
  (deck's getTooltip only re-runs on pointer moves — a watched station froze at hover-time values;
  verified live: counts now tick in place); **starved-only waiting halos survive LOD** (new
  `waiting-overview` layer ≥ STARVED_WAITING at overview zoom — exactly one waiting layer per
  frame); **line satisfaction gains a live queue term** (mean waiting at the line's stops via one
  `game.lineQueue` join) so "100% happy" can't coexist with visibly piling platforms; dashboard
  clock formats simHour (was a raw float "21.4458…:00"); **bulldoze finally has an echo** (red
  ripple + "Demolished <name> — $XM written off" toast; it was the one Command with none);
  palette slots 2/4 swapped off the semantic alert hues (healthy-green/busy-amber → Tol teal/wine,
  ferry chip synced); attribution de-duplicated (the CARTO style self-attributes); top-left chrome
  (title · Undo · Stats) is a flex row instead of overlapping fixed offsets.
- **Audit verdicts otherwise:** engineering conventions all clean (determinism bans, membrane,
  command-sourcing, geo boundary, render hot path, two clocks, pins, attribution); scope growth
  judged seam-respecting (nothing half-built). Remaining known gaps: economy lacks an on-map
  pressure channel beyond the balance + new P&L rows; objectives loss state is dismissible without
  trace; no extend/insert/redo line editing (correctly deferred — needs new Command seams).

## The "one more day" pass — growth, beats, and the score chase (2026-06-10)

Follow-up to the audit: the game had a strong minute-loop but no session hook — days were
statistically interchangeable, so there was no reason to play one more of them. This pass makes
the in-game day a *turn*. All tiers green (cargo 28 suites incl. new growth tests / vitest 21 /
playwright 15).

- **Transit-oriented demand growth** (`demand::grow`, the engine): once per in-game day
  (clock-derived, deterministic, pure f(clock, service)), cells within a catchment of a SERVED
  station grow at the city's `growth_bp_per_day` (CityData knob, serde-default 250 = +2.5%/day;
  `CityData::default()` = 0 so native tests opt in), the rest at a third (ambient sprawl — stop
  extending and the city outgrows you). Capped at 2× the strongest initial cell (dataset-agnostic:
  city grids and the metro-population globe both get headroom). Multiplicative — empty cells stay
  empty (growth densifies, it doesn't invent demand). Growth is good news that creates problems:
  riders + coverage drift up while queues and crush-load pressure mount. Tests (`growth.rs`):
  near > ambient, cap, disabled-at-0, bit-for-bit replay. Known limit: an agent-mode population is
  generated once, so growth moves gravity/coverage but not agent trip counts — logged, not hidden.
- **Day-rollover report card** (`Beats.tsx DayReport`, reads the new `simDay` +
  `demandOriginTotal` stats fields): "Day N complete — riders +X · coverage Y (+Z) · gave up +W ·
  🏙 the city grew +G%". The turn punctuation; auto-dismissing, non-modal, baseline-aware
  (silent on boot + after undo/load rebuilds).
- **Milestone beats** (`Beats.tsx Milestones`): rider thresholds (100…250k) + every +5 coverage,
  one celebratory pill at a time, chime included. Baselines from the FIRST snapshot so loading the
  real network at coverage ~40 fires nothing — only new progress counts.
- **The score chase**: every city's REAL network is now an explicit bar to clear. Anchors measured
  in-browser against committed data (method + date documented in `cities.ts`): Singapore 41,
  Tokyo 38, Calgary 13, Istanbul 46, Manhattan 23, Dublin 39, Chicago 31, SF 47, Brisbane 29,
  London 45, Pyongyang 38, Glasgow 39, globe 54 — a natural difficulty ladder (beat the C-Train
  first, BART last). Menu city cards show "real network ~N · your best M" (localStorage best,
  from-scratch runs only); the StatsBar gauge tooltip names the bar; crossing it fires the
  headline "🏆 You beat the real <city> network" beat.
- **Sticky outcomes + retry**: the objectives end-banner now offers Retry (primary on a loss) next
  to Keep building, and the panel keeps a persistent "↻ Retry the challenge" after dismissal. The
  menu mirrors every start into the URL (`?city=&network=&scenario=`), so retry/refresh re-boots
  the exact setup instead of dumping to the menu.

## Line editing — extend, insert, redo (2026-06-10)

The audit's biggest remaining tweak-step friction: changing a committed line meant bulldoze +
redraw. It turned out to need NO new Command vocabulary — `AddStop{after}` covered extension and
insertion all along; only the UI (and a redo stack) was missing. Tiers: vitest 21 / playwright 16
(new `edit-line.spec.ts` asserts committed stop ORDER for both termini + mid-line insertion +
undo/redo round-trip + fork-clears-redo + the edited line still carries riders).

- **Extend** (`game.startExtend(line, head)`): seeds the draft with a terminus and reuses the
  whole draft pipeline — ghost, snap ring, DraftControls ("+N stops", ✓ Extend), water/afford
  pre-flight. The ghost dashes in the LINE'S OWN colour (extension reads as "continuing this
  line", not starting another). Commit = AddStop appends (tail) or insert-at-0 per stop (head —
  ordering preserved outward). Entry: Editor "Extend line" buttons (named by terminus), or
  pressing a terminus of the SELECTED line with the line tool (the Mini-Metro grab). Loop lines
  have no termini — refused. Waypoint handles are suppressed while extending (no "append bends"
  vocabulary — the blueprint must never differ from what commits). Mid-sequence afford rejection
  stops the sequence (a shorter extension is still a contiguous valid line; no RemoveStop exists
  to roll back with) — noticed, not hidden.
- **Insert mid-line** (`game.insertStopOnLine`): right-click an off-line station with a line
  selected → "➕ Add to <line>" places it at the span it projects closest onto (loop closing span
  included) — one AddStop, one undo step.
- **Redo** (`SimBridge.redoStack`): undo pushes the popped command; redo re-applies it through
  the normal `apply` path (log stays append-only); any fresh command forks history and clears the
  stack; loading a save clears it. Ctrl-Shift-Z / Ctrl-Y + a Redo button that only renders when
  there's something to redo. The UI slice gained `historyLen`/`redoLen` so the chrome re-renders
  exactly when history moves.
- Known limit: the Editor's Extend buttons live in the full (post-assignment) editor; an
  unassigned line extends via the terminus grab — which is the natural pre-assignment flow.

## Backlog clearance — aircraft picker, agent growth, runway (2026-06-10)

The three remaining deferred items, all unblocked. Tiers: cargo 28 suites (growth.rs now 4 tests)
/ vitest 21 / playwright 16; picker + runway verified live.

- **Aircraft picker** (the AIR roster's missing UI half, deferred since 2026-06-05 on contested
  files): the Editor shows an "Aircraft" ladder for AIR lines (Narrowbody/Regional/Widebody/Jumbo
  — capacity vs gate-turn, mirrored from `trainset.rs` in `shared.ts AIR_ROSTER`, index =
  `AssignTrainset.spec`). `LineStat` gains `trainset_spec` so the selection reads back from the
  snapshot; `game.assignTrainset` takes a spec defaulting to the line's CURRENT one — the headway
  slider's count re-derive no longer silently resets a chosen aircraft (verified live: pick Jumbo,
  drag headway, spec stays 3). Also fixed the deferred `world.rs` load-factor calc: per-line load
  now divides by `vehicle_spec().capacity` (the assigned aircraft), not the mode default.
- **Agent-mode growth** (the known limit from the one-more-day pass): `Population::grow_to` —
  once per day after `demand::grow`, the population tops up to the homes-derived target, drawn
  from the GROWN grid (new residents cluster where the growth happened). Append-only (existing
  citizens + in-flight trips untouched), RNG keyed by (seed, start index) so replayed top-up
  sequences redraw identical citizens — `growth.rs` asserts the rise AND bit-for-bit replay with
  agents + growth + top-ups all active.
- **Economy runway** (the audit's "no embodied failure signal"): the money box gains an
  operating-cash trend — `statsHistory.cashTrend` fits fares−opex per in-game day over recent
  samples (capital EXCLUDED: a one-time build is a step, not a burn). ▲/▼ glyph beside the
  balance, full detail in the tooltip ("burning $X/day — ≈N days of runway"), and the inline ≈Nd
  count appears only under 30 days — the drain is visible long before the afford-gate fires.

## Headway made honest — the clock-frame unification (2026-06-11)

The audit's last open headway item, deferred twice as "would require rebuilding the speed model".
It did. The sim's physics ran in real-world time while the player's clock ran 30× faster, so
"Headway: 6 min" meant a train every 3 in-game HOURS; waits, trips, and the day were three
different time languages. Now there is ONE: `tod::CLOCK_SCALE = 30` (= 3.6M / HOUR_MS), and every
duration the player reads is true against the clock they watch.

- **The rescale** (constants only — zero algorithm changes, determinism untouched): ground/water
  speeds ×30 (a "80 km/h" metro covers 80 km per CLOCK hour), accelerations ×900 (braking distance
  v²/2a is frame-invariant, so stopping behaviour over real metres is identical), dwells ÷30
  (rail 21 clock-s, restated per mode), headway clamps → 1–60 CLOCK-minutes (2_000–120_000 sim-ms,
  slider unchanged at 2–20 but now meaning clock minutes), patience → 10 clock-min, walk speed +
  ACCESS_DECAY ×/÷30 (footpath transfers ~5 clock-min; accessibility weighting over real geography
  unchanged), curve LAT_ACCEL ×900 (curves bind identically over real radii).
- **Mini-Metro capacity rescale** (÷30: rail 7, bus 3, ferry 13, heavy 18): trips/day ×30 ×
  riders/trip ÷30 ⇒ load factors, queue magnitudes, denied/renege pressure, fares/day and opex
  trajectories all match the old tuning with spawn rates, agents, ToD, growth, and day length
  UNTOUCHED. DWELL_PER_PAX 100 ms (3 clock-s/boarder) keeps bunching at the same relative strength.
- **Parity measured live**: starter corridor day-1 = +223 riders (was +240 on a slightly bigger
  build), load 7%, growth +0.9%/day — and the readouts finally cohere: headway 4.1 clock-min
  (the auto-suggest now lands on plausible metro numbers by itself), avg wait 2.5 clock-min
  (≈ headway/2 ON THE CLOCK), avg trip 11.3 clock-min for ~7 km. **1× is playable**: trains every
  ~8 wall-seconds instead of 6 minutes of dead air. Full e2e wall time fell 3.2 min → 53 s for
  the same behavioural asserts (ridership develops 30× faster in wall time).
- **The globe stays story-framed** (deliberate, documented in `spec_for_mode`): air speeds remain
  "a hop is near-instant"; its 45–120 s roster dwells now read as plausible 22–60 clock-minute
  gate turnarounds for free (picker copy updated). The globe is the one mode whose speeds aren't
  clock-honest.
- **Mirrors + displays**: `shared.ts SIM_MS_PER_CLOCK_MIN = 2_000` is the single display
  conversion (slider, roster, tooltips, dashboards, reach bands, journey/wait minutes in
  `journey.rs`); `MODE_SPECS` replaces `roundTripMs`'s stale hardcoded rail spec (which had been
  estimating bus/ferry/heavy headways at rail speeds — a real pre-existing bug). Anchors
  re-measured post-change (most +1–3 from the friendlier quality span; globe 54→64); cities.ts
  updated.
- **Caveats**: old saves replay with out-of-range headways clamped into the new 1–60 clock-min
  band (sim version bump, acceptable); opex's "per day" basis (86.4M sim-ms) was already
  real-time-denominated and is deliberately untouched (balance trajectories preserved).
- Tiers: cargo 28 suites (ferry speed-proxy window shortened; clock-frame re-pins in ridership/
  raptor/pressure-buckets), vitest 21 (capacity re-pin), playwright 16. Adversarially reviewed by
  a 3-lens find→refute workflow over the diff before commit.

## Capacity & topology roadmap — P1: block following + train length (2026-06-12)

Started the **capacity-as-buildable** roadmap (full design in [docs/capacity-roadmap.md](docs/capacity-roadmap.md)):
line capacity stops being a flat clamp (`MIN_HEADWAY_MS`/`MAX_TRAINS_PER_LINE`) and becomes
network physics that emerges from track and is fixed by spending money — a **movement-authority
layer** that each phase extends (P1 following → P3 branching → P2 single/double track → P4 junction
conflict → P5 shared-track go/no-go). Decisions locked: route-**tree** branching (handles the JRL
3-way Bahar Junction + the missing Circle Line CE branch), train length **derived from spec** (not a
lever), service pattern **round-robin default but player-settable**, sequence **P1→P3→P2→P4**.

**P1 (this commit)** — the authority layer with its first resource, TDD red-first
(`tests/following.rs`):
- **Train length** (`trainset.rs` `length_mm`, spec-derived: rail 140 m … bus 12 m; AIR flavour-only)
  + `brake_distance_mm`/`block_gap_mm` helpers. No new state, no schema/command/save change.
- **Dispatch density cap** (`dispatch.rs`): a line places only as many trains as fit at
  `braking + standoff + length` apart; the surplus isn't dispatched, so over-provisioning is
  self-limiting and the effective headway floors at the block. `MAX_TRAINS_PER_LINE` demoted to a
  buffer-sizing backstop. Long lines unaffected (their block fits >24); only short over-subscribed
  lines bind.
- **Move-phase follow clamp** (`vehicle.rs`): in the monotone loop coordinate `p` (unifies loop /
  out-and-back; leader = next vehicle in the per-line SoA run, cyclic, a lone train's leader is
  itself so it never binds), each train's advance is capped to hold a braking-distance gap behind the
  leader's **tail**. Start-of-tick leader snapshot ⇒ order-independent & deterministic. Binds only on
  desync (a dwelling/slowed leader, bus self-congestion, later branches) — homogeneous lines stay
  byte-identical (parity), so no overtaking and emergent bunching, no per-frame cost.
- **Why no re-pins:** the clamp is a no-op for well-spaced low-count lines and the dispatch cap only
  trims over-provisioned ones, so every existing sim/e2e scenario was unchanged — confirming default
  parity. The one observable change: an over-subscribed short line now runs fewer trains than the
  slider requests (read back from the snapshot, per the AGENTS "UI reflects the clamped value" rule).
- Tiers: cargo (determinism gate + new `following.rs` + all existing suites green, **zero re-pins**),
  vitest 21 (wasm rebuilt), playwright 16 (real Singapore MRT still carries riders, full slice, Tokyo
  boot). Determinism bans clean.
- **Next:** P3 — branching lines (the Circle Line / JRL fix; the data-model + contract-mirror phase).

## Capacity & topology roadmap — P3 core: branching lines (2026-06-12)

The data-model phase (docs/capacity-roadmap.md): a line is a **trunk + a tree of branches**, so a
Y-shaped real service is representable — the Circle Line's missing CE branch (Promenade→Bayfront→
Marina Bay) and the Jurong Region Line's 3-way Bahar Junction. Confirmed root cause first: `Line` was
a flat linear `stops` array (`line.rs`), and the importer's `min_stops=3` stub filter dropped the
standalone branch. TDD red-first (`tests/branching.rs`: a Y-line where a train must reach a branch
terminus E off the trunk).

The clean model (no graph rewrite): each route through the tree is materialised as a linear service
**`Path`** (the trunk, or a trunk-prefix continued onto a branch), so every `Path` is exactly the old
single-polyline line and all vehicle motion / routing / rendering runs **per-path unchanged**. A
non-branched line is one path (`paths[0]`) and behaves identically — which is why **zero existing
tests re-pinned**.

- **`line.rs`** — extracted geometry into `Path` (polyline/arclen/stop_arclen/speed_cap/min_radius/
  span_mode + loop flag); `Line` keeps the trunk `stops` + `branches: Vec<Branch>` + derived `paths`,
  with trunk-delegating accessors. `path_specs()` enumerates root-to-leaf paths (branches may share a
  divergence — JRL's 3-way).
- **`command.rs`** — `AddBranchStop{line,branch,diverge_at,station}` (+ `BranchStopAdded`), mirrored
  in `types.ts`/`codec.ts` the same commit. `branch==len` opens a branch off `diverge_at`; `<len`
  extends it. Afford-gated + rollback like `AddStop`.
- **`world.rs`** — `rebuild_line_geometry` builds one Path per route (corridor A* for bus/ferry,
  waypoints for the trunk); cost/water/disruption sum over paths with **shared-trunk-prefix skipping**
  (no double-count); serving + `best_headway_at` include branch stations; `state_hash` gains
  `veh_path` (a train's path is state). Branch span-mode editing + per-branch waypoints deferred.
- **`dispatch.rs`** — trains split **round-robin across paths** (train k → path k % npaths), each path
  its own circuit with the P1 block-density cap. **`vehicle.rs`** — `advance` is path-aware and the
  P1 follow-grouping keys on `(line, path)` (cross-path conflict on the shared trunk is the deferred
  P4 junction phase). **`raptor.rs`** — `build_routes` emits a directed route set per path; rider wait
  scales ×npaths (npaths==1 ⇒ old headway/2, so routing is unchanged for normal lines).
- Tiers: cargo 28 suites (determinism gate + new `branching.rs` + all existing, **zero re-pins**),
  vitest 21 (wasm rebuilt), tsc clean, playwright 16 (real MRT carries riders, Tokyo, full slice).
  Bans clean.
- **Deferred to Stage C** (clearly, not half-built): the **importer** (real Circle Line/JRL branch
  data — needs lifting the stub filter), **branch TRACK rendering** (a `LineView` contract change to
  carry per-branch polylines), and the **service-pattern UI** (`SetServicePattern` — the round-robin
  default already works; a player lever is the add). Until then a branch is operable + routable in the
  core but draws only its trunk.

## Real-world imported lines follow OSM geometry (2026-06-12)

Stage-C work began with the geometry ask: imported real networks were drawn as idealised
Catmull-Rom curves through the STATIONS (the importer captured stops only), so they didn't follow
the real track. Now they do — and the merged-interchange snapping that surfaced is fixed.

- **Literal geometry** (`line.rs` `Path.literal` / `Line.literal`, `CreateLine.literal` + mirror):
  an imported line follows its supplied (OSM) vertices directly with only a VERY MINOR centripetal
  pass (`LITERAL_SAMPLES = 2` — rounds the raw corners, no big invented curve, no 10× polyline
  bloat). Player-drawn lines keep the full smooth. Curve speed caps still come from the real
  vertices, so a real line's tight curves slow trains correctly.
- **Importer geometry** (`build_networks.py`): the Overpass query now `out geom`; `stitch_ways`
  chains the member ways into one continuous track polyline; `span_geometry` splits it at each
  station (nearest vertex) into per-span intermediate vertices; **Douglas-Peucker** (`simplify_rdp`,
  ~30 m) decimates the dense OSM track (Tokyo 91788→9279 verts, ~10%) so the save / state-hash /
  boot stay cheap. Emitted as `geometry` in the network JSON; `applyNetwork` creates the line
  `literal` and sets the per-span [lng,lat]→mm waypoints (the one `coords/geo.ts` crossing).
- **Interchange snapping fixed** (the reported artifact): same-name stops merge to ONE station id
  (for transfers) but each line runs through its OWN platform, so forcing the merged point into the
  polyline spiked the track to whichever line was imported first. `Path::rebuild` now anchors a
  literal line's stop to its **on-track** vertex (the adjacent real waypoint), not the merged point —
  the spike is gone; the station id stays shared. (The station DOT still sits at the merged point; a
  per-line platform model is a larger follow-up.)
- **Re-baked 11 cities** with decimated geometry (brisbane/calgary/chicago/dublin/glasgow/istanbul/
  manhattan/pyongyang/sf/singapore/tokyo). **London timed out** on Overpass (97-line Tube) and keeps
  its old no-geometry data — it degrades gracefully to straight lines. Re-baking also refreshed the
  networks to current OSM + importer (e.g. Tokyo 440→477 stations, 32→72 lines), so the cities.ts
  real-network anchors may want re-measuring (soft calibration, not a correctness issue).
- Browser-verified on Singapore: lines follow the real MRT alignment, interchanges weave cleanly (no
  spikes), gently rounded; 0 console errors; 58 trains, ridership develops. Tiers: cargo 28 suites
  (determinism green; literal only affects imports, zero re-pins), vitest 21, tsc clean, playwright
  16 (Tokyo re-boots in ~52 s with the decimated geometry; previously timed out at 60 s).
- **Still pending in Stage C** (the original branch goal — the geometry ask took priority): **branch
  import** (detecting the Circle Line CE / JRL branches from OSM — a heuristic sub-problem), **branch
  TRACK rendering** (LineView per-branch polylines), and the **service-pattern UI**. The P3 core runs
  branches; they're just not yet imported or drawn, so the Circle Line still shows no Marina Bay spur.

## Stage C — branches surfaced: the Circle Line gets its Marina Bay spur (2026-06-12)

The P3 core ran branches but nothing imported or drew them. Closed two of the three Stage-C gaps —
**branch import** + **branch track rendering** — so a real branched service now appears on the map.

- **Branch import** (`build_networks.py`): OSM carries the Circle Line as four `ref=CCL` route
  variants — two terminate at Dhoby Ghaut (the arc), two run to Marina Bay (the CE spur) — and the
  importer used to keep only the longest and discard the rest. Now it keeps ALL variants, takes the
  longest as the trunk, and mines the others for branches: `branch_from_variant` finds a variant that
  shares a prefix with the trunk then continues into NEW track, and emits it as a branch diverging at
  the junction. Robust (uses OSM's own variants, no cross-line guessing) and conservative (a shorter/
  subset variant never invents a branch). Re-baked Singapore → the Circle Line now carries a branch
  **@ Promenade → Bayfront → Marina Bay**, and exactly one line gets a branch (no false positives).
  Emitted as `branches` in the network JSON; `applyNetwork` builds each via `AddBranchStop`.
- **Branch track rendering** (`stats.rs` `LineView.branch_polylines_mm`, `world.rs` export, `types.ts`,
  `game.ts`): the line geometry view now carries one polyline per branch path; the `LinePath[]`
  builder flat-maps a line into its trunk + branch paths (same id/colour), so the deck PathLayers draw
  the spur in the line's colour. Browser-verified: the Circle Line shows its Marina Bay branch,
  selectable, 0 console errors.
- Tiers: cargo (determinism + branching green, LineView field additive — zero re-pins), vitest 21,
  tsc clean, playwright 16 (Singapore + Tokyo boot and carry riders).
- **Still deferred:** **branch literal geometry** (the spur renders STRAIGHT — branch paths get no
  imported waypoints yet, so Promenade→Marina Bay is a straight segment; fine for a short spur, a
  follow-up for long branches); **service-pattern UI** (`SetServicePattern` — round-robin default
  already works); **branch water-legalization** (`SetSegmentMode` is trunk-only, so a branch crossing
  water at surface would park — the CE branch doesn't, so it runs); and **re-baking the other cities**
  with branch detection (only Singapore re-baked this pass; the others have geometry but not yet
  branches — the importer handles it on their next bake).

## Stage C — branches follow real geometry; every city re-baked (2026-06-12)

Closed the branch-geometry gap and spread branches to all cities.

- **Branch literal geometry** (`Branch.waypoints` + `SetBranchWaypoints` command + mirror): a branch
  now carries its OWN per-span shaping points (the spur's real OSM alignment). `rebuild_line_geometry`
  builds a branch path's spans as the **trunk's waypoints for the shared prefix** (so the spur matches
  the trunk exactly up to the divergence) **plus the branch's own waypoints** for the spur — so a
  branch follows the real track end-to-end, not a straight chord. The importer emits each branch's
  geometry by splitting its variant's track at `[junction, *branch_stops]`; `applyNetwork` sets it via
  `SetBranchWaypoints`. Browser-verified: the Circle Line's Marina Bay spur now curves along the real
  alignment (227-vertex branch polyline vs the old straight ~5).
- **Re-baked all 12 cities** with the full pipeline (real geometry + branch detection + branch
  geometry). **London succeeded this time** (now has real geometry + 20 branches — the Tube is heavily
  branched). **30 branches detected across cities** (london 20, tokyo 4, chicago/dublin/glasgow/
  istanbul/manhattan/singapore 1 each) — the variant-based detection generalises well, no manual
  per-city tuning. (Boot is NOT the bottleneck — measured __MAP_READY ≈ 8 s Tokyo / 10 s London even
  with the branches; the e2e spec's ~1 min is the ridership-develop wait, not the boot.)
- Tiers: cargo (determinism + branching green, additive — zero re-pins), vitest 21, tsc clean,
  playwright 16 (Singapore + Tokyo boot, carry riders).
- **Service-pattern UI: deferred by design.** Trains already split round-robin across a branched
  line's paths (a sensible default); a per-branch service-bias lever is exactly the NIMBY-depth AGENTS
  says to defer unless it sharpens the thin loop, so `SetServicePattern` stays a seam, not built.
  **Stage C is otherwise complete** — branches import, follow real geometry, and render in every city.
- Remaining branch caveats (small): a branch crossing water at surface can't be tunnelled
  (`SetSegmentMode` is trunk-only) so it would park (no current city hits this); branch waypoint
  *editing* in the UI is absent (imports set them; players can't yet hand-shape a branch).

## Stage C watch items — branch water-legalization fixed (2026-06-12)

Worked the post-Stage-C watch items; one was a real bug, one a false alarm.

- **Branch water-legalization (FIXED).** A loaded real network whose branch crosses water at surface
  kept the WHOLE line parked (no dispatch, renders red), because `legalizeWaterCrossings`'s safety net
  tunnels via a whole-line `SetSegmentMode` that only touched the trunk path. Hit on a flagship city:
  **London parked the Elizabeth line, Mildmay (Overground) and DLR** — all branch the Thames/docks.
  Fix: a whole-line `SetSegmentMode` (`span == u32::MAX`) now sets EVERY path's span modes (trunk +
  branches), so legalizing tunnels the branch crossings too. Per-span (specific span) edits stay
  trunk-only (per-branch span editing still deferred). Verified live: London **0 parked (was 3)**, 313
  vehicles dispatched, ridership develops; boots in ~8 s. Non-branched lines unchanged (their only path
  IS the trunk), so determinism + all tiers stay green (cargo, vitest 21, playwright 16).
- **Boot performance (FALSE ALARM).** The earlier "Tokyo ~58 s boot" was a misread — that's the e2e
  spec's TOTAL (incl. the 45 s ridership-develop wait). Measured `__MAP_READY` directly: **~8 s Tokyo,
  ~10 s London** even with the branches. No fix needed; PROGRESS note above corrected.
- Still open (small, by design): per-branch span-mode EDITING in the UI, branch waypoint editing, and
  the service-pattern lever (round-robin default works).

## Importer fix — drop depot/siding detours from line geometry (2026-06-12)

Reported on the .sg geometry deploy: the Downtown Line "looped to depot" at Bukit Panjang, and the
LRT loops looked broken. Root cause: `stitch_ways` chains ALL of a relation's member ways including
**non-revenue depot/siding track**, so a span inherited a huge detour — the DTL into Gali Batu depot
(span Hillview→Hume: 7.6 km path for a 0.9 km gap, 8.6×), and the Bukit Panjang / Punggol / Sengkang
LRT loops' closing spans into their depots (5.7–11×).

- **Fix** (`span_geometry`): drop any span whose path length exceeds **3× the straight station gap** —
  that's never the real alignment (a real curve is ≲2×), so the span falls back to straight. General
  and robust (no per-line tagging).
- **Post-processed all committed networks** with the same filter (no re-fetch): **764 detour spans
  dropped** — singapore 4, but it was widespread (london 207, manhattan 216, tokyo 125, glasgow 81,
  chicago 51, …); many cities had depot/siding wanders. Pyongyang had none.
- Browser-verified on Singapore: the Downtown Line terminus at Bukit Panjang is clean (no depot loop)
  and the BPLRT renders as a proper loop. Tiers: e2e network + slice green (Singapore + Tokyo boot,
  carry riders).

## Importer fix — split same-ref distinct routes (LRT East/West loops) (2026-06-12)

Reported on .sg: the Sengkang & Punggol LRT each show only ONE of their two loops. Cause: OSM tags
BOTH the East Loop and West Loop with the SAME `ref` (SKLRT / PGLRT), and the importer grouped all
same-ref variants into one line keeping the longest — so the other loop was discarded. But East and
West are DIFFERENT physical loops (disjoint stations, sharing only the central interchange), not
direction variants.

- **Fix** (`cluster_variants`): within a ref, cluster variants by **station-set overlap** (union-find;
  share >half the smaller variant ⇒ same route). Same-route variants stay one line (→ trunk +
  branches, e.g. the Circle Line); near-disjoint ones (the LRT East vs West loops) each become their
  own line, so neither loop is lost. Re-baked: Singapore now has **both** Punggol loops AND both
  Sengkang loops. The split also (correctly) separates a few genuinely-distinct same-ref services
  elsewhere (e.g. an airport shuttle), at the cost of a couple of duplicate line names — acceptable.
- Re-baked all 12 cities (with the depot-detour filter from the prior fix): line counts rose where
  routes split (london 99→105, tokyo 72→77, singapore 10→13). Tiers: e2e 16 (Singapore + Tokyo boot,
  carry riders), determinism green.

## Branch UI — branches become first-class in the Editor (2026-06-12)

Took the deferred branch-UI bits: branches are now editable objects, not read-only imported topology.

- **Sim** (`SetBranchTrack` + `RemoveBranch` commands, mirrored): per-branch build mode (sets the
  branch's OWN spans past the divergence — the shared trunk prefix stays governed by the trunk's
  Track control, afford-gated like the line Track), and bulldoze a branch (the trunk + other branches
  stay; reversible by replay). `LineView` gained `branch_modes` (uniform mode per branch, −1 if mixed)
  + `branch_termini` (terminus station id) for the UI.
- **Editor** (`Panels.tsx`): a **Branches** section on a branched line — each spur shows "⑂ → <terminus>"
  with a rail Surface/Elevated/Tunnel toggle and a × bulldoze (testids `branch-N`,
  `branch-N-mode-M`, `branch-N-remove`). `game.setBranchMode` / `removeBranch` funnel to the commands.
- Browser-verified on the Circle Line: the Branches row shows "→ Marina Bay"; setting it to Tunnel
  updates `branchModes` to [2]; removing it drops the spur (0 termini/polylines). Tiers: cargo
  (determinism + new commands green), vitest 21, tsc clean, playwright 16.
- **Service-pattern lever** stays deferred by design (round-robin works); branch waypoint *editing*
  (hand-shaping a spur) and per-branch *span* editing also remain out (whole-branch mode covers the
  real cases).

## Capacity roadmap — P2: single vs double track (2026-06-13)

The cost/capacity lever (docs/capacity-roadmap.md): a per-span `track_type` (default **Double**, so P1
replays byte-identical until set). On a SINGLE span opposing trains cannot both be inside — they MEET
at the bounding stations (passing places); single track is **~half the build cost, lower capacity**.
Built with a 3-workflow pipeline (understand → adversarial design → adversarial review), which is what
made the determinism + deadlock correctness hold.

- **Meet protocol** (`vehicle.rs`): `advance()` split into three index-ordered passes — derive
  (P1-clamped desired advance), resolve (single-span entry gating), commit. Single-track is **block
  working by train identity** (one train per single span); a train at a TERMINUS reserves the adjacent
  single span through its turnaround (a dead-end is not a passing place). The gate keys off the span
  the move CROSSES INTO (`next_arc`'s stop), not `span_of(s+ds)` — the decisive fix the design
  adversaries found (correct for sub-tick spans + gate-departing trains). Loops are exempt (one-way ⇒
  no meets). Occupancy is **re-derived from the SoA each tick** (sorted Vecs, binary search, no
  HashMap, integer) — never persisted, never hashed; the only hashed addition is `Path.track_type`,
  so **double-track motion is bit-identical (zero re-pins)**.
- **Liveness via dispatch cap** (`dispatch.rs`): the review caught a real **P1×P2 deadlock the design's
  proof missed** — once trains exceed a line's passing capacity, the meet protocol gridlocks (an
  occupant is P1-blocked behind a train P2-held by that occupant). The meet protocol can't untangle
  it; **liveness is guaranteed upstream** by a single-track capacity cap: a path with single spans
  runs at most (DOUBLE spans + 1) trains — a fully-single out-and-back is a one-train shuttle — the
  surplus undispatched (the `max_fit` self-limiting pattern). Single-track placements snap to passing
  places (never mid-block) so the head-on invariant holds from tick 0; loops skip the snap (no perturb).
- **Cost** (`world.rs`): single track = `SINGLE_TRACK_PCT=55`% of double per-km capital. **Command**
  `SetSegmentTrack{line,span,track}` mirrors `SetSegmentMode` (whole-line over all paths / per-span
  trunk; afford-gated; does NOT set `dispatch_dirty`); `types.ts`/`codec.ts` mirrored. **UI**: a
  Double/Single toggle in the Editor's Track section (rail), reading `LineView.track_types`.
- **Tests** (`tests/single_track.rs`, red-first): head-on safety + multi-train meet liveness;
  **over-provisioned never freezes** (the at/above-threshold deadlock cases the first cut missed —
  2-stop/2-train, 4-stop/4-train, …); determinism with `SetSegmentTrack` in the log; cost cheaper;
  single-track loop = double-loop dispatch (pure discount); double-track lets opposing trains pass.
- Tiers: cargo 29 suites (determinism gate + 7 single_track + all existing, zero re-pins), vitest 21,
  tsc clean, playwright 16. **Deferred** (seam, not half-built): per-branch single track; coalescing a
  multi-span single run as one block (each span is independently reservable today); a `max_fit` shrink
  for single-track (emergent bunching is the intended AGENTS-aligned pressure).

## Capacity roadmap — P4: junction conflict (2026-06-13)

The authority layer's **4th `min()`** (docs/capacity-roadmap.md): a branched line's trains genuinely
converge on the shared switch where a branch leaves/rejoins the trunk, so a mutex forbids two consists
straddling it. P4v1 covers **same-line branch divergence/convergence** only; at-grade *crossings*
between distinct lines and the shared-trunk *section* mutex stay P5 seams. Built with the
understand → adversarial design → adversarial review workflow pipeline (the design adversaries caught a
deterministic, replay-gate-invisible deadlock the candidate's liveness proof missed).

- **The mutex** (`vehicle.rs`): Phase **B.4** clamps `ds` after P1 block-follow and the P2 meet — a
  train cannot cross a switch cluster another consist occupies. A consist occupies the cluster while
  `[head−dir·len, head]` overlaps its per-path `[lo,hi]` (half-open `group_overlap`, the ONE shared
  predicate used in both the occupancy pass and the owner early-out, so they can't disagree).
  Occupancy is **re-derived each tick** (Phase A.1.5; sorted Vecs, `occ_claim`/`occ_owner`/`try_claim`,
  binary search, no HashMap, integer) — never persisted, never hashed. `world.junctions` is derived in
  `dispatch.rs` on `dispatch_dirty` (same trigger as `serving`), also **unhashed** — a pure function of
  the already-hashed topology. So a **non-branched network is byte-identical (zero re-pins)** — strictly
  stronger than P2, which had to hash `Path.track_type`.
- **Coalescing = the liveness fix.** Two switches within one consist-length on the trunk form a 2-cycle
  deadlock under a naive point-mutex (A holds J1 + gated at J2, B holds J2 + gated at J1; the denial arm
  is index-independent, so the tiebreak the candidate's proof relied on is never consulted — the same
  failure class as P2's terminus). Merging them into **one atomic group** (key = `min` member
  `StationId`, command-order-independent) collapses the cycle: a consist straddles ≤1 group, all
  contenders for any member point share one key ⇒ an acyclic depth-1 wait-for forest. Clearing time
  ∝ `length_mm` for free (a 200 m HEAVY holds the switch longer).
- **Two corrections vs the locked design** (RED-first tests pin both): (1) the design's §4.3 **dispatch
  cap was dropped** — a switch is a POINT crossing occupied only ~`length_mm` of travel, so its
  throughput dwarfs P1's per-path block density (`max_fit` binds first) and the coalesced mutex is
  deadlock-free, so over-provisioning just queues at the gate; the block-sized cap the design implied
  would throttle every branched line to ~2 trains (the `dense_*` tests pin a real fleet). (2) The
  design's **Phase B.5 junction no-rest extension was unnecessary** — B.4's crossing test uses
  `s <= gate` (trains almost always *depart* the junction station, which sits ON the gate, so `s==gate`
  is the dominant case; strict `<` would never bind), and start-of-tick occupancy denies entry while
  occupied, so a non-owner can never enter — let alone rest — inside a cluster.
- **Dispatch snap** (added, not in the design; the cap removal exposed the tick-0 case the cap had
  implicitly covered): a placement whose consist would straddle a cluster snaps to the near gate (the
  junction station), so the switch is collision-free from tick 0. **Verified load-bearing by sweep** —
  42 dense-early-junction configs straddle at dispatch without it.
- **The adversarial review found two real bugs** (both deterministic ⇒ the replay gate is blind to
  them): (a) **CRITICAL — coalescing keyed on the wrong axis** (the design's Residual Risk #2, realised
  with ordinary smoothed geometry): it coalesced on the **trunk** gap, but the mutex keys on **per-path**
  spans, and a branch path's shared-prefix arclen can be *shorter* than the trunk's (Catmull-Rom pulls
  the branch straight while the trunk bows toward its post-junction stop) — so two switches >`len` apart
  on the trunk but <`len` on a branch stayed split, and a branch consist straddling both **gridlocked
  the line** (the exact 2-cycle coalescing exists to kill). **Fixed:** coalesce on the **MIN gap over
  shared paths**. (b) **P5 seam, deferred — single-track on a branched line's SHARED TRUNK:** P2's meet
  keys per `(line, path, span)`, so the trunk path and a branch path get different keys for the *same
  physical rail* and opposing consists pass through each other. Pre-existing P2×P3 (untouched by P4);
  a correct fix needs the P5 physical-track model **plus** a cross-path liveness cap (a half-fix turns
  it into a *worse* deadlock). Captured `#[ignore]`d.
- **Tests** (`tests/junction.rs`, red-first): mutual exclusion (Y-line + JRL 3-way); coupled-junction
  never-deadlock (RED without coalescing, RED on safety until the mutex); **branch-coupled junctions
  coalesce + run** (RED with trunk-only coalescing — bug a); dense early junction + dense loop+spur
  clean from dispatch; single train not self-gated; grade-sep does NOT dissolve the switch mutex;
  determinism replay + command-order-stable keys; non-branched line derives no junctions; `#[ignore]`d
  P5 shared-trunk seam (bug b).
- Tiers: **cargo 31 suites** (determinism gate + 10 junction +1 P5-ignored + P1/P2/P3 + every existing
  fixture, zero re-pins), vitest 21, tsc clean, playwright 16 (incl. Singapore real MRT + Tokyo
  440-station). **Deferred** (seams, not half-built): at-grade line crossings + shared-trunk section
  mutex incl. single-track-on-shared-trunk (P5 go/no-go); a turnout speed cap at divergences
  (`speed_cap_at` seam); the optional `LineView.junction_points` amber-dot readout.

## Known gaps / deferred

- **T7 (self-host PMTiles)** — deferred per PLAN §15; slice ships on the hosted CARTO/MapLibre style. Not on the critical path.
- **Real OSM demand (pyrosm)** — deferred; T13 ships a deterministic synthetic grid (sim consumes the JSON identically).
- **Done since the slice:** curves+speed caps, time-of-day, transfers (BFS+cache), real OSM networks +
  6 cities, buildability/build-modes, economy (capital+fares), transport modes (rail/bus/ferry/air),
  demand layer, settings, **time-dependent RAPTOR routing**, demand/traffic visibility (5 tracks),
  **accessibility isochrone**, **inter-station footpaths**, freeform line waypoints. Remaining seams:
  multiplayer, GTFS import, departure **timetable**, track junctions, terrain gradient.
- **idea.md "pt 2" (user-added 2026-06-04):** game modes — *sim mode vs grand-tycoon mode*, *pure-sim vs
  GSG-inspired mode with events*. Future scope, well beyond the thin slice. Noted, not built (guard the loop).
  The command-sourced deterministic core is mode-agnostic, so a future "mode" is a new outer-ring layer +
  Command/Event variants, not a core rewrite.
