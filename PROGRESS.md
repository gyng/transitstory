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
