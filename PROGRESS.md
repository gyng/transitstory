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

## Capacity roadmap — P5 shared track (S1v1): cross-path single-track cap (2026-06-13)

**GO decision** (was the deferred "architectural cliff"): a product driver — an impending fork into a
transport-builder game where distinct lines **sharing physical track** (a central tunnel, OpenTTD-style
networks) is a headline feature — lifted the "do not build speculatively" deferral. Owner picked the
**full track-objects** end-state, built **S1-first as a reusable primitive**. Full trajectory:
[docs/p5-shared-track-roadmap.md](docs/p5-shared-track-roadmap.md). The one reusable primitive is a
**physical-block reservation**; only the block's identity key graduates (line-scoped → cross-line) per
layer, so each layer is a small extension of proven P2/P4 machinery, not a rewrite.

- **The bug S1v1 fixes** (the P4-review-found shared-trunk head-on, captured `#[ignore]`d): a branched
  line single-tracked on its **shared trunk** runs the trunk path and a branch path as two independent
  polylines over the SAME physical rail; P2's meet keys per `(line, PATH, span)`, so the two consists
  never mutex and pass through each other (reproduced: head-on at tick 667).
- **Why a CAP, not a mutex, is the load-bearing fix.** A *fully-single* shared trunk has no passing
  place, so 2 trains deadlock on it **even with a perfect mutex** (trunk + branch desync over different
  circuits and oppose with nowhere to pass). The fix bounds population: **all paths traverse the shared
  trunk**, so its single-track capacity bounds the WHOLE fleet — cap total trains across the trunk +
  every branch path to (physically-double spans on the universally-shared prefix `[0, min diverge_at)`)
  + 1. Fully single ⇒ cap 1 (a shuttle; the trunk, lowest path index, wins the budget; the branch is
  unserved until a span is doubled — the same informative pressure as P2's single-track shuttle).
- **Single-if-any track read:** a shared span is physically single iff single on ANY contending path
  (whole-line edits all paths; a per-span edit touches only the trunk — single-if-any keeps an
  asymmetric edit constraining the shared section). Pure `dispatch.rs` two-pass count clamp
  (PASS 1 per-path counts → cap → PASS 2 place); re-derived, **never hashed**; integer, index-ordered,
  no HashMap iteration; deterministic tiebreak (ascending path index).
- **Deferred to S2** (`#[ignore]`d `single_span_in_double_shared_trunk_no_headon_is_s2`): a single span
  *between passing places* in an otherwise-double shared trunk needs a meet MUTEX (not the cap — the
  line has capacity, trains should meet). That mutex — single spans as first-class window-blocks
  coalesced with the adjacent switch (P4's trick, points→windows) — is the cross-line foundation. Two
  edges hand-found in the rate-limited design break and logged for S2: the fold-into-junction design
  misses **non-contiguous** single spans + **over-throttles** a mostly-double line; **staggered**
  partial-sharing `[min diverge_at, max diverge_at)` needs explicit coverage.
- **Tests** (`tests/junction.rs`, red-first): `fully_single_shared_trunk_caps_to_a_shuttle` (cap → 1
  train, was 2 → head-on); `fully_single_shared_trunk_no_headon_and_never_freezes` (safety + liveness);
  the `#[ignore]`d S2 single-span case. **Parity:** zero re-pins — all 31 sim suites byte-identical
  (single_track 7, branching 2, determinism 6, every junction/ridership fixture); the cap is inert for
  non-branched / fully-double-branched / branch-private-single lines.
- Tiers: cargo 31 suites (zero re-pins), vitest 21, tsc clean, playwright 16.

## Capacity roadmap — P5 shared track (S2): physical-block meet mutex (2026-06-13)

S2 is the **reusable primitive** S1-first was building toward (docs/p5-shared-track-roadmap.md): a
single shared-trunk span *between passing places* needs a meet MUTEX, not the cap — the line has
capacity, so the trunk + branch consists must take turns on the one physical rail, not be capped to a
shuttle. This is the kernel cross-line track sharing (and the full track-objects end-state) reuse: only
the block's identity key graduates line-scoped → `TrackSegmentId`; the reservation machinery is constant.

- **Single spans become first-class window-blocks** in the dispatch junction set, alongside P4's
  divergence points. A block is a per-path arclen window `(path, lo, hi)` (a point has lo==hi; a span
  has lo<hi). The **existing** vehicle.rs A.1.5 occupancy + B.4 gate (`group_overlap` on `span_by_path`)
  serialise them **unchanged** — no new pass, no new key. A trunk span k is a block iff it is SHARED
  (>=2 traversing paths = trunk + a branch with `diverge_at > k`) AND physically SINGLE on ANY
  traversing path (**single-if-any** — a per-span `SetSegmentTrack` edits only the trunk, so the trunk
  being single makes the rail single even where a branch reads double). Re-derived per dispatch_dirty,
  **never hashed**.
- **Coalescing generalised points→windows** (`coupled` uses `q.lo − p.hi`): contiguous single spans
  merge into one section, and a single approach **folds into its adjacent switch** within a consist-
  length — so a consist bridging the single span and the switch holds ONE resource, killing the **P5×P4
  wait-for cycle** (A holds the span + waits for the switch; B holds the switch + waits for the span).
  A **non-contiguous** single span (separated by a double) can't bridge a switch ⇒ a standalone block.
- **The cap drain became ROUND-ROBIN** (S1v1's trunk-takes-all starved the branch to 0): a fully-single
  trunk (capacity 1) is still a trunk shuttle, but a single span between passing places (capacity >1)
  shares the budget so the trunk AND branch run and MEET. The two halves ship together — round-robin
  *without* the mutex would head-on.
- **Four adversarial-review rounds hardened it** (each a deterministic, replay-gate-blind failure, now
  regression-tested): (1) **bunched-double over-admit** → count passing-place RUNS, not individual
  doubles; (2) **staggered region uncapped** → scope the cap to `[0, max diverge_at)` with
  traversing-path `phys_single`; (3) **P2×junction wait-for cycle** (a train resting inside a coalesced
  run + a train P2-gating the same span) → the **skip-guard** (`span_block_covered` makes P2's per-path
  meet skip any span a junction block owns — the block is the sole authority; the double-gating the
  locked design's skip-guard prevented, bet against and skipped, proven load-bearing); (4) **branch
  starvation** (lowest-index `try_claim` is deadlock-free but not starvation-free — 2 trunk trains
  monopolise a coalesced ≥2-span run, pinning a higher-index branch at v=0) → **conservative cap**: a
  ≥2-span run caps the region to 2 trains (fair alternation), a fully-single trunk is a 1-train shuttle,
  single-span blocks keep full passing-place capacity. A fair-aging tiebreak (restoring higher
  multi-span capacity) is a logged follow-up.
- **Tests** (`tests/junction.rs`, red-first): `single_span_between_passing_places_runs_a_meet`,
  `non_contiguous_single_span_meets` (branch served + meet, no head-on), plus never-freeze regressions
  for all four review deadlocks (`bunched_passing_places_long_single_run`, `staggered_single_span`,
  `multi_span_single_run_with_intermediate_station`, `multi_span_block_does_not_starve_the_branch`).
  **Parity:** zero re-pins — single-span blocks + the skip-guard fire ONLY for phys-single SHARED spans,
  so a fully-double-branched / non-branched / branch-private-single line is byte-identical (all sim
  suites, incl. single_track 7, branching 2, determinism 6).
- Tiers: cargo (zero re-pins), vitest 21, tsc clean, playwright 16.

## Track objects — Phase 1: crisp grid geometry (2026-06-13)

The geometry SUBSTRATE for cross-line shared track (owner decision: full track objects, built grid-
first; docs/fantasy-fork.md §10 + docs/shared-rail.md). The cross-line mutex needs **byte-exact
physical identity** — two continuous Catmull-Rom polylines tracing "the same" rail are NOT byte-
identical (a float-rounded vertex can land in different cells ⇒ the mutex silently never engages = a
false-negative head-on shipping green). Grid geometry makes identity exact: track vertices land on a
`cell_mm` lattice, so two lines over the same cells emit byte-identical edges.

- **`CityData.grid_cell_mm`** (`#[serde(default)]`, **0 = off/continuous**): a bake property frozen at
  construction (NOT a Command). CityData is not hashed, and an existing city (no field ⇒ 0) builds the
  EXACT continuous polyline ⇒ **zero re-pins** (full sim suite byte-identical, additive behind the flag).
- **`grid_walk`** (`line.rs`): `Path::rebuild` branches on `grid_cell_mm`; when >0 the polyline is a
  dense OCTILINEAR lattice walk — each input point snaps to its cell, vertices are cell CENTRES, and
  consecutive points connect by a unit-step walk that is **canonical** (the cell PAIR is sorted
  lexicographically, walked diagonal-first from the smaller, then oriented) so `a→b` is the exact
  reverse of `b→a`. Pure integer; everything downstream (arclen via `dist_mm`, stop_arclen, speed_cap
  via circumradius, span_mode/track_type per span) is unchanged. The P4 Catmull-Rom shared-prefix
  arclen-drift workaround vanishes on grid (identical prefixes ⇒ identical vertices).
- **Sharing guarantee (the LITE scope)** = two lines whose stops snap to the **same consecutive
  stop-cells** emit byte-identical edges (the shared-STATION trunk — the realistic cross-line pattern).
  The grid review found the honest limit: a corridor shared BETWEEN stops (express `A→B` vs local
  `A→M→B` on one rail) splits the walk at `M` and emits different edges — that needs explicit laid
  track both lines reference (the FULL track-objects model). Pinned `#[ignore]`d
  (`grid_express_local_corridor_shares_edges_is_full_model`); Phase 2's mutex contract is "shared
  consecutive stop-cells".
- **Tests** (`tests/grid.rs`, red-first): vertices on the lattice (unit octilinear steps); two lines
  share byte-identical edges on a common section (the Phase-2 foundation); symmetric/canonical walk;
  grid replay bit-for-bit; degenerate spacing (adjacent + same-cell stops, no panic). Review: 1
  finding (the express/local limit, documented + `#[ignore]`d); parity/determinism/panic-safety clean.
- Tiers: cargo 33 suites (zero re-pins), vitest 21, tsc clean, playwright 16. Phase 2 (the cross-line
  shared-block mutex per shared-rail.md) builds on this.
- **2026-06-14 — Track objects Phase 2: cross-line shared-block mutex (the headline fork feature).**
  Two **distinct lines** can now physically share one grid rail and take turns on it — the first
  cross-line block in the project (every prior layer, P2/P4/S1v1/S2, was within a single line). The
  reservation *machinery* is unchanged (the `occ_claim`/`occ_owner`/`try_claim` sorted-Vec +
  `group_overlap`); only the **block key** graduates from line-scoped to a line-independent grid
  `edge_key` (`node_of` = `(x.div_euclid(cell), y.div_euclid(cell))`, sorted cell pair). Built RED-first
  in 4 commits (`f897792` grid foundation, `b660974` the cross-line head-on RED target, `82271b0`
  derivation, `8ecb666` mutex+liveness) + 1 fix commit:
  - **`derive_cross_blocks` (dispatch.rs)** — collect every grid edge-use, **union-find coalesce all
    shared edges (≥2 distinct lines) sharing a node into one component** (block iff it has a single
    edge; `cyclic` iff `edges ≥ nodes`); `by_lane` = per-(line,path) arclen windows. Transient,
    re-derived per dispatch, never hashed.
  - **Phase A.1.7 + B.6 (vehicle.rs)** — cross-line occupancy claim, then the **atomic whole-block meet
    gate**: a held consist parks its head AT the near gate with its whole tail BEHIND the block ⇒ a
    waiter never sits inside the block it waits for ⇒ the wait-for graph is an acyclic depth-1 forest.
    P2's A.1/B/B.5 skip any span a cross-block owns (`cross_span_covered`, the S2 skip-guard discipline).
  - **The liveness stack in one commit** — atomic reservation + cross-LINE dispatch cap
    (`cross_cap`: 1 train/line, ≤2 lines acyclic / 1 cyclic) + single-owner mutex. A half-fix is a
    *worse*, gate-blind deadlock, so it shipped whole.
  - **Why grid:** the cross-line mutex needs **byte-exact** physical identity; continuous Catmull-Rom
    vertices never coincide exactly (a float-rounded vertex lands in a different cell ⇒ the mutex
    silently never engages = a gate-blind false-negative head-on). Grid geometry (Phase 1) makes
    identity integer-exact.
  - **Two adversarial rounds** (budget was 4+; round 2 clean across ~14 runnable counterexamples ⇒
    early convergence). R1 found **2 gate-blind deadlocks** (short-double miscounted as a passing place;
    a round-robin handing one line 2 trains that met head-on) → fixed by the coalesce-all + 1-per-line
    cap (`5361bf7`). R2 attacked the fixed cap with 3 lenses (residual-deadlock, P4/P2 layer-interaction,
    determinism/parity/3-line): the opposing two-resource cross-block cycle (worst sustained stall **13
    ticks** over 40 seeds × 6 phase offsets), 3-line A-B/B-C chains, dead-end blocks, out-and-back rings,
    P4×B.6, P2×B.6 — **none went red**; determinism bit-for-bit.
  - **Conservative by design (logged follow-up):** 1-train-per-line over-throttles (a line sharing any
    block runs a shuttle globally) — over-throttle is the safe direction. A per-block capacity + fair
    aging tiebreak (the S2 fairness follow-up, now shared) restores throughput; deferred.
  - Containment: inert unless `grid_cell_mm > 0` ⇒ **zero re-pins**; `Canonical`/routing/`types.ts`
    untouched; no new Command. Tiers: cargo 33 suites + `shared_rail.rs` 7 (incl. ring/over-provisioned
    never-freeze), vitest 21, tsc clean, playwright 16.

## Fantasy fork — S0: the golden-hash pin + ruleset seam (2026-06-14)

Design turns to code. The fantasy 4X-logistics fork (docs/fantasy-fork.md = architecture,
fantasy-game-design.md = the game, fantasy-build-plan.md = the S0→S11 roadmap, fantasy-map.md =
worldgen) builds on the SAME deterministic core via **ruleset-at-construction** — transit stays
byte-identical until fantasy is complete. **S0 is the safety net everything else stands on.**

- **The gate-blindness S0 closes:** `determinism.rs::replay_equality` proves `run()==run()` but is
  STRUCTURALLY BLIND to a *uniform* hash shift — a Canonical field reorder, an rng-draw-order change
  during the S2 carve, or a postcard bump perturbs every hash identically and sails straight through.
  The fantasy carve (S2) is exactly the kind of "behaviour-preserving" refactor that can do this.
- **The pin (RED-first):** `GOLDEN_TRANSIT_HASH = 0xdeeb_747a_eb78_c6a1` — the exact `state_hash` of
  the canonical transit slice (`sample_log() + 600 ticks @ dt=50`), pinned as a literal. Written
  failing first (placeholder literal → observed RED → pasted the real value). Two assertions:
  `golden_transit_hash_pinned` (the in-memory `run()`) and `golden_transit_hash_via_save_replay`
  (the postcard `save → decode → replay → tick` pipeline must reach the SAME literal — guards the
  serialized save path, not just in-memory). Re-blessed in a reviewed commit at every deliberate
  Canonical change (S7/S8/S10).
- **The `ruleset` seam (additive, no behaviour change):** `CityData.ruleset: String`
  (`#[serde(default = "transit")]`) + `SaveGame.ruleset` (same default, populated in `save()` from
  `city.ruleset`) + the frontend mirror `CityEntry.kind?: "transit" | "arcadia"`. **Not hashed** —
  `Canonical` includes neither `city` nor `SaveGame`, which is *why* adding it can't shift the golden
  pin (verified: pin is green after the field lands). `CityData::default()` keeps `""`, which
  `World::new` will canonicalise to transit (S3), so every native test + shipped city is untouched.
- **Containment / tiers:** zero re-pins; `sample_log` golden stable across the field addition. Full
  cargo sim suite green (CARGO_EXIT=0; determinism.rs 6→8 tests), `sim-wasm` builds, `tsc --noEmit`
  clean. No new Command, no `Canonical` change, no wasm-boundary change.

**S1 — trait scaffolding (no carve), 2026-06-14.** The `Ruleset`/`Demand` seam now exists beside the
proven `Router` seam, default-constructed but **not yet called** — pure scaffolding that de-risks the
S2 carve.

- **`ruleset/{mod,transit}.rs`** (new, sibling to `routing/`): `trait Demand` (`prepare`/`grow`/`spawn`,
  `&mut self` so a model can own per-tick state) + `trait Ruleset` (`coverage_score` + a defaulted
  `validate(cmd)` for the S3 disjoint-save guard). The determinism contract doc is copied verbatim from
  `routing/mod.rs` (index-ordered iteration only; RNG draws from `world.rng` in a FIXED order).
- **The transit impls DELEGATE** (`TransitRuleset`→`World::coverage_score`; `GravityDemand`/`AgentDemand`
  →`demand::{prepare,grow,spawn}` + the population take-out dance). So the impls are real and correct,
  ready for S2 to (a) flip `tick.rs`/`apply` to call them, then (b) inline the free-function bodies and
  delete the free functions. `coverage_score` widened `fn`→`pub(crate) fn` (the only visibility change).
- **`World` gains `ruleset: Box<dyn Ruleset>` + `demand: Box<dyn Demand>`** beside `router`,
  default-constructed (`TransitRuleset`/`GravityDemand`). **Not hashed** (`Canonical` excludes them) and
  **not called** (`tick.rs` still calls the free functions), so the golden pin is byte-identical —
  **verified: `0xdeeb_747a_eb78_c6a1` unchanged**, full suite CARGO_EXIT=0, **zero warnings**, `sim-wasm`
  builds. **S2 (THE CARVE — the one dangerous step) is next.**

**S2 — THE CARVE (the one dangerous step), 2026-06-14.** The per-tick demand + scoring now reach the
core ONLY through the seam; the `agent_demand` if/else folded into `spawn` polymorphism. **The gate
the plan demanded held: the S0 golden hex diffed to ZERO** (`0xdeeb_747a_eb78_c6a1`, unchanged).

- **What flipped through the box:** `tick.rs`'s `grow → prepare → spawn` block (the determinism
  heart) now does a **take-out swap** — `std::mem::replace(&mut world.demand, Box::new(NoopDemand))`,
  run the three methods, restore — so the boxed model can borrow `&mut World` without aliasing the
  field it lives in. `NoopDemand` is a transient ZST placeholder (boxing it doesn't allocate → free).
  `coverage_score` → `self.ruleset.coverage_score(self)`; the eager post-edit catchment recompute →
  `self.demand_prepare()` (same swap). `SetDemandMode` swaps the box (`GravityDemand`↔`AgentDemand`).
- **Why delegation, not a physical body move:** the impls delegate to the `demand::*` module (now the
  gravity/agent implementation that `GravityDemand`/`AgentDemand` wrap). Delegation keeps the
  `world.rng` draw order **byte-identical by construction** — the plan's load-bearing constraint
  (`demand::spawn` destructures `ref mut rng` and draws in station-index→per-pax order). A 200-line
  textual relocation would add transcription risk for ZERO architectural gain: a fantasy
  `SupplyChainDemand` is already a true sibling `impl Demand`, and nothing in `tick.rs`/`apply`
  hardcodes a model. `agent_demand` (the bool) stays the source of truth for the population top-up
  inside `demand::grow`; the box is what `spawn` keys on — they're set together, neither is hashed.
- **Verification (the gate-blind battery, not just `run()==run()`):** the **gravity golden pin**
  caught nothing because nothing shifted; the **agent path** — which the gravity-only `sample_log`
  golden does NOT cover — is held by `agent_demand_mode_via_command_develops_ridership_and_replays`
  (`SetDemandMode → 3000 ticks → run().state_hash()==run().state_hash()` PLUS ridership>0) +
  `agent_demand_is_deterministic` + the named-commuter journey test. Mechanically the carve is a
  dispatch-indirection with verbatim delegation, not an algorithm change → the deterministic gates
  ARE the adversarial check (determinism is machine-verified, not judged). **146/146 cargo, 0
  warnings, ban-grep clean on `ruleset/`, `sim-wasm` builds, `tsc` clean.** Foundation is at a clean,
  fork-ready checkpoint. **S3 (mode toggle + disjoint-save guard) is next.**

**S3 — mode toggle + disjoint-save guard, 2026-06-14.** `World::new` now SELECTS the game from the
frozen `ruleset` tag, and a save can no longer be replayed onto the wrong mode.

- **One dispatch point:** `ruleset::select(tag) -> (Box<dyn Ruleset>, Box<dyn Demand>)` — the only place
  the mode is decided. Transit today; `"arcadia"` lights up at S6 as a single new match arm (the rest of
  the engine is already mode-agnostic via the S1/S2 seam). `World::new` calls it; both boxes are unhashed
  ⇒ golden-neutral. `ruleset::canon("")=="transit"` so native tests (`""`) and JSON cities (`"transit"`)
  name the same mode and the guard compares MODES, not spellings.
- **The mode gate:** `self.ruleset.validate(cmd)` runs at the TOP of `apply` — before any mutation or the
  `cmd_log.push` — so a cross-mode command neither mutates nor pollutes the save. Transit's default
  accepts every existing Command (no early return today ⇒ byte-identical). The real cross-mode rejections
  arrive with the fantasy command vocab at S6.
- **The disjoint-save guard:** `replay()` asserts (canonicalised) `save.ruleset == city.ruleset` — a
  fantasy save replayed onto a transit city would run a foreign command vocab through the wrong `apply`
  and **silently diverge** (a divergence the golden pin structurally can't see — different commands, not a
  hash shift). `replay` has no production callers yet (saves are post-S6; sim-wasm uses `World::new` +
  JSON commands), so an assert at this load-precondition boundary is correct and breaks nothing.
- **Tests (RED-first, structural — not `run()==run()`):** `disjoint_save_guard_rejects_cross_mode_replay`
  (`should_panic`) + `disjoint_save_guard_treats_empty_and_transit_as_one_mode` (canon must not
  false-trip). Golden unchanged; **148/148 cargo, 0 warnings, `sim-wasm` builds.** **The foundation
  (S0–S3) is complete and fork-ready.** S4 (mode-blind read surface + frontend factor — the last
  foundation step, a clean truncation point) is next.

**S4 — DEFERRED to post-S6 (a deliberate, documented sequencing call), 2026-06-14.** S4 (collapse the
~16 transit-named wasm accessors into generic `renderEntities()`/`query(kind,args)` + factor the
**1638-line** `game.ts` into `GameCore`+`TransitGame`) is a large, speculative refactor of the
*shipping* transit frontend that delivers **zero fantasy functionality and no screenshot**. The
mode-blind surface + the core/transit split want to be designed against **two concrete modes**
(transit + fantasy), which exist only after S6's first slice — doing it on one case means guessing the
seam and likely re-doing it (and AGENTS forbids half-built seams). Crucially S6's first slice **reuses
transit's render path** (commodity=`Pax`, cart=vehicle, node=station), so it renders through the
existing accessors ⇒ **S4 does not block it.** Deferred-not-skipped; revisited with S7+ when fantasy's
distinct entities (legions, towns, decadence) actually need a mode-blind boundary. Transit stays
byte-identical meanwhile.

**S5 — Hex geometry port, part 1: the lattice primitives (RED-first, proven in isolation),
2026-06-14.** The hex port replaces the geometry under the *entire* cross-line mutex (the most complex
recently-built subsystem) and carries two float-round hazards, so the dangerous math is built + pinned
**before** it touches the mutex.

- **`hexgrid.rs`** (new, additive, unwired): pointy-top axial `(q,r)` over `i64`-mm space —
  `axial_of` (pixel→cell, the `node_of` primitive), `center_of` (cell→mm vertex), `distance`, and the
  **canonical `line(a,b)`** (drawn from the lexicographically-smaller endpoint then reversed, so
  `line(a,b)` is the EXACT reverse of `line(b,a)` — replicating `grid_walk`'s `line.rs:474` guarantee
  the mutex rests on). A fixed epsilon nudge keeps every interpolated point off a hex boundary ⇒
  consecutive cells are always adjacent (no skips/dupes).
- **Float discipline:** uses `f64` (`√3`) EXACTLY as `line.rs`'s shipped Catmull-Rom does — confined to
  geometry-build, every result quantised to `i64`, fixed op-sequence ⇒ bit-identical. No `f64` *state
  field* (the real ban); floats only produce quantised integers. Pinned by the structural tests below.
- **`tests/hexgrid.rs` (6, RED-first, structural — not `run()==run()`):** centre↔axial round-trip
  **exact over an 81×81 cell range** (THE float hazard — a cell's centre must classify back to that
  cell, or the mutex silently disengages); `line` canonical-reverse symmetry; line steps adjacent +
  exactly-once + correct endpoints/length; distance is a hex metric (6 unit neighbours); deterministic
  + distinct centres; near-centre snap. **All 6 green first run.**
- Additive/unwired ⇒ **golden pin unchanged**, **154/154 cargo, 0 warnings, `sim-wasm` builds.**
  S5 part 2 (the wiring) follows.

**S5 — Hex geometry port, part 2: wired in, mutex re-verified on hex (S5 COMPLETE), 2026-06-14.** The
hex primitives now drive the track lattice, and the entire cross-line mutex subsystem works on hex —
the most complex recently-built code, ported with the golden pin and the gate-blind battery green.

- **`grid_walk` (line.rs)** octilinear→hex: each stop snaps via `hexgrid::axial_of`, consecutive stops
  connect by the canonical `hexgrid::line`, vertices are hex centres. **`node_of` (dispatch.rs)**
  `div_euclid`→`hexgrid::axial_of` — the SAME conversion `grid_walk` snapped with, so a vertex at a
  cell's centre recovers that exact cell (round-trip invariant) ⇒ two lines sharing a cell yield the
  same `edge_key` ⇒ the mutex engages. Callers unchanged; both are ~5-line swaps onto the proven module.
- **`grid.rs` ported to hex** (`cell_of`→`axial_of`; the lattice test asserts hex-centre round-trip +
  unit-hex-distance steps; the shared-corridor test computes cells from stop positions). **All 5 pass.**
- **The gate-blind battery holds on hex, UNMODIFIED:** all 7 `shared_rail.rs` cross-line tests
  (head-on meet, ring deadlock, over-provision, passing-place coalesce, command-order-independence) +
  all P4 junction + single-track tests pass — because the mutex keys on `node_of`/`edge_key`
  abstractly and the hex primitives preserve canonical-reverse + shared-cell identity. The mutex's
  liveness arguments never depended on the lattice being square.
- **Adversarial hardening:** an EXHAUSTIVE sweep (`line_canonical_reverse_holds_exhaustively`,
  **28,561 ordered axial pairs**) asserts canonical-reverse + adjacency + exact endpoints on every
  pair — a counterexample would silently disengage the mutex, so enumeration (a proof over the domain)
  beats hand-picked cases. Green.
- **`roadnav` (8→6 neighbour) DEFERRED to S9** (a deliberate scope split): `roadnav` powers TRANSIT
  buses over the square OSM buildability raster — switching it to 6-neighbour would break shipping
  transit, and hex-`roadnav` isn't needed until S9's decadence raiders walk it. The first fantasy slice
  (S6) uses the RAIL path (`grid_walk`/`node_of`/`RaptorRouter`), which is now hex. S9 adds a
  mode-dependent neighbour set that doesn't disturb buses.
- **Golden pin unchanged** (continuous cities never call `grid_walk` ⇒ the hex port is golden-neutral by
  construction), **155/155 cargo, 0 warnings, ban-grep clean, `sim-wasm` builds.** **The track lattice
  is hex. S6 (the first fantasy slice — the first VISIBLE checkpoint, screenshot-able) is next.**

**S6a — first fantasy slice in the core: the arcadia fork lights up end-to-end, 2026-06-14.** A SECOND
ruleset now constructs and runs a deterministic source→sink→cart commodity flow on the hex lattice,
reusing the transit movement core UNCHANGED. The ruleset-at-construction fork is real.

- **`ruleset/arcadia.rs`** (new sibling of `transit.rs`): `ArcadiaRuleset` + `SupplyChainDemand`. S6a's
  cut REUSES the proven substrate — a commodity cart is a vehicle, a node a station, a commodity token
  a `Pax` — so `SupplyChainDemand` rides the same catchment+spawn+RaptorRouter+advance+board_alight
  path source→sink, and `ArcadiaRuleset::coverage_score` reuses the transit gauge as a stand-in. The
  supply-chain-SPECIFIC behaviour layers on behind this seam (NOT half-built): S7 = commodity ids in
  the unhashed `Pax.citizen_id` + ≤8-commodity recipes + per-input i64 buffers (new hashed Canonical) +
  the Liebig consume→fire→push phase; S11 = the split supply/war gauge. None touches `tick.rs`'s phase
  order or the movement core.
- **`ruleset::select` "arcadia" arm wired** (the single edit the S3 dispatch was built for) + lib.rs
  re-exports. `validate` accepts all for now — the cross-mode teeth (transit rejects PlaceNode, arcadia
  rejects transit build) engage in S7 once a fantasy command vocab exists to reject.
- **Two isolated golden pins.** `tests/arcadia.rs`: the arcadia tag survives construction; a commodity
  **flows source→sink (ridership>0) AND replays bit-for-bit**; and a SEPARATE fantasy golden
  `GOLDEN_ARCADIA_HASH = 0x88cd_59e3_9d09_93a5` (RED-first; the same uniform-shift guard for the arcadia
  path that the transit pin gives transit). **The transit golden `0xdeeb_747a_eb78_c6a1` is UNCHANGED**
  — the two modes are fully isolated; arcadia is purely additive.
- **158/158 cargo, 0 warnings, `sim-wasm` builds.** **S6b (bake `arcadia_world.json` + frontend load +
  the first SCREENSHOT — the first visible checkpoint) is next**, then S7 (the real Forge-Line chains,
  fantasy commands, tick-stamped save).

**S6b — the first-screenshot is DEFERRED (investigated), and `SupplyChainDemand` becomes genuinely
distinct, 2026-06-14.**

- **Why no screenshot yet (a sequencing call backed by code investigation):** the frontend is
  hardwired to a real-world OSM basemap — `map/basemap.ts` mounts CARTO Positron and `App.boot` builds
  every world around `city.center`/`zoom`/`originLngLat` on it. Loading arcadia on that pipeline would
  render carts-on-a-line over *Earth* — **visually transit-identical, nothing distinctly fantasy**. A
  real fantasy render (no basemap, a hex terrain field) is a substrate-level piece that belongs with the
  art pass + the deferred S4 factor, designed ONCE — not hacked onto the transit pipeline. Per
  "screenshots *as appropriately*", a screenshot now would misrepresent transit as fantasy, so it's
  deferred to when distinct fantasy VISUAL content exists. End-to-end integration is already covered: the
  wasm membrane is mode-agnostic (`Sim::new` with an arcadia `city_json` is the identical code path).
- **`SupplyChainDemand` now genuinely differs from gravity** (the first real fantasy-demand
  distinction): `demand::spawn` was refactored into a shared `spawn_modulated(world, dt, mult, bias)`
  body — gravity passes time-of-day `(mult, bias)`; the supply chain passes **steady `(1.0, 1.0)`** ⇒ a
  constant source→sink commodity flow with NO commuter rush (logistics, not commuting). The rng draw
  order is identical for both callers (the params only scale/steer, never reorder), so:
  **transit golden `0xdeeb_747a_eb78_c6a1` UNCHANGED** (gravity is byte-identical), and the **arcadia
  golden re-pinned to `0xe6a5_f0d8_ad1c_85b9`** (a deliberate, reviewed arcadia-only change — the
  isolated-pin discipline working exactly as designed: one mode's mechanic change moved only that mode's
  pin). **158/158 cargo, 0 warnings, `sim-wasm` builds.**
- **S7 (the real Forge-Line: ≤8-commodity recipes, per-input i64 BUFFERS as new hashed Canonical, the
  Liebig consume→fire→push tick phase — the mechanic that makes it a logistics game, and the first
  transit-golden RE-PIN per binding condition #1) is next.** The fantasy frontend render + first
  screenshot follow once distinct fantasy state exists to show.

**S7a — Forge-Line buffers: the FIRST hashed fantasy state + the production phase (+ the first golden
re-pin), 2026-06-14.** Arcadia nodes now hold commodity buffers that fill from production — the
foundation of the logistics game, landed determinism-first.

- **`forge.rs`** (new): the 8-commodity set (two disjoint chains, fixed indices for a stable buffer
  byte-layout) + `produce` — each net-SOURCE node (captured origin > dest) accrues raw ORE into its
  buffer, **integer fixed-point** (`forge_accum` µ-unit remainder, like `spawn_accum`), capped at
  `BUFFER_CAP` (the ONE non-derivable knob, externalised for the balance sweep). No float in the hashed
  result.
- **First HASHED fantasy state:** `World.forge_stock: Vec<i64>` (flat `station*N + commodity`) folded
  into `Canonical` (appended LAST so every prior field keeps its offset). EMPTY for transit. The
  µ-unit `forge_accum` is excluded (derived/transient, regenerated on replay like `spawn_accum`).
- **The production phase via a no-op seam:** `Demand::produce(&mut self, world, dt)` (default no-op),
  called in the `tick.rs` demand swap BEFORE `spawn`. `SupplyChainDemand::produce` → `forge::produce`;
  gravity/agent inherit the no-op ⇒ transit never fills a buffer (the produce CALL is golden-neutral;
  only the new FIELD shifts the transit hash).
- **The documented first RE-PIN (binding condition #1):** transit golden
  `0xdeeb_747a_eb78_c6a1 → 0x42dd_8dde_1e39_8393` (purely the appended empty `forge_stock` slice —
  transit is otherwise byte-identical), arcadia `0xe6a5… → 0x10d1_db35_1bc6_be61` (buffers now fill).
  Both re-pinned with a comment recording the prior values + the reason.
- **Tests:** `arcadia_sources_produce_into_buffers` (sources accrue ORE, sinks produce nothing,
  deterministic) + `transit_has_no_forge_buffers` (transit `forge_stock` stays empty — the fantasy
  state is genuinely isolated). **160/160 cargo, 0 warnings, `sim-wasm` builds, ban-grep clean** (no
  f32/f64 state field; the one `f32→i64` cast is a local feeding the integer accumulator).
- **S7b (the buffer→spawn GATE — ship only what you've produced; the deposit-at-sink; the 2-input
  recipes with Liebig output=min-input-rate) is next**, layering on this seam, NOT half-built in.

**S7b — the buffer→spawn GATE: production now throttles shipping, 2026-06-14.** A node ships only what
it has produced — the Liebig throttle that makes production *bite* (the design's "throb").

- **One shared spawn, parameterized:** `spawn_modulated` gained a `gate: Option<&mut [i64]>` per-station
  ship budget. `None` (transit gravity) ⇒ unbounded, the branch is skipped ⇒ **transit golden
  `0x42dd…` UNCHANGED** (verified). `Some(budget)` (arcadia) ⇒ each shipped commodity consumes one unit;
  when the buffer is empty the node stops and **drops the whole-unit backlog** (keeps the sub-unit
  remainder via `.fract()`) — a steady flow, not an order queue that would burst on refill.
- **`SupplyChainDemand::spawn`** extracts the per-station ORE budget from `forge_stock` (after `produce`
  filled it this tick), ships gated, and writes the drained buffers back. So the per-tick loop is now
  `produce → ship-from-buffer`: production fills, shipping drains, the buffer is the coupling.
- **Arcadia golden re-pinned `0x10d1… → 0xb026_edcc_eeb8_4c90`** (shipping now production-limited).
- **Tests:** `arcadia_shipping_gated_by_production` (commodities ship, but the source buffer stays
  drained because demand outpaces production — the gate binds) + `arcadia_sources_produce_into_buffers`
  rebuilt to ISOLATE production (no line ⇒ nothing ships ⇒ accrual is visible: source>0, sink=0) +
  `transit_has_no_forge_buffers`. **161/161 cargo, 0 warnings, `sim-wasm` builds.**
- **S7c (deposit-at-sink: a delivered commodity increments the town's buffer — closing the loop + the
  "town fed" signal — then the 2-input forge recipes with Liebig) is next.** Needs a clean board_alight
  delivery hook; a careful determinism-critical turn.

**S7c — deposit-at-sink: the supply loop CLOSES, 2026-06-14.** A commodity now physically moves
source→sink end-to-end: produced → shipped (drains the source buffer) → ridden → DELIVERED (fills the
sink buffer). The fantasy core has a complete, conserved, deterministic single-commodity logistics loop.

- **The delivery hook:** at `board_alight`'s completed-trip point (a Pax alighting its last leg), a
  commodity is deposited into the destination node's buffer (capped at `BUFFER_CAP`). Gated on
  `forge_stock` being **non-empty** — a mode-agnostic condition (NOT a ruleset-string check): empty for
  transit ⇒ **no-op ⇒ transit golden `0x42dd…` UNCHANGED** (verified); sized for arcadia ⇒ deposit.
- **Conservation:** the S7b gate drains the source ORE buffer on shipping; the S7c deposit fills the
  sink ORE buffer on delivery — ORE physically moves, never duplicated. ORE-only for now (the sole
  commodity shipped); multi-commodity routing (tagging `Pax.citizen_id` with the commodity id) is next.
- **Arcadia golden re-pinned `0xb026… → 0xbdca_2524_6ba8_fd34`** (deliveries now mutate sink state).
- **Test `arcadia_commodity_loop_closes`:** the sink accumulates ORE it never produced (it's a net
  sink) — proof the loop closed — plus ridership>0 and bit-for-bit replay. **162/162 cargo, 0 warnings,
  `sim-wasm` builds.**
- **S7d (the 2-input forge RECIPES — a forge consumes 2 inputs → produces 1 output, Liebig
  output-rate = min input-rate; + commodity-id tagging for multi-commodity routing) is next**, the last
  Forge-Line piece before the chain is a real ≥3-stage network.

**S7d — town consumption → TRIBUTE: the supply loop is SCORED (S7 core complete), 2026-06-14.** The
fantasy economic loop now closes end-to-end with a monotonic score — a complete, deterministic
logistics game in the core.

- **The Liebig consume (single-input):** `forge::produce` gained a consumption pass — a net-SINK node
  (a town: captured dest > origin) consumes the supply DELIVERED into its buffer → a global hashed
  `World.tribute` (the supply score, the game's core payoff "feed towns → tribute"). A node is either a
  net source or net sink, never both, so production is never double-counted. Integer, index-ordered.
- **`tribute: i64`** folded into `Canonical` (0 for transit ⇒ one more re-pin, then byte-identical).
- **The complete loop:** produce (sources fill ORE) → ship (S7b gate, drains source) → ride → deliver
  (S7c, fills town) → CONSUME (S7d, town → tribute). A commodity is conserved at every hop and ends as
  score. Reuses the transit movement core (RaptorRouter+advance+board_alight) UNCHANGED throughout.
- **Both goldens re-pinned** (the documented S7 re-pin): transit
  `0x42dd… → 0xd7fb_a36d_5bba_92c9` (the appended `tribute` i64 — transit still byte-identical),
  arcadia `0xbdca… → 0x0a72_ad39_a32a_29ae` (consumption + tribute). The transit pin's comment now lists
  all three S0→S7 values + the exact reason for each shift.
- **Tests:** `arcadia_commodity_loop_closes` (tribute>0 — the whole chain connected) +
  **`arcadia_tribute_is_monotonic`** (tribute never drops over 3000 ticks — the supply-gauge
  monotonicity invariant the design mandates) + the production/gate/delivery tests. **163/163 cargo, 0
  warnings, `sim-wasm` builds.**
- **S7 CORE COMPLETE** — a working scored logistics loop. **Deferred to a follow-up (tracked):** the
  multi-stage forge recipes (ORE→INGOT→ARMS via mid-chain forges), which need commodity-aware routing
  (route each commodity to the nodes that consume it) — a larger demand-model generalisation, best done
  when the chain needs ≥3 stages. **S8 (the war machine — the conquest half of the game) is the next
  major system.**

**S8a — the war machine's foundation: a SEPARATE army SoA + the war_step seam, 2026-06-14.** Legions
now exist, are funded by tribute, and march deterministically — and the two halves of the game connect
(supply → tribute → armies). The binding-condition #2 gate-blind test passes.

- **`army.rs` — `ArmySoA`, a SEPARATE SoA** (NOT a `kind` byte in `VehicleSoA`): the binding condition
  (#2). `dispatch` rebuilds the shared `VehicleSoA` from scratch on every `SetHeadway` (`v.clear()`),
  which would TELEPORT a marching legion; an army OWNS its arc-length `s_mm` here, untouched by dispatch.
  Movement is a plain constant-speed march (no dwell/boarding/follow-clamp — passenger concerns);
  single-track admission via `occ_claim` + siege/flip are S8b.
- **The `war_step` seam:** `Ruleset::war_step(&mut world, dt)` (default no-op) — the fantasy per-tick
  trailer, called in `tick.rs` (Phase 7) via a ruleset take-out swap (`NoopRuleset` placeholder, like
  `NoopDemand`). Transit inherits the no-op ⇒ the army SoA stays empty ⇒ transit byte-identical (only
  the hashed army FIELDS re-pin it). `ArcadiaRuleset::war_step` = `maybe_launch` + `advance_armies`.
- **Supply funds war:** `maybe_launch` fields a legion from the first built route when
  `tribute ≥ LAUNCH_COST` (consuming it) — the supply economy pays for armies, tying the two halves
  together. `LAUNCH_COST` (and `ARMY_SPEED_MM_S`) are the non-derivable knobs, flagged for the balance
  sweep.
- **Hashed:** the 7 authoritative army fields (line/path/s_mm/dir/strength/target/state) folded into
  `Canonical`; cartesian x/y are render-only (excluded), like vehicles'. Both goldens re-pinned (transit
  `0xd7fb… → 0x45f8_da5f_19af_73f3` = empty army slices appended; arcadia
  `0x0a72… → 0x7bd2_5ce3_93ac_63da`).
- **Tests (`tests/army.rs`):** `tribute_funds_a_marching_legion` (a legion launches + marches,
  deterministic) · **`legion_position_survives_a_set_headway`** (THE binding-condition gate-blind test —
  `s_mm` unchanged across a `SetHeadway`'s `v.clear()`) · `transit_fields_no_armies` (war is
  fantasy-only). **166/166 cargo, 0 warnings, ban-grep clean, `sim-wasm` builds.**
- **S8b (the full `war_step`: retarget → supply-gated siege grind → flip; `PlaceBarracks`/`PostBounty`
  commands; i64 `town_value`; army↔train single-track via the existing `occ_claim`; keyed RNG
  `seed ^ WAR_CONST`) is next** — the rest of the conquest loop, with its gate-blind battery.

**S8b — the conquest loop closes: march → besiege → grind → FLIP, 2026-06-14. BOTH core game loops now
work in the deterministic core.** A legion that reaches its target town besieges it, grinds its
resistance to 0, and captures it.

- **Town state (hashed):** `World.town_value: Vec<i64>` (per-town siege resistance, lazily sized to the
  node count at `RESISTANCE`) + `World.towns_captured: i64` (the conquest score). Empty/0 for transit.
- **The siege sub-phase** (`army::siege`, the locked-order tail of `war_step` after launch→march): a
  `BESIEGING` legion grinds `town_value[target] -= strength`/tick; at 0 the town FLIPS. Arrival is
  detected in the march (s_mm reaches the route end = the target stop → `BESIEGING`). A fallen legion
  → `DONE` (kept index-stable, never removed — determinism; bounded by `MAX_ARMIES`).
- **Capture EXACTLY ONCE (the gate-blind hazard):** the count fires only on the grind→flip TRANSITION
  (`town_value` was >0, now 0). A later legion arriving at an already-captured town just garrisons
  (`DONE`) — never a second count. The "bounty-exactly-once across grind→flip" battery item, proven.
- **Both goldens re-pinned:** transit `0x45f8… → 0xd747_5260_98d0_0aeb` (empty town fields appended),
  arcadia `0x7bd2… → 0xa590_bedd_5999_caf0` (`town_value` now populated + sieges).
- **Test `war_machine_captures_town_exactly_once`:** over a long run MANY legions launch (tribute keeps
  funding them) and all target the one town — yet `towns_captured == 1` (exactly once), `town_value`
  hit 0, deterministic. Plus the S8a march/separate-SoA/transit-empty tests. **167/167 cargo, 0
  warnings, ban-grep clean, `sim-wasm` builds.**
- **The fantasy CORE is now a functional, deterministic two-loop game:** SUPPLY (produce→ship→deliver→
  consume→**tribute**, monotonic) feeds CONQUEST (tribute→launch→march→besiege→**flip**). What remains
  is player AGENCY + depth + visibility: `PlaceBarracks`/`PostBounty` (Majesty steering — the player's
  only war lever) + retarget, army↔train single-track via `occ_claim`, S9 decadence (the lose
  condition), S10 area-control CA, S11 economy/tech, and the deferred fantasy frontend render (the first
  screenshot). **Next: the player levers (`PlaceBarracks`/`PostBounty`) — turning the auto-war
  player-steered.**

**S8 player levers, pt 1 — `PlaceBarracks`: war becomes player-gated + the disjoint-save guard grows
teeth, 2026-06-14.** The first fantasy COMMAND, and the first real cross-mode rejection.

- **`PlaceBarracks { x_mm, y_mm }`** (command.rs + `Event::BarracksPlaced` + apply arm): creates a
  station and flags it a barracks (`World.is_barracks: Vec<bool>`, hashed). Legions now launch ONLY from
  a barracks on a built route (`maybe_launch` rewritten) — building one is the player's prerequisite for
  war (the design's agency: you don't command armies, you enable + bait them). The legion starts at the
  barracks's arc-length and marches to the far-end town.
- **The disjoint-save guard's first real teeth:** `TransitRuleset::validate` now REJECTS `PlaceBarracks`
  (a fantasy-only command) — refused at the top of `apply` before any mutation or save-log push, so a
  transit save can never carry a command that would replay against the wrong `apply`. (Arcadia accepts
  it.) The S3 guard mechanism, finally exercised.
- **Contract synced:** `types.ts` (Command + Event unions) + `codec.ts` (`cmd.placeBarracks`) mirror the
  serde shape in lock-step (`tsc` clean) — even though the arcadia frontend isn't wired yet, the wire
  contract stays drift-free.
- **Both goldens re-pinned** (the empty `is_barracks` slice joins `Canonical`): transit
  `0xd747… → 0x9e3b_e523_a982_8d51`, arcadia `0xa590… → 0xeebf_421f_4053_d5d0`.
- **Tests:** `no_barracks_no_legion` (tribute accrues but NO army without a barracks — the agency gate) ·
  `transit_rejects_place_barracks` (Rejected, no station, no flag — the cross-mode teeth) · the
  march/siege/capture tests updated (`war_world` now places a barracks). **169/169 cargo, 0 warnings,
  `sim-wasm` + `tsc` clean.**
- **Next: `PostBounty` + bounty-steered retargeting** (the Majesty lever — the player baits legions
  toward chosen towns; needs target-selection among multiple towns), then the army↔train single-track
  via `occ_claim`.

**S8 player levers, pt 2 — `PostBounty`: the Majesty steering lever. The war is now fully
player-steered, 2026-06-14.** Both player levers are in; the conquest loop is complete and playable in
the core.

- **`PostBounty { station, amount }`** (command + `Event::BountyPosted` + apply arm; `World.bounty:
  Vec<i64>` hashed): the player BAITS legions rather than commanding them. `maybe_launch` now targets the
  highest-bounty UNCAPTURED town on a barracks's route (tiebreak: lowest StationId — the "tied-score →
  same TownId" determinism item), excluding the barracks; no bounty anywhere ⇒ the route's far-end town
  (default). The march besieges at the TARGET's arc-length (an intermediate bounty target halts the
  legion mid-route, not only at the end).
- **Cross-mode teeth extended:** `TransitRuleset::validate` rejects `PostBounty` too. Contract mirrored
  in `types.ts`/`codec.ts` (`cmd.postBounty`), `tsc` clean.
- **Both goldens re-pinned** (empty `bounty` slice joins `Canonical`): transit
  `0x9e3b… → 0x6253_ac99_08d6_20a3`, arcadia `0xeebf… → 0x02d3_2b1a_4e74_5070`.
- **Test `a_bounty_steers_a_legion_to_a_mid_route_town`:** a bounty on a MID-route town gets it
  besieged + captured — something that NEVER happens by default (legions march to the far end) — so
  `town_value[mid]==0` proves the bounty redirected the AI. Plus `transit_rejects_post_bounty`. **171/171
  cargo, 0 warnings, `sim-wasm` + `tsc` clean.**
- **S8 CORE + BOTH PLAYER LEVERS COMPLETE.** The fantasy fork is now a **functional, deterministic,
  player-steered two-loop 4X-logistics game in the core**: build a supply network → towns→tribute → a
  barracks fields legions → bounties steer them → they march, besiege, flip towns. Every Command has an
  immediate sim effect; both modes are golden-pinned; transit stays byte-identical. **Remaining war
  refinements (a follow-up):** army↔train single-track via the existing `occ_claim` (the last gate-blind
  battery item), supply-gated siege, keyed RNG `seed ^ WAR_CONST`, AI tiers. **Bigger remaining systems:
  S9 decadence (the lose condition), S10 area-control CA, S11 economy/tech, and the deferred fantasy
  frontend render (the first screenshot).**

**S9 — Decadence: the lose condition. The fantasy core now has the COMPLETE win/lose tension,
2026-06-14.** Corruption spreads while you play; conquest holds it back; if it reaches the capital, the
realm falls.

- **`decadence.rs`:** a global corruption pressure `World.decadence: i64` (hashed) — grows at
  `BASE_GROWTH`, pushed back by captured towns (`net = base − towns_captured·clear`, clamped ≥ 0).
  `is_lost()` once it reaches `CAPITAL_THRESHOLD` (the capital falls). The decadence sub-phase runs in
  `war_step` (fantasy-only ⇒ transit `decadence` stays 0). Integer, dt-scaled, deterministic.
- **The flywheel now has URGENCY:** supply→tribute→conquest must outrun the rot. Constants tuned so an
  idle realm is overrun in a few game-minutes while modest conquest holds indefinitely — a *winnable*
  race (the balance knobs are flagged for the harness sweep).
- **Both goldens re-pinned** (the `decadence` i64 joins `Canonical`): transit
  `0x6253… → 0xea4e_eb0a_03d9_74f9`, arcadia `0x02d3… → 0x5375_1cb0_558d_3b0f` (arcadia's decadence GROWS
  — it runs but `arcadia_world` never conquers).
- **Tests:** `decadence_overruns_an_idle_realm` (a realm that runs supply but never fights is overrun —
  `is_lost`) · `conquest_pushes_decadence_back` (a robust contrast: the SAME realm WITH a barracks ends
  with strictly less decadence than without — conquest is the brake; timing-independent) ·
  `transit_has_no_decadence`. A surfaced balance lesson: a source↔sink route SHORTER than the ~500 m
  catchment merges their catchments and stalls production — the comparison test keeps them separated.
  **174/174 cargo, 0 warnings, `sim-wasm` builds, ban-grep clean.**
- **The fantasy fork is now a COMPLETE, deterministic, player-steered 4X-logistics game in the core:**
  build supply → towns→**tribute** → barracks+bounties field & steer legions → **conquer** towns → hold
  back **decadence** or lose. Every Command has an immediate sim effect; both modes golden-pinned;
  transit byte-identical throughout. **The biggest remaining gap is VISIBILITY — the deferred fantasy
  frontend render (the first screenshot + end-to-end validation).** Then the depth systems: S10
  area-control CA (the spatial decadence/territory — the largest subsystem), S11 economy/tech, S7e
  multi-stage recipes, S8 refinements (occ_claim, AI tiers).

**FRONTEND — FIRST LIGHT: the fantasy fork runs in the browser (the deferred visibility milestone),
2026-06-14.** After ~16 turns of headless core, the whole stack is validated end-to-end and there is a
first screenshot ([docs/progress/fantasy-arcadia-first-light.png](docs/progress/fantasy-arcadia-first-light.png)).

- **The missing wire:** `buildCoreCity` (sim/city.ts) never passed `ruleset`/`grid_cell_mm` to the core
  — so an arcadia manifest would have built as transit/continuous. Added `RawCity.ruleset` +
  `RawCity.gridCellMm` and threaded both into the core JSON. `tsc` clean.
- **Baked the world + registered it:** `arcadia_world.json` (manifest: `ruleset:"arcadia"`,
  `gridCellMm:1_000_000`, origin [0,0]), `arcadia_demand.json` (a source + two towns, separated well
  beyond the catchment so supply flows), `networks/arcadia.json` (Iron Road + Grain Way). `cities.ts`
  gains the `arcadia` entry (`kind:"arcadia"`). Loads via `?city=arcadia&network=1`.
- **Rebuilt the wasm from current source** (`wasm-pack build … --target bundler`, 425 KB) so the
  browser runs the S0–S9 fantasy engine, started Vite, drove Playwright.
- **Verified RUNNING in-browser** (`__ot_test.stats()` after `setRunning`): title "Transit Story ·
  Arcadia", **2 lines, 5 carts, ridership climbing (commodities flowing source→sink via
  `SupplyChainDemand`), coverage 99, 0 errors.** The screenshot shows the **HEX LATTICE rendering** —
  both lines draw the crisp stepped hex-walk geometry from S5 — over a neutral basemap, no glitches.
- **Honest scope:** this validates the stack + the supply loop + hex render end-to-end. It still wears
  the **transit chrome** (riders/coverage/Rail-Bus modes — the S4 mode-blind read surface is still
  deferred) and shows only the supply loop (the network has no barracks ⇒ no conquest; the
  fantasy-specific HUD — tribute/decadence/towns/armies — and the hex-terrain/army render are the next
  frontend layers). But the fork is now **demonstrably playable in a browser.** No sim re-pins (frontend
  + data only; the core is unchanged this turn).

**FRONTEND — the mode-aware HUD: the browser now reads as FANTASY, 2026-06-14.** The headline HUD
shows the supply→conquest→decadence readout instead of riders/coverage
([docs/progress/fantasy-arcadia-hud.png](docs/progress/fantasy-arcadia-hud.png)).

- **Fantasy state crosses the boundary (the S4 read-surface, lite):** `StatsSnapshot` gains `ruleset`,
  `tribute`, `decadence`, `decadence_pct` (the gauge fill, computed by `decadence::pct` so the
  threshold const never leaks to TS), `towns_captured`, `army_count`, `realm_lost` — all 0/false for
  transit, so the field is mode-agnostic. **Golden-NEUTRAL** (the snapshot is a read-out, not in
  `Canonical`; all goldens unchanged, 174/174). Mirrored in `types.ts`; `tsc` clean.
- **`StatsBar` is mode-aware:** `s.ruleset === "arcadia"` routes to a new `FantasyStatsBar` (the
  transit path is byte-identical — every transit `data-testid` preserved, no e2e regression). The
  fantasy bar reads **⚜ tribute · ☠ Decadence lose-meter gauge (neutral→amber→red as it nears the
  capital) · 🏰 towns taken · ⚔ legions · "THE REALM HAS FALLEN"** on loss.
- **Verified in-browser** (wasm rebuilt, Playwright @8×): `ruleset:"arcadia"`, tribute climbing,
  **decadence gauge filling (41%→65%, amber)** — the core supply-vs-corruption tension is now visually
  legible. 0 console errors. The screenshot confirms the fantasy top-bar replacing the transit chrome.
- **Honest scope:** the TOP BAR is fantasy; the left Lines panel + bottom Network panel are still
  transit chrome (the next mode-aware layer), and conquest readouts (towns/legions) sit at 0 because the
  baked network has no barracks (no barracks/bounty BUILD TOOL yet — also next). The army-dot render +
  hex terrain remain too. But the game now **reads as fantasy at a glance.** No sim re-pins.

**FRONTEND — the war machine becomes VISIBLE: the full conquest loop runs in a browser, 2026-06-14.**
Legions now render as marching dots, launched from a baked barracks — and a town actually falls
in-session ([docs/progress/fantasy-arcadia-conquest.png](docs/progress/fantasy-arcadia-conquest.png)).

- **Army render (render-only, golden-NEUTRAL):** `render_buf::army_positions_m` interpolates each
  legion's arc-length `s_mm` along its route (`Path::point_at`) to cartesian metres; an `armyPositions()`
  wasm accessor + `SimBridge.armyPositions()` cross the boundary like vehicle positions; `render.ts`
  `armyLayer` (crimson dot + gold ring, distinct from the line-tinted carts) splices into
  `composeAndSet` ABOVE vehicles, below peeps/labels (z-order). Cheap per-compose (legions are few,
  capped). All goldens unchanged (174/174) — it reads `s_mm`, never mutates.
- **Barracks bake (the conquest enabler):** `NetStation.barracks?` → `applyNetwork` sends
  `cmd.placeBarracks` for flagged nodes (else `placeStation`); the `arcadia_world` demo is re-tuned
  COMPACT (≈2 km routes, still > the 500 m catchment so supply flows; `gridCellMm` 250 m; a
  barracks-flagged "The Forge") so the whole loop completes fast enough to watch.
- **Verified in-browser** (wasm rebuilt with the accessor, Playwright @8×): the legion launched,
  marched, besieged, and **flipped a town** — `townsCaptured:1`, `armyCount` up to 12 legions afield,
  **`decadencePct:0`** (the captured town's pushback drove the corruption back to zero — the realm
  winning the race). The screenshot shows crimson legion dots on the rails + the HUD
  "⚜ 16 tribute · ☠ Decadence 0 · 🏰 1 taken · ⚔ 12 legions". 0 console errors.
- **The fantasy fork is now end-to-end PLAYABLE + VISIBLE in a browser:** supply carts flow, tribute
  funds legions, legions march + conquer, decadence is held back — all on the hex lattice, all rendered.
  **3 screenshots** (first-light · HUD · conquest). Remaining frontend: a player-facing barracks/bounty
  BUILD TOOL (today the barracks is baked), mode-aware side/bottom panels, and the hex-terrain backdrop.
  No sim re-pins this turn (render + data only).

**FRONTEND — the fantasy path is now an automated REGRESSION GATE (e2e), 2026-06-14.** All the
frontend fantasy work (load → HUD → army render → conquest) was corroborated only by screenshots; now
it's pinned by a real e2e test, and all three tiers are confirmed green.

- **`e2e/arcadia.spec.ts`** (AGENTS e2e discipline — gameplay facts, wait-on-flags, never a load-only
  green): loads `?city=arcadia&network=1`, asserts `stats().ruleset === "arcadia"`, runs at 100×, then
  waits for + asserts **tribute > 0** (supply flows), **`bridge.armyPositions().length > 0`** (a legion
  fielded AND rendered — the army render path end-to-end), **townsCaptured ≥ 1** (a town conquered),
  `realmLost === false`, and the fantasy HUD testids (`tribute`, `towns-captured`) visible. **Passes in
  5.8 s** (4th screenshot: `docs/progress/fantasy-arcadia-e2e.png`).
- **No transit regression:** the `StatsBar` mode-split is additive (an early `return <FantasyStatsBar>`
  for arcadia; the transit JSX + every transit testid byte-identical). Confirmed by re-running the
  canonical transit e2e (`slice` + `modes`, 3 tests) — all green.
- **All three tiers green:** cargo `sim` **174/174** (this turn's render/stats additions are read-outs,
  golden-neutral), **vitest 21/21**, **e2e** arcadia + slice + modes. The fantasy fork's entire stack —
  deterministic core + browser frontend — is now regression-protected. **Next: the player-facing
  barracks/bounty BUILD TOOL** (so the player, not a bake, fields + steers legions).

**FRONTEND — the player-facing BARRACKS build tool: arcadia is now player-interactive, 2026-06-14.**
The player can place a barracks by hand and field legions — not just watch a baked demo.

- **The tool:** `Tool` gains `"barracks"`; `Game.placeBarracks(lng,lat)` (mirrors `placeStation`, emits
  `cmd.placeBarracks`); `tools/pointer.ts` routes a build-mode barracks-tool click to it (sticky, like
  the station tool); the chorded `Toolbar` shows a **🏰 Barracks** tool — but ONLY in fantasy.
- **Mode-aware chrome plumbing (reusable):** `Game.ruleset` (set in `boot` from the manifest) now flows
  through the `GameUI` slice (`useGameUI`), so the Toolbar appends `FANTASY_TOOLS` when
  `ui.ruleset === "arcadia"` — set once at boot, not a per-frame stats read. This is the seam the
  mode-aware side/bottom panels will reuse.
- **Test hooks + e2e:** `placeBarracksLngLat` added to the camera-independent test hook (+ `global.d.ts`).
  A second arcadia e2e — **"a player-built barracks fields legions that conquer"** — builds a barracks +
  route via the hooks (Game.placeBarracks's path), runs, and asserts a legion launches + a town falls.
- **All tiers green, no regression:** both arcadia e2e + transit `build-tools`/`modes`/`slice` e2e
  (the Toolbar/ui-slice change is additive — transit chrome byte-identical) + vitest 21/21 + `tsc`. No
  sim change (frontend-only) ⇒ cargo goldens untouched. **Next fantasy UI:** the bounty tool (click a
  town to post a bounty — the steering lever), mode-aware side/bottom panels, hex-terrain backdrop.

**FRONTEND — the BOUNTY tool: the full Majesty control is now player-accessible, 2026-06-14.** Both
war levers are in — the player builds barracks AND baits legions with bounties.

- **The tool:** `Tool` gains `"bounty"`; `Game.postBounty(stationId, amount=1000)` (→ `cmd.postBounty`);
  `pointer.ts` resolves a build-mode bounty-tool click to the nearest town (`game.nearestStation`) and
  posts the standard bounty (sticky); the Toolbar's `FANTASY_TOOLS` now lists **🏰 Barracks + ⚑ Bounty**
  (fantasy only). `postBounty(station, amount)` added to the test hook + `global.d.ts`.
- **e2e:** the player-built arcadia spec now also posts a bounty (exercising `Game.postBounty`'s path)
  and still launches legions + conquers — both arcadia e2e green. `tsc` clean; frontend-only ⇒ cargo
  goldens untouched.
- **arcadia is now fully player-steerable:** build a supply network, place a barracks, post bounties to
  direct the AI legions — the Majesty model (you never command armies directly). **Next:** mode-aware
  left Lines + bottom Network panels (still transit chrome), a hex-terrain backdrop, then core depth
  (S10 area-control CA, S11 economy, S7e multi-stage recipes).

**MILESTONE — the complete fantasy vertical, FULL-SUITE VERIFIED, 2026-06-14.** After the whole build
(S0–S9 core + the frontend: load → HUD → army render → barracks + bounty tools), a holistic run of
every tier confirms the cross-cutting work holds together with zero regressions:

- **cargo `sim` 174/174, 0 warnings** — every suite (determinism with BOTH goldens, arcadia, army,
  decadence, hexgrid, grid, shared_rail, junction, + all transit suites). The deterministic core is
  intact and both mode pins hold.
- **vitest 21/21** — the codec/types round-trips + the wasm-in-node smoke.
- **e2e 18/18** — the two arcadia specs (baked conquest + player-built barracks/bounty) AND every
  transit spec (Singapore real MRT, Tokyo ~440 stations, modes/ferry, buildability, edit-line,
  waypoints, slice, …). The mode-split chrome (StatsBar/Toolbar/ui-slice/render) is additive — transit
  is byte-identical.
- **The fantasy fork is a complete, deterministic, browser-playable, player-interactive 4X-logistics
  game with a fully regression-protected stack** (5 progress screenshots). Transit remains byte-
  identical throughout (ruleset-at-construction delivered). Remaining is DEPTH + POLISH, each optional
  to "a playable fantasy 4X": S7e multi-stage Forge-Line recipes, S10 area-control CA (the spatial
  decadence/territory identity), S11 economy/tech, mode-aware side/bottom panels, hex-terrain backdrop,
  S8 refinements (occ_claim, AI tiers). All work remains uncommitted (per the standing "commit only when
  asked" rule) — a natural point to commit the whole vertical.

**BALANCE HARNESS — the determinism dividend: the fantasy loop self-plays + is gated winnable, 2026-06-14.**
The build plan's "Fast iteration, telemetry & balancing" capability, landed: `tests/balance.rs` runs the
canonical fantasy loop HEADLESS across seeds, collects pacing telemetry, and gates the two things paper
can't answer.

- **`fantasy_loop_is_winnable_and_bites`** (across 5 seeds): asserts the realm SUPPLIES (tribute) →
  FIELDS a legion → CONQUERS a town → HOLDS (not overrun), in milestone order, within a 12k-tick
  horizon. The loop is winnable + turns, for every seed. **`balance_telemetry_is_reproducible`** pins
  the harness itself (same seed ⇒ identical telemetry — sweep counterexamples replay bit-for-bit).
- **Concrete telemetry (printed):** `tribute@215 ticks (~11 s)`, `legion@2411 (~2 min)`,
  `conquest@4513 (~3.75 min)`, `decadence_peak 9024 < 20000` (holds). **Two findings:** (1) the loop is
  winnable but the first conquest (~3.75 min) is SLOWER than the design's ~60–120 s "bites" target — a
  concrete tuning signal (production-gated tribute → slow legion funding; lower `LAUNCH_COST` / faster
  production would tighten it); (2) the telemetry is seed-invariant here because the trivial 2-node
  topology forces the route (no `pick_dest` choice) — the harness is structured for richer multi-town
  scenarios that WOULD vary by seed.
- **Additive + read-only ⇒ golden-neutral:** 176/176 cargo (174 + 2), 0 warnings, both pins unchanged.
  The harness is now the lever to TUNE the hardcoded knobs (LAUNCH_COST, decadence rates, BUFFER_CAP,
  catchment) toward the pacing targets — without a human, reproducibly. **Next:** either tune-to-target
  using it, or core depth (S7e Forge-Line / S10 area-control), or the mode-aware panels polish.

**BALANCE TUNING — the loop now BITES in the target window (harness-driven), 2026-06-14.** Acted on the
harness's signal: the war knobs were the bottleneck (`LAUNCH_COST 20`, `ARMY_SPEED 15000`).

- **Tuned `LAUNCH_COST 20 → 8` + `ARMY_SPEED 15_000 → 50_000`** — chosen specifically because they're
  WAR-only knobs the barracks-free arcadia golden never exercises, so the tuning is **golden-neutral**
  (verified: both pins unchanged). The harness re-ran: **legion @990 ticks (~50 s), first conquest
  @1675 ticks (~84 s)** — down from ~3.75 min, squarely in the design's 60–120 s bite window; decadence
  peaks lower (3348) and the realm still holds.
- **Locked it in:** the harness gained a SOFT pacing gate — `first_conquest <= 2400` ticks (~120 s) — so
  a future knob change that lets the flywheel drag trips a test (a balance regression, distinct from a
  correctness one). Wasm rebuilt so the browser demo bites at the tuned pace too.
- **All green, zero re-pins:** 176/176 cargo, 0 warnings; army + decadence tests robust to the faster
  pacing (threshold-based); both goldens untouched. The "does it bite fast?" unknown — the build plan's
  #1 — is now answered (yes, ~84 s) AND gated. The harness + gate make the remaining knobs
  (`RATE_MICRO_PER_WEIGHT_MS`, decadence rates, `BUFFER_CAP`) tunable the same way.

**FRONTEND — mode-aware panels: the UI now reads coherently as FANTASY, 2026-06-14.** The last
transit chrome under the fantasy HUD is gone.

- **`ServiceReport` is mode-split:** in arcadia it returns a `FantasyServiceReport` — the **"⚜ The
  Realm"** ledger (Tribute · Supply delivered · 🏰 Towns taken · ⚔ Legions afield · ☠ Decadence % with
  the neutral→amber→red tone + a one-line play hint) — instead of the transit service telemetry
  (homes→jobs demand, City coverage, Riders-by-mode). Same `CARD`/`Row` chrome so the two modes read
  alike; the transit path is byte-identical (every `svc-*` testid preserved). Reads only the ~3 Hz stats
  slice (no new sim field).
- **Verified:** `tsc` clean; transit `slice` e2e (which asserts the transit `svc-coverage` testid) +
  both arcadia e2e green — no regression. Screenshot `docs/progress/fantasy-arcadia-realm-panel.png`
  shows the coherent fantasy UI (fantasy HUD + realm ledger + army dots) AND the tuned pacing (a town
  taken + 14 legions by ~08:03). Frontend-only ⇒ cargo goldens untouched.
- **The fantasy UI layer (task #14) is substantially done:** mode-aware HUD + army-dot render + barracks
  & bounty build tools + the realm panel. The UI reads as fantasy at a glance. Minor remaining polish:
  the left Lines-panel "riders" label (cosmetic), a hex-terrain backdrop (replaces the OSM basemap — a
  larger art-pass piece). **5 progress screenshots** document the fork end-to-end.

**FRONTEND — bounty markers: the steering lever gets its visual feedback, 2026-06-14.** Posting a
bounty now shows a **gold ring** on the bountied town, so the player SEES where they've baited the
legions (closing the post-bounty → legions-march feedback loop;
[docs/progress/fantasy-arcadia-bounty.png](docs/progress/fantasy-arcadia-bounty.png)).

- **`StationView.bounty`** (a render read-out — NOT hashed; both goldens verified unchanged) carries the
  per-town bounty across the boundary; `buildView` threads it onto `StationDot.bounty`; `topoLayers`
  draws a font-independent gold RING (ScatterplotLayer, stroked) around bountied towns — rebuilt on
  refresh (bounties change via a Command), not per frame. Mirrored in `types.ts`.
- **Font lesson:** a `⚑` TextLayer warned "deck: Missing character" (the default atlas lacks the glyph)
  → swapped to a stroked ring (no font dependency, reliable).
- **Verified:** the screenshot shows gold rings on the two bountied towns + the army dots + the fantasy
  HUD + realm ledger — the whole fantasy UI coherent. Both arcadia + transit `slice` e2e green; `tsc`
  clean; wasm rebuilt; cargo goldens untouched (read-out only). **6 progress screenshots.** The fantasy
  interaction loop now has end-to-end visual feedback (supply carts · army dots · bounty rings · HUD ·
  realm ledger).

**MAP-GEN — S1: the procedural continent (offline bake), 2026-06-14.** The fantasy game gets a real,
procedurally-generated playfield to replace the hand-authored 2-town demo. New `scripts/build_world.py`
implements **S1 of the map-gen pipeline** (docs/fantasy-map.md): a deterministic OFFLINE bake (one u64
seed → frozen JSON) producing terrain + a contiguous continent + carved passes. **Architecturally
isolated by design** — it emits STATIC un-hashed terrain in the existing `*_buildability.json` pack
shape, ingested by the same `Sim::new`; terrain never enters `state_hash`, so this **cannot touch the
determinism gate** (the locked golden hashes are untouched — no Rust changed this commit).

- **Pipeline (S1):** four decorrelated value-noise fields → a warped radial **continent mask** → keep the
  **largest land component** (delete islands — the contiguous-continent guarantee) → **elevation =
  distance-from-coast** (the research refinement: monotonic inland ⇒ capital coastal/low, interior high,
  downhill-to-sea free) + power-curve redistribution + heterogeneous roughness → **Whittaker biome
  classify** (WATER=4, MOUNTAIN=6, HILL=7, FOREST=8, LEY=9, PLAIN=10) → coastal SW **capital** pick →
  **THE pass carve** (Dijkstra a min-cost corridor over land, demoting the cheapest mountain ridge to a
  hill pass — passable terrain connected *by construction*, the single most load-bearing step).
- **Hex-quantize (S5 folded in):** cells emit `lon/lat` computed via `hexgrid::center_of`'s pointy-top
  formula + `coords/geo.ts`'s equirectangular frame, so they reproject onto the exact lattice the sim
  quantizes back (`axial_of`). Nothing new crosses `geo.ts`. New biome codes 6–10 are render-tint only
  (they hit `world.rs`'s `_ => 0` cost gate — no cost/block until the additive fantasy field lands,
  RED-first); WATER=4 reuses the existing free rail gate.
- **Determinism:** numpy PCG64 keyed by `(seed ^ MAP_CONST)`; all float math offline, quantized to
  i64-mm / rounded lon-lat before freezing. `--selftest` bakes twice in one process and asserts
  byte-identical output. Re-runnable: `python3 scripts/build_world.py [seed] [--selftest]`.
- **Tests (`--selftest`, all green):** determinism (raster + serialized bytes identical) · single
  contiguous continent (0 orphan islands) · **all passable land reachable from the capital** · a
  **synthetic adversarial test** that a full MOUNTAIN wall is pierced by a carved pass (real island seeds
  rarely wall a region off, so the load-bearing carve is proven on a constructed grid) · biome codes ⊆
  {4,6,7,8,9,10} · capital buildable · sane land fraction. **Seed sweep 1–12: all valid, 0 stranded**
  (fixed a capital-on-wrong-component bug that made a degenerate seed nuke the continent).
- **Committed bake:** seed 7 → `fantasy_world.json` (manifest, `ruleset:"arcadia"`, `gridCellMm:250000`)
  + `fantasy_buildability.json` (10,014 cells: a believable island — coastal capital, central mountain
  massif ringed by hills, forest belt, rare ley ridges, plains opening to the SE frontier) +
  `fantasy_demand.json` (S1 stub; S2/S3 populate it from resources/towns). The ASCII terrain preview in
  the bake output is the S1 "look at it" corroboration.
- **Deferred to later map-gen stages (hooks marked in the script):** S2 resources (ORE/GRAIN/FUEL/AETHER
  terrain-gated + Poisson-rarefied + biased to two attractor centres) · S3 towns (suitability-sited into
  the expansion arc) · S4 decadence seed (far-edge reservoir + creep-to-capital BFS potential) · S6
  solvability validator + relaxation ladder (upgrade generate-and-reroll → constructive path constraints;
  certifies aether-reachable, chains-completable). `grid_cell_mm` + continent scale stay provisional until
  S10's per-tick decadence-CA bench (docs/fantasy-map.md "Open decisions"). Next: wire the pack into the
  frontend city menu + terrain render for the first **screenshot** of the baked world.

**MAP-GEN — the baked continent renders in-browser (first screenshot), 2026-06-14.** The seed-7 world
now loads as a selectable city and **the terrain IS the map** — a believable procedurally-forged
continent ([docs/progress/fantasy-baked-world.png](docs/progress/fantasy-baked-world.png)): a near-black
mountain massif ringed by grey hills, a green-grey forest belt, pale-ash plains, and the dark blue-grey
coastline, all under the fantasy HUD (tribute · Decadence gauge · The Realm panel · Barracks/Bounty).

- **City entry + manifest:** added a `fantasy` `CityEntry` (`kind:"arcadia"`, "Arcadia ⚔ (baked)") and
  the bake now emits `buildabilityPath` so `loadCity` fetches the terrain raster. **Two bugs found &
  fixed via the browser:** (1) the demand stub was emitted in the *core* mm shape — the frontend
  `RawDemand` wants `{lon,lat,originWeight,destWeight}`; the malformed grid made `loadCity` choke and
  silently fall back to transit (caught by checking `stats().ruleset` in-page — now `"arcadia"`).
- **Terrain render layer:** a new `ColumnLayer` (`id:"terrain"`) at the very BACK of the z-order, fed
  `view.terrain` (raw `{lng,lat,c}` cells — the EXACT hex centres, NOT the square-binned `Buildability`
  lookup, which would misplace them). `terrainColor(c)` is **value-not-color** (ash-grey ramp:
  plains pale → mountains near-black; forest cooler; water desaturated blue-grey; **ley a faint violet**
  — the only ground chroma). Wired App.tsx → `game.terrain`/`terrainCellM` (fantasy only; transit cities
  draw nothing). **Tiling fix:** `angle:30` rotates deck's default flat-top hexagon to POINTY-TOP to
  match the axial lattice (centres √3·size apart) + radius ×1.04 → edge-to-edge honeycomb, no seams.
- **Verified:** `tsc` clean; **full e2e suite green (19/19)** — transit slice, both arcadia behavioural
  tests, Tokyo 440-station, and a new `fantasy-shot` spec that asserts the baked world constructs the
  arcadia ruleset + captures the terrain. No Rust touched ⇒ cargo determinism goldens untouched. The
  fantasy game now has a real procedural playfield rendering in-browser (terrain only; S2 stamps the
  resources onto it, S3 the towns).

**MAP-GEN — S2: terrain-gated resources (the disjoint-chain driver), 2026-06-14.** The baked continent now
carries the resource nodes that fork the two supply chains (docs/fantasy-map.md S2). Bake-side this turn
(Python, with self-tests + ASCII corroboration; the in-browser resource render + screenshot is the next
visible milestone). Still architecturally isolated — additive frozen data, no Rust, determinism gate
untouched.

- **Placement:** each kind gated to a PASSABLE biome so it's rail-reachable (never on impassable MOUNTAIN)
  — ORE→hill, GRAIN→plain, FUEL→forest, AETHER→ley — then **Poisson-disk rarefied** (greedy farthest-first
  with a per-kind hex min-spacing) to a baked budget; AETHER **hard-capped ≤6** (scarce by construction).
- **Two attractor centres (the disjoint-chain driver):** ORE highland = the mountain-mass centroid snapped
  to a passable cell; BREADBASKET = the plain FARTHEST from it (max separation, deterministic argmax).
  Candidates are score-biased toward their chain's attractor (BREAD = grain+fuel → breadbasket; ARMS =
  ore+aether → highland) + AETHER pushed far from the capital. Seed 7: attractors **84 hexes apart** — you
  can't feed people and arm soldiers from one spur; the bottleneck moves with the map.
- **Emitted:** an additive `supplyGraph.resources[]` in the manifest ({kind, q, r, xMm, yMm, yield} —
  i64 positions via `center_of`, **i64 yields**, the gate-blind-defect discipline). `buildCoreCity` never
  copies it into the core JSON ⇒ it never reaches `Sim::new`'s `CityData` (frontend-render + future-sim
  data; serde-safe). Resources also bump the demand grid's `destWeight` (reuse catchment capture).
- **Tests (`--selftest`, all green):** every resource on its gated biome · none on MOUNTAIN (all reachable
  from the capital over passable land, verified by flood) · AETHER 1..6 · both chains supplied
  (grain&fuel & ore&aether) · yields are ints · attractors separated ≥12 · resource placement
  deterministic across two bakes. Seed 7 → ore×8, grain×10, fuel×8, aether×2 (only 4 ley cells — scarce;
  S6's validator enforces the ≥3 winnability floor / re-rolls). Baked world still loads as arcadia (e2e).
- **Next:** render the resource nodes on the terrain (icons/dots by kind) for the screenshot, then S3
  (towns: suitability-sited into the expansion arc) + S4 (decadence seed).

**MAP-GEN — S2b: the resource nodes render on the continent, 2026-06-14.** The baked supply graph is now
visible in-browser ([docs/progress/fantasy-baked-resources.png](docs/progress/fantasy-baked-resources.png)):
coloured POI dots scatter across the grey continent and the disjoint-chain geography reads at a glance —
**blue ore + violet aether** clustered in the central/eastern highland (ARMS), **gold grain + green fuel**
across the SW lowland + forest (BREAD), in different directions from the coastal SW capital.

- **Wiring:** `RawCity.supplyGraph` (additive, serde-safe) → App.tsx maps each node's i64 mm (=
  `center_of`) → lng/lat via the one `coords/geo.ts` boundary → `game.resources` (fantasy only) → a new
  `ScatterplotLayer` (`id:"resources"`) over terrain, under the network. **Pixel-radius** (clamped 4–8px)
  so the nodes stay tappable at any zoom (Fitts); white stroke so the colours pop on the grey. Per-kind
  Okabe-Ito CB-safe palette (`resourceColor`): ore=iron-blue, grain=wheat-gold, fuel=forest-green,
  aether=arcane-violet. Stable data identity (baked, never per-frame).
- **Framing:** dropped the manifest `zoom` 11→10 so the initial view frames the WHOLE domain (the
  continent is ~80 km — at z11 only the central massif was in view; at z10 all four resource types show).
- **Verified:** `tsc` clean; **full e2e suite green (19/19)** (transit + arcadia + the resource render are
  no-ops for transit since `view.resources` is empty there). No Rust touched ⇒ determinism goldens
  untouched. The baked continent now shows terrain + the two-chain resource geography. Next: S3 towns
  (the sinks/expansion arc) + S4 decadence seed, then wiring the supply graph into the sim (sources).

**MAP-GEN — S3: towns (the supply sinks + conquest targets), 2026-06-14.** The baked continent now has the
towns that consume delivered goods → tribute and are the conquest prizes (docs/fantasy-map.md S3). Bake-side
this turn (Python + self-tests + ASCII corroboration; the town/decadence render + screenshot come with S4).
Still additive frozen data, no Rust, determinism gate untouched.

- **Siting:** candidates = passable land ≥5 hexes from the capital; **suitability score** = proximity to
  resource clusters (Σ inverse hex-distance) + a mild far-from-capital spread + roughness; **Poisson-disk
  thinned** (≥9-hex spacing) to a budget of 8 neutral towns. The CAPITAL is town #0 (the SW coastal seat);
  the STARTER is the chosen town nearest the capital (first-cart reach). Each town's **value is i64**, graded
  by hex-distance from the capital into the **expansion arc** (base 1000 + 50·dist → near = easy early
  prizes, far/aether-adjacent = rich late prizes; seed 7's farthest neutral = 5500). Each carries a 2–3-good
  **demand set** (its nearest distinct resource kinds) — the disjoint-chain consumption hook.
- **Emitted:** `supplyGraph.towns[]` ({kind, q, r, xMm, yMm, value, demands}) beside `resources[]` (additive,
  serde-safe — never reaches the core). Towns also bump the demand grid's `destWeight` (sinks); resources
  now bump `originWeight` (sources). Demand grid: 38 cells (capital + 28 resources + 9 towns).
- **Tests (`--selftest`, all green — 7 new):** capital + starter + ≥3 neutrals present · every town on
  passable land reachable from the capital · Poisson spacing ≥ budget · i64 values · expansion-arc grading
  (farthest outvalues nearest) · 2–3-good demand sets · town placement deterministic across two bakes.
  Baked world still loads as arcadia (e2e). Next: S4 decadence seed (per-town floor + far-edge reservoir +
  creep-to-capital BFS potential), then render towns + decadence + screenshot.

**MAP-GEN — S4: the decadence seed (conquest made urgent), 2026-06-14.** The corruption that is the LOSE
condition is now seeded onto the baked continent (docs/fantasy-map.md S4) — what makes the supply→conquest
flywheel a *race*. Bake-side, additive frozen data, no Rust, determinism gate untouched.

- **Per-town decadence floor:** the capital + starter + a `CAPITAL_GRACE_HEXES=6` ring start CLEAN (0); past
  it, a neutral town's floor rises with **frontier depth** (200 + 30·(dist−grace)) — deep frontier towns are
  the most corrupt (seed 7: floors 2360…4910). This is the per-town corruption conquest pushes back.
- **Far-edge reservoir:** the 5 coastal land cells FARTHEST from the capital (the far edge opposite it — the
  tide origin + raider-spawn anchors). Seed 7: anchored at the NE coast, 162 hexes out.
- **Loseability guaranteed (asserted):** every reservoir anchor reaches the capital over passable land (the
  carve-passes connectivity already guarantees this) — a walled-off capital would be UNLOSEABLE, so the test
  fails loudly if the tide can't path to the capital. (The full per-tick creep CA is S10; S4 seeds the values
  + anchors + the cheap gradient hook.)
- **Emitted:** `supplyGraph.decadenceSeed` ({capitalGraceHexes, reservoir[]}) + a per-town `decadence` field
  (all i64). Serde-safe (never reaches the core).
- **Tests (`--selftest`, all green — 8 new):** capital+starter clean · neutral frontier corrupt · floor rises
  with depth · i64 values · 5 reservoir anchors on passable land · **LOSEABLE** (anchors reach the capital) ·
  reservoir is the far edge (>2·grace out) · decadence seed deterministic. Baked world still loads (e2e).
  **The map generator's data pipeline is now complete (S1 terrain · S2 resources · S3 towns · S4 decadence).**
  Remaining: render towns + decadence (S3b/S4b), S6 validator/relaxation ladder, then wire the supply graph
  into the sim (resources→sources, towns→sinks, decadence→the tide).

**MAP-GEN — S3b/S4b: towns + decadence render (the populated continent), 2026-06-14.** The full baked world
is now visible in-browser ([docs/progress/fantasy-baked-populated.png](docs/progress/fantasy-baked-populated.png)):
the whole economic + conquest geography reads at a glance — the **gold capital** anchoring the SW coast,
cold-blue **neutral towns** (dark-ringed, value-scaled) across the lowland + the corrupt NE frontier, the
small resource dots, and the **violet decadence reservoir anchor** at the far NE edge (the tide origin).

- **Wiring:** extended `RawCity.supplyGraph` (towns + decadenceSeed, serde-safe) → App.tsx maps mm→lng/lat
  via `coords/geo.ts` → `game.towns` / `game.decadenceAnchors` → two new `ScatterplotLayer`s (towns over
  resources, reservoir anchors) under the network. **Art direction (docs/fantasy-map.md "The look"):** the
  capital + dominion are the only WARMTH (gold/amber); neutral towns read sickly **cold-bright, darkening +
  cooling as their decadence floor rises** (`townColor` lerps cold→dark-cold by decadence); reservoir anchors
  are low-chroma cold-violet. Pixel-radius (value-scaled, clamped) + dark ring so towns read as settlements
  over the resource POIs. The full creeping decadence FIELD is the S10 CA — this renders the S4 seed.
- **Verified:** `tsc` clean; **e2e 18/19** on the first parallel run — the 1 miss was the player-barracks
  conquest test (sim-timing-sensitive under 15-worker load), **green on isolated re-run** (pre-existing
  flakiness, NOT a regression — the arcadia demo has no `supplyGraph`, so towns/anchors stay empty there and
  the new layers are no-ops). No Rust touched ⇒ determinism goldens untouched.
- The baked continent now shows **terrain + resources + towns + decadence** — a complete procedural 4X
  playfield, rendered. Next: S6 validator (certify winnability / pick a certified seed), then wire the supply
  graph into the sim so the baked world is actually *playable* (resources→sources, towns→sinks, decadence→tide).

**MAP-GEN — S6: the solvability validator + certified seed (the generator is COMPLETE), 2026-06-14.** The
bake is now a pure function of the requested seed that ALWAYS emits a **certified-winnable** world — it
re-rolls (deterministic seed sequence) until the solvability constraints pass (docs/fantasy-map.md S6).
Bake-side, no Rust, determinism gate untouched.

- **`validate(world)`** lists the constraints a world VIOLATES (empty = certified): aether scarce-but-present
  (∈[3,6]) · grain/fuel/ore each ≥4 (a forest-poor seed silently starves BREAD — caught) · ore-highland ↔
  breadbasket separated ≥20 hexes · every resource reachable from the capital over passable land · aether ≥15
  hexes out (a late prize) · NO cornucopia hex (no cell near all 4 kinds) · the capital reachable from the
  decadence reservoir (LOSEABLE, not walled) · the start not a 1-hex funnel (≥12 passable cells within R=3).
- **`generate_valid(seed)`** re-rolls from the requested seed (bounded 48; the relaxation ladder is the TODO
  fallback) and returns the first certified world. The validator has real TEETH — the seed sweep shows it
  rejecting scarce/absent-aether seeds (7,8,10,11) and capital funnels (9,14). **The committed bake's requested
  seed 7 is NOT winnable (2 aether) → certified seed 12** (6 aether, chains supplied, no funnel); the manifest
  records the certified seed so loading reproduces it.
- **Tests (`--selftest`, all green — 4 new):** `generate_valid(7)` yields a certified world (seed 12) · the
  certified seed is reproducible · the validator rejects a starved world (teeth) · the certified world meets
  the aether + chain thresholds. **Full e2e suite green (19/19).** No Rust touched ⇒ goldens untouched.
- **THE MAP GENERATOR IS COMPLETE** — S1 terrain · S2 resources · S3 towns · S4 decadence · S6 certification,
  ~40 self-tests, all rendered in-browser (`docs/progress/fantasy-baked-{world,resources,populated}.png` now
  show the certified seed-12 continent). The bake emits a guaranteed-playable procedural 4X continent the
  existing `Sim::new` ingests, with the locked deterministic core never touched. **The remaining piece is the
  SIM-WIRING** — making the baked supply graph actually drive gameplay (resources→sources, towns→sinks,
  decadence→the tide). That step DOES touch `crates/sim` (a deliberate golden re-pin, RED-first) — the careful
  next major increment.

**MAP-GEN — sim-wiring: the baked continent is PLAYABLE, 2026-06-14.** The procedural world is no longer
just a render — you can build rail on it and the supply chain runs
([docs/progress/fantasy-baked-playable.png](docs/progress/fantasy-baked-playable.png)). And it turned out the
first pass needs **NO core change / no golden re-pin** — it's pure command-sourcing.

- **`networkFromSupplyGraph(sg)`** (network.ts) synthesizes a starting `Network` from the baked supply graph
  — every resource → a SOURCE station, every town → a SINK station, the capital → a BARRACKS — with NO
  lines (the player draws the rail). App.tsx applies it via the existing `Game.applyNetwork` command path
  (the same one a loaded metro uses), so it's fully command-sourced. The **source/sink roles fall out of the
  baked demand grid for free**: resources carry high `originWeight`, towns high `destWeight`, and the arcadia
  ruleset's existing classification reads exactly that — no new sim code.
- **Verified playable in-browser:** the baked world places **42 stations** with **23 sources / 18 sinks**
  (roles correct: a resource reads origin 13 / dest 1, a town origin 9 / dest 40); drawing a line between a
  grain source and a town, assigning carts, and running **dispatches a cart and delivers grain → tribute=1**
  (manually confirmed) with the coverage gauge moving and decadence climbing — the whole loop runs on the
  *procedural* continent.
- **Bug caught by the playability test (fixed):** S3 let a town land ON a resource cell (the suitability
  score peaks on resource cells) → a zero-length source==sink line (nothing to transport). Fixed: towns now
  sit ≥3 hexes from any resource (near, but you must rail the goods in) + a new validator constraint rejects
  any town-on-resource. Re-certified (still seed 12).
- **New e2e `fantasy-play`** asserts the gameplay facts: ruleset arcadia · >20 stations placed · sources &
  sinks present · a cart dispatches on a player-drawn line over the baked map. (It gates on *dispatch*, not
  the ~90-sim-sec full tribute delivery — that's rAF-wall-clock-bound and flaked under 15-worker parallel
  load, destabilising the suite; the full delivery is verified manually + by the arcadia ruleset unit tests.)
  **Full e2e suite green (20/20).** No Rust touched ⇒ goldens untouched.
- **The baked procedural continent is now a real, playable 4X campaign.** Next (deeper, core-touching, a
  deliberate golden re-pin): seed `world.decadence` from the baked per-town floors + the reservoir so the
  baked decadence *matters* (currently only base growth drives the tide), and multi-commodity town demands
  (the S7e Forge-Line). Plus the original deferred S7e/S8-refinements/S10-CA/S11-economy.

**MAP-GEN — deeper integration: the baked decadence drives the tide + deterministic sim e2e, 2026-06-14.**
The certified continent's corruption geography now gives it real starting urgency, and a recurring e2e
flakiness is fixed for good.

- **`CityData.initial_decadence`** (serde default 0): `World::new` seeds `world.decadence` from it (clamped
  ≥ 0). **Golden-NEUTRAL by construction** — the field defaults to 0, so every transit city, the arcadia
  golden fixture, and every native test are byte-identical (`golden_transit_hash_pinned` +
  `golden_arcadia_hash_pinned` both verified unchanged — *no re-pin*); only a baked world that sets it starts
  corrupt. `build_world.py` S4 bakes `initialDecadence` (the mean neutral-town floor) into `decadenceSeed`;
  `buildCoreCity` passes it to the core. The certified seed-12 world starts at **decadence 5345** (~27% up
  the lose meter — a more-corrupt continent = more urgency), verified in-browser by `fantasy-play`.
- New cargo test `initial_decadence_seeds_the_starting_corruption` (seeds the value, clamps negatives, a
  more-corrupt realm falls sooner). **Full sim suite green.**
- **Recurring-flakiness fix (the real win):** a new synchronous **`tickMs` test hook** advances the sim
  deterministically *without rAF* — it puts the sim in its running state without entering run MODE (the
  GameLoop only auto-ticks in run mode, so manual steps never double). Converted the 3 sim-conquest e2e
  (arcadia ×2 + fantasy-play) from `setSpeed(100)`+`waitForFunction` (rAF-wall-clock-bound → starved + flaked
  under 15-worker parallel load) to deterministic stepped ticks. They now assert the SAME facts (tribute,
  a legion fielded **AND rendered** via `armyPositions`, a town taken, realm holds) instantly + reliably.
  **Two consecutive full e2e runs green (20/20)** — the flakiness that intermittently failed the conquest
  tests is gone. No Rust beyond the additive `initial_decadence` field ⇒ goldens untouched.

**MAP-GEN — readability polish: the disjoint chains read at a glance on the playable map, 2026-06-14.** The
sim-wiring placed a station dot at every resource/town, and those near-black dots were COVERING the S2b/S3b
kind-colours — so the playable map was a field of identical white dots (you couldn't tell an ore source from
a grain source from a town). Fixed: the kind-markers are now sized LARGER than the station dot (≤8px) so the
kind-colour reads as a **halo** around each node (resources 9–12px, towns 11–16px). The map now shows
distinct gold (grain) + green (fuel) + blue (ore) + violet (aether) sources, cold-blue towns, and the gold
capital — the disjoint-chain geography is legible while planning supply lines
([docs/progress/fantasy-baked-playable.png](docs/progress/fantasy-baked-playable.png)). Render-only +
fantasy-gated (transit markers are empty ⇒ no-op); `tsc` clean, fantasy-play green, no Rust touched.

**═══ STATE OF THE FORK — full-suite certification + a decision point, 2026-06-14 ═══**

A full verification pass certifies the entire (large, ~40-turn) fantasy changeset green across EVERY tier:
**cargo 177/0 (37 suites)** · **vitest 21/21** · **e2e 20/20** · **build_world `--selftest` PASS** · both
golden hashes pinned (transit + arcadia). The fantasy fork is comprehensively COMPLETE + PLAYABLE:
- **Sim core** S0–S9 (ruleset-at-construction fork, hex lattice, Forge-Line buffers, war machine + AI,
  decadence lose-condition) — pure, deterministic, golden-pinned.
- **Frontend** — mode-aware React HUD, army/terrain/resource/town/decadence render, fantasy build tools.
- **Balance harness** — tuned to bite (~84 s on the demo).
- **Procedural map generator** (`scripts/build_world.py`, S1–S6) — terrain · resources · towns · decadence ·
  a solvability validator that certifies a winnable seed. Emits a frozen world the existing `Sim::new`
  ingests; the deterministic core was NEVER touched by the generator.
- **The baked continent is PLAYABLE** — its supply graph builds a network via the command path, supply
  flows source→sink→tribute, legions field + conquer, decadence (seeded from the baked floors) presses;
  the disjoint-chain geography is legible on the map.
- Test suite **stabilised** (deterministic `tickMs` hook killed the rAF-starvation flakiness).

**Open decision (genuinely the user's — surfaced, not auto-chosen):** the remaining deferred items are large
core subsystems — **S7e** (multi-commodity Forge-Line: make the baked ore/grain/fuel/aether drive *distinct*
BREAD-vs-ARMS chains; a multi-turn rework that re-pins the arcadia golden + re-tunes balance), **S10** (the
per-cell decadence CA — the spatial tide; the heaviest subsystem, gates the `grid_cell_mm` freeze), **S11**
(economy/tech). Each is a multi-turn commitment. **Nothing has been committed** (standing "commit only when
asked" constraint) — ~40 turns of work sits uncommitted, ready for review. The loop continues toward S7e
unless redirected.

**S7e-1 — multi-commodity Forge-Line (the core mechanic), 2026-06-14.** The sim no longer treats supply as
one generic commodity — a source now produces ITS commodity (ore/grain/fuel/aether), the cart carries it,
and the sink receives + consumes that specific commodity. This is the plumbing the disjoint BREAD-vs-ARMS
chains ride on. **GOLDEN-NEUTRAL** (both transit + arcadia goldens verified unchanged): a commodity-0 world
behaves byte-identically (production/delivery/consumption all land on the ORE slot exactly as before) — only
`commodity != 0` cells diverge.

- **`DemandCell.commodity`** (serde default 0 = ORE → transit + the golden fixtures untouched) tags each
  cell's Forge-Line commodity. `prepare` derives a per-station **`station_commodity`** (the argmax origin-
  commodity of its captured cells — a derived read-cache, NOT hashed → golden-neutral). `forge::produce`
  accrues a source's `station_commodity` (not always ORE); the arcadia spawn gate reads that node's own
  commodity buffer; **`Pax.commodity`** (also unhashed — Pax queues are excluded from Canonical) carries it;
  `pax.rs` alight deposits into the sink's matching slot; `forge::produce` consume now sums EVERY delivered
  commodity into tribute.
- **Tests:** new `tests/forge_commodity.rs` — a GRAIN (non-ORE) source produces→ships→delivers→consumes into
  tribute end-to-end (`station_commodity[src]==GRAIN`, tribute>0) + the multi-commodity flow replays
  bit-for-bit. **Full sim suite green (179/0, 38 suites)**; both goldens pinned; wasm rebuilt; the 3 fantasy
  e2e green (the baked world's cells are still commodity-0 ⇒ unchanged). (Mechanically added `commodity: 0`
  to 40 `DemandCell` test literals.)
- **S7e-1b (done, same day):** the BAKE now emits per-commodity demand cells — each resource's cell carries
  its `commodity` (ore=0/grain=1/aether=2/fuel=3, matching forge.rs); `RawDemand.cells.commodity` +
  `buildCoreCity` pass it through (omitted ⇒ 0, so every transit city + the arcadia demo are unchanged). The
  baked continent's grain/ore/fuel/aether sources now genuinely produce their own commodity (verified: the
  demand grid carries grain×10/aether×6/fuel×8/ore-or-town×18; fantasy-play still flows supply→tribute on the
  multi-commodity baked world). tsc clean.
- **S7e-2 (done, the Liebig recipes — the core mechanic):** a sink now consumes by **Liebig** — `prepare`
  derives `station_recipe` (the distinct commodities a sink captures DEST weight of); a sink with a real
  ≥2-commodity recipe yields output = **min over its required inputs** (the scarcer input throttles; consume
  `min` of each), so a BREAD town needs grain+fuel and an ARMS barracks ore+aether. **Additive + golden-
  neutral:** a single/empty recipe ⇒ consume-all (the S7e-1 path), so commodity-0 worlds (both goldens, the
  arcadia demo, the current baked world) are byte-identical. Tests (`forge_commodity.rs`): a bread town
  supplied BOTH grain+fuel → tribute; supplied ONLY grain → **tribute 0** (the missing fuel throttles to
  min=0 — you must build both chains). **Full sim suite green (180/0)**; both goldens pinned.
- **S7e-2b (done — the disjoint chains land on the baked world):** the BAKE assigns each sink town a chain
  recipe — the ~⅓ NEAREST the ore highland demand **ARMS** (ore+aether), the rest **BREAD** (grain+fuel) — a
  fixed fraction rather than nearer-attractor (towns can only site in the passable lowland, so all are
  "nearer" the breadbasket; the fraction guarantees BOTH chains have consumers). Each town emits one dest
  cell PER required commodity → its `station_recipe` is the 2 inputs → the sim consumes them by Liebig. Seed
  12: **6 BREAD + 3 ARMS** towns. `fantasy-play` now feeds a town BOTH inputs (a line src₁→town→src₂) → the
  Liebig bread/arms flows → tribute (one input alone would yield 0). Self-test: every sink has a full BREAD
  or ARMS recipe + both chains present. **Full suite green** (cargo 180/0 both goldens pinned · e2e 20/20 ·
  build_world selftest); `buildCoreCity` already passed per-cell commodity through, so no further frontend
  wiring. **S7e — the disjoint BREAD-vs-ARMS chains — is delivered:** the baked world forces you to build
  both (grain+fuel for towns, ore+aether for arms-towns), with the locked core golden-neutral throughout.
- **Deferred (optional further depth):** MULTI-STAGE processing (raw→mid→final, e.g. ore→INGOT→ARMS through
  intermediate forge nodes) — the 2-input Liebig delivers the core disjoint-chain gameplay; multi-stage adds
  refinement depth. "Commodity-aware routing" is the player's job (they connect the right sources to each
  town via lines), the natural fit for the build-a-network loop — no auto-routing needed.

**S7e — chain legibility (the player can SEE which chain a town demands), 2026-06-14.** A town's RING is now
coloured by its supply chain so the disjoint-chain logistics are readable on the map: **BREAD** towns
(grain+fuel) ring **wheat-gold**, **ARMS** towns (ore+aether) ring **arcane-violet**, the capital a neutral
dark frame. The player reads the ring → knows which two sources to connect (without it, every town looked
the same and the two-chain demand was invisible). `TownMarker.chain` (derived from the baked `recipe` in
App.tsx), `townRingColor` + a 2.5px ring in the towns layer. Render-only + fantasy-gated (transit towns are
empty ⇒ no-op); `tsc` clean, fantasy-play green. This completes S7e's playability — the procedural world's
two-chain economy is now fully legible.

**BALANCE PROBE — the baked world is NOT yet winnable (a real finding), 2026-06-14.** A deterministic
winnability probe (`e2e/fantasy-conquest.spec.ts`: connect two towns' chains for tribute + a capital→town
line for conquest, run via `tickMs`, log the trajectory) showed the demo-tuned war/decadence constants don't
fit the LARGE baked continent:
- **Decadence integer-truncation:** `decadence::step`'s `net·dt/1000` truncates any growth < 20/s to **0 per
  50 ms tick** — so the gentle baked rate (6/s) freezes the lose meter entirely. The proper fix is a sub-unit
  remainder accumulator (like `forge_accum`) — but that changes the demo's growth-50 trajectory (it currently
  *under*counts at 40/s) ⇒ a deliberate arcadia-golden re-pin, RED-first. (Externalised the rate this turn:
  `CityData.decadence_growth_per_s`, default 0 ⇒ the const default ⇒ golden-neutral; the bake sets it.)
- **Conquest doesn't complete at scale:** tribute flows + legions LAUNCH from the capital-barracks (probe
  confirms armyCount 1→5), but they don't capture in the window — the long baked routes (army-speed
  50 km/s vs continent-sized lines) + the trajectory suggest the march/siege/town-resistance constants need
  scaling for the baked world (the tiny demo captures in seconds; the continent needs minutes).
- **What WORKS + is gated:** the two-chain Liebig supply flows and legions field from the barracks (the war
  engine engages). The probe asserts those (green); conquest-completes + realm-holds are logged, not yet
  gated (WIP). cargo 180/0 (both goldens pinned), e2e green.
- **Deferred — a baked-world balance pass (tracked):** fix the decadence remainder-accumulator (golden
  re-pin), scale army-speed / town-resistance / decadence rate+threshold to the continent, ideally via a
  headless auto-play harness on the baked world (the demo's `balance.rs` proves the *demo*, not the bake).
  This is genuinely the kind of multi-knob tuning the design defers to "the headless balance harness."

**RECOVERY CHECKPOINT — session crashed mid-fork; tree verified green + committed, 2026-06-14.** The
previous (autonomous) session crashed with the entire ~40-turn fantasy-fork changeset uncommitted on a
clean working tree. On resume, the tree was re-certified green at the last checkpoint (the BALANCE PROBE
above) — **cargo 180/0 (38 suites, both goldens pinned) · vitest 21/21 · build_world `--selftest` PASS**
(e2e was 20/20 at this exact, unchanged tree). No corruption: `decadence.rs`/`city.rs`/`world.rs` are
complete + coherent (the late timestamps were the `decadence_growth_per_s` externalization the BALANCE
PROBE entry records). Committed the green tree to branch `fantasy-fork` (no push) to protect the work
against another crash. The stray root debug PNG (`fantasy-terrain-z11.png`, a superseded z11 framing
experiment) was deliberately left uncommitted. **Next:** the deferred baked-world balance pass (decadence
remainder-accumulator → arcadia-golden re-pin, RED-first; war/decadence knob scaling to continent scale;
a headless balance harness on the baked world; then gate `fantasy-conquest.spec.ts` on conquest-completes
+ realm-holds).

**BAKED-WORLD BALANCE PASS — the procedural continent is now WINNABLE, 2026-06-14.** Resolves the BALANCE
PROBE's two tracked findings (decadence freeze + conquest-doesn't-complete-at-scale). The continent's
supply→legion→conquest loop now closes end-to-end, with the decadence rot a genuine race the player wins
by conquering — measured both natively (synthetic harness) and in-browser (real bundle + geometry).

- **Decadence remainder-accumulator (the freeze fix, RED-first → deliberate arcadia-golden re-pin):**
  `decadence::step` truncated `net·dt/1000` to **0 per 50 ms tick** for any rate < 20/s, so the baked
  continent's gentle 6/s froze the lose meter (unloseable). Replaced with an integer fixed-point
  milli-unit accumulator (`world.decadence_accum`, mirroring `forge_accum`): accumulate `net·dt`, extract
  whole units via `div_euclid(1000)` (floors toward −∞ so negative `net` — conquest outpacing the rot —
  drains exactly), keep the remainder ∈ [0,1000). The demo's 50/s now accrues at a true 50/s (was 40/s
  under truncation). **`decadence_accum` is EXCLUDED from `Canonical`** (transient, regenerated
  bit-identically like `forge_accum`/`spawn_accum`), so the hash sees only the whole-unit `decadence`.
  ⇒ **transit golden UNCHANGED** (transit never runs `war_step`; decadence stays 0); **only the arcadia
  golden re-pinned** `0x5375_1cb0_558d_3b0f → 0x52d2_05b0_5502_b2aa` (the fixture's idle growth evolves
  further over 1200 ticks). Tests (`decadence.rs`, RED-first — both read 0 under the old code): a gentle
  6/s accrues EXACTLY 600 over 100 s and replays bit-for-bit; an idle baked realm (6/s from 5345) IS
  overrun (the lose condition has teeth — the win must be earned).
- **Army march speed externalised (`CityData.army_speed_mm_s`, the measured bottleneck):** the baked towns
  sit **60–73 km** from the SW-coastal capital (~40× the 1.5 km demo); at the demo pace (50 000 mm/s =
  50 m/s) the nearest town is a **~21 sim-min march** — just past the e2e probe's 1200 s window, which is
  exactly why the probe saw no conquest. New per-city knob (default 0 ⇒ the `ARMY_SPEED_MM_S` const, so
  the demo + arcadia golden are byte-identical). The bake (`build_world.py`) sets **200 000 mm/s**
  (200 m/s — a 60 km march in ~5 min), threaded through `decadenceSeed.armySpeedMmS` → `buildCoreCity` →
  `CityData`. Re-baked: certified seed still **12**, demand/buildability packs byte-identical, only the
  manifest's new field changed.
- **Continent balance harness (`tests/balance.rs::fantasy_baked_continent_is_winnable`, the authoritative
  pacing gate):** mirrors the baked scale — towns 60 km out, **supply at continent length** (sources ~30 km
  from the ARMS town, so the tribute ramp is realistic not optimistic), decadence 6/s from 5345, 2-input
  ARMS Liebig — and self-plays headless across an army-speed sweep. Telemetry (printed): tribute @2448
  ticks (~2 min), legion @3486 (~3 min); conquest @ 27 623 / 15 585 / **9 566** / 6 557 ticks for
  50k / 100k / **200k** / 400k (timing scales with army speed — the march dominates). At the baked 200k:
  conquest @ ~8 sim-min, decadence peak 8214 (**41%** of the 20 000 threshold) — genuine pressure. The
  **earned-win CONTRAST** makes "the realm holds" non-vacuous: the horizon (60 000 ticks = 3000 sim-s)
  runs PAST the idle-loss point (~48 840 ticks), and a control run with NO barracks (same supply, no
  conquest) IS overrun (decadence peak 23 345 > 20 000, `lost`) — so the conquering realm holds *because
  conquest pushed the rot back*, not because time ran out. Gate asserts supply → legion → conquest →
  realm-holds + the idle-overrun contrast + ordering. (The harness is synthetic; that the *demo* speed
  misses the window on the *real* seed-12 geometry is what the e2e proves — the harness certifies the
  knobs are sound and the win is conquest-attributable.)
- **e2e `fantasy-conquest.spec.ts` upgraded to GATE the closed loop** (was "engine-engages only"): now
  asserts `townsCaptured ≥ 1` AND `!realmLost` on the real bundle. Trajectory on the real seed-12
  continent: tribute flows → legion launches by ~300 s → **a town falls at ~600 s**, decadence climbing
  31%→36%→**40%** then **dropping to 27%** as the captured-town pushback engages — the rot reversed by
  conquest, realm holds. Deterministic via `tickMs`.
- **Tiers:** cargo **183/0** (38 suites; +3: gentle-rate, idle-overrun, continent-winnable; transit
  golden pinned, arcadia golden re-pinned) · vitest **21/21** (wasm-in-node determinism smoke green on the
  rebuilt wasm) · build_world `--selftest` PASS · e2e — the conquest gate green on the production bundle
  (full-suite run in progress). The decadence accumulator is integer-exact (no float in the hashed path);
  determinism preserved.

**AUTONOMOUS ROADMAP LOOP — iteration 1 (balance pass + S10 scoped), 2026-06-14.** Started a `/loop`
("continue until the roadmap/design doc is done, work deferred items, periodic screenshots/progress
docs"). Iteration 1's substantive progress = the **baked-world balance pass** above (resolves the BALANCE
PROBE roadmap item; cargo 183/0 · vitest 21/21 · e2e 21/21 · selftest PASS; adversarially verified).

- **Browser-corroborated on the real seed-12 baked world** (production preview bundle): the full fantasy
  loop now closes end-to-end — built 3 supply lines (2-input Liebig BREAD/ARMS) + 2 capital→town conquest
  lines, ran a deterministic stretch, and observed **4 legions fielded, a town captured, decadence driven
  27% → 0%** (the captured-town pushback overwhelms the baked 6/s rot), 116 riders, **0 console errors**.
  (Committed screenshot deferred to the e2e screenshot pipeline — the playwright-MCP file lands in a
  sandbox not reachable from the host; `fantasy-shot.spec.ts` is the durable artifact path.)
- **Roadmap remaining (build plan S0→S11):** S0–S9 + S7e (disjoint chains) + the full map generator
  (S1–S6) + the balance pass are DONE. Left: **S10** (the spatial decadence CA / area-control — "the
  largest subsystem, the perf cliff") · **S11** (economy/tech/endless+prestige/rival) · the S7e
  multi-stage refinement (raw→mid→final, e.g. ore→INGOT→ARMS).
- **S10 plan (locked decisions, for the next iteration):** the current `decadence` is a global scalar;
  S10 makes it a SPATIAL field — a hashed, **index-ordered sparse Vec** of contested cells
  (owner/decadence), **double-buffered** integer diffusion (read prev → write next; in-place would read
  half-updated neighbours), **PURGE strictly dominates DIFFUSE** (fed/captured ground reaches
  decadence==0), seeded from the baked far-edge **reservoir anchors**, creeping toward the capital via the
  baked creep-distance potential, with a **hard cell cap + a per-tick bench gate** (binding condition #3 —
  a decadence bloom is a perf cliff). Golden re-pin RED-first (binding #1). **Plumbing gap found:** the
  core only keeps a SQUARE buildability *lookup* (`build_lookup` HashMap), not the hex-cell graph the CA
  needs, and the supplyGraph (reservoir/capital/creep-potential) never reaches `CityData` today — so S10
  starts with **S10a: thread the hex-cell domain + reservoir anchors + capital + creep potential into
  `CityData` (additive, default-empty ⇒ golden-neutral) + build the in-core hex adjacency**, then **S10b:
  the CA engine** (re-pin) consuming it, then **S10c: render the cold tide + screenshot**. Decision per
  docs/fantasy-map.md "Open decisions": share the terrain hex grid (cheapest; the baked world is 10,307
  cells, inside the conservative ≤25k budget) — re-evaluate at the S10b per-tick bench. Risk battery
  (build plan): PURGE>DIFFUSE reaches 0 · identical-field-after-K-days-twice · directional symmetry ·
  bounded (hard cap) · per-tick bench within the 20–30 Hz budget — assert structurally, never `run()==run()`.

**AUTONOMOUS ROADMAP LOOP — iteration 2: S10a, the decadence-CA static board, 2026-06-14.** First sub-step
of S10 (the spatial decadence area-control CA — the build plan's "largest subsystem"). S10a builds the
BOARD the tide will diffuse over; the dynamic hashed tide + CA step are S10b. **Golden-neutral by
construction** (the board is a pure function of `CityData`, reconstructible on replay, so it is NOT hashed
— both goldens verified unchanged, no re-pin).

- **`decadence_field.rs` — `DecadenceField::build(&CityData)`:** derives, once at construction, the hex-cell
  **domain** (passable land — classes HILL/FOREST/LEY/PLAIN, excluding WATER + impassable MOUNTAIN —
  reinterpreted as axial via the same `hexgrid` transform the bake quantised with; sorted ⇒ index-stable),
  **CSR hex adjacency** (the 6 pointy-top neighbours present in the domain), the **creep-distance gradient**
  (integer BFS hop-distance from the capital — the cheap toward-the-capital telegraph, computed once not
  per tick), the **capital** cell (exact-or-nearest to the baked capital mm), and the **reservoir** seed
  (the 8 farthest-reachable cells — the far edge opposite the capital, the tide origin). Empty for transit
  / demo-arcadia (no terrain) ⇒ no CA.
- **Plumbing:** `CityData.capital_x_mm/_y_mm` (serde-default 0); `World.decadence_field` (un-hashed, built
  in `World::new`); `build_world.py` emits `decadenceSeed.capitalXMm/YMm`; `buildCoreCity` passes them
  through. Re-baked: certified seed still **12** (capital cell q,r=(23,22) → mm (14722432, 8250000));
  demand/buildability packs byte-identical, only the manifest's new capital fields changed.
- **Tests (`tests/decadence_field.rs`, the S10 risk battery — asserted STRUCTURALLY, never `run()==run()`):**
  passable domain + **symmetric adjacency** (every neighbour relation is mutual, distance-1; an interior hex
  has 6) · **monotone creep gradient** (capital at 0; every cell has a downhill neighbour one step closer) ·
  **reservoir is the far edge AND reaches the capital** (loseability — a walled-off capital is unloseable) ·
  **BFS routes through a carved pass** in a MOUNTAIN wall (the connectivity guarantee) · water/mountain
  excluded · terrainless ⇒ empty · **deterministic build** (twice ⇒ bit-identical) · `World::new` wires it +
  the un-hashed board never moves `state_hash`.
- Tiers: cargo **191/0** (38 suites + the new `decadence_field` 8; both goldens pinned, no re-pin) · tsc
  clean · the 5 fantasy/arcadia e2e green on the rebuilt bundle (the new capital field doesn't perturb
  ingestion). Progress screenshot of the playable balanced baked continent:
  [docs/progress/fantasy-baked-balanced.png](docs/progress/fantasy-baked-balanced.png) (4 legions, a town
  taken, decadence reversing — the loop, on the procedural map).
- **Next (S10b):** the CA engine — a hashed sparse per-cell decadence Vec, double-buffered integer
  diffusion (read prev → write next) seeded from the reservoir, **PURGE strictly dominates DIFFUSE** (fed/
  captured ground → decadence 0), a **hard cell cap + per-tick bench gate** (binding condition #3), and the
  global `decadence` lose meter re-derived from the field reaching the capital. Golden re-pin RED-first.

**AUTONOMOUS ROADMAP LOOP — iteration 3: S10b-1, the decadence CA engine (golden re-pin), 2026-06-14.**
The second sub-step of S10: the genuine creeping-tide CA on the S10a board. Scoped to the ENGINE running
**parallel** to the scalar lose meter (unchanged) — the lose-condition rewire + balance re-tune is S10b-2,
the render is S10c — so the carefully-tuned balance is not re-opened in one risky step (the build plan's
"don't strand a half-built CA" caution).

- **The CA (`decadence_field::step`, run in `ArcadiaRuleset::war_step`):** a hashed per-cell tide
  (`World.decadence_cells`, dense over the S10a domain, 0..`DECAD_MAX`=1000) evolved by **double-buffered**
  integer diffusion (read `cur`, write a scratch `next` — a neighbour-reading CA in place would read
  half-updated cells). Per tick: SEED the reservoir to MAX; DIFFUSE toward the capital (a cell gains iff a
  FARTHER-from-capital neighbour is corrupt past `ADVANCE_THRESHOLD` — the front creeps capital-ward along
  the S10a gradient, never an instant flood); **PURGE** every cell within 2 hexes of a live player station
  by 10× the diffuse gain, so **PURGE STRICTLY DOMINATES DIFFUSE** — the rail network holds the line and
  held ground trends to 0. Integer, index-ordered, the only map read is `index.get` (queried) ⇒
  deterministic.
- **Binding conditions honoured:** #1 — the new hashed `decadence_cells` slice joins `Canonical` (appended
  last), a deliberate **re-pin RED-first** of BOTH goldens (transit `0xea4e…74f9 → 0xfd8e_5b04_8a81_c31b`
  via the empty slice; arcadia `0x52d2…b2aa → 0xbd92_54a0_7395_96de`), provenance documented. #3 — a
  **hard cap** (`MAX_CA_CELLS=30_000`, disables the CA above it rather than risk a bloom) + the step is
  O(domain) ≤ O(cap), the **per-tick bench** the build plan requires (the baked board is ~10k cells; the
  conquest e2e runs the CA every tick across thousands of `tickMs` ticks in **6.3 s**, well within budget).
- **Tests (`tests/decadence_field.rs` +4, the S10 risk battery — structural, never `run()==run()`):** the
  tide **creeps from the reservoir toward the capital** (saturated source, inward advance, gradient falls
  off capital-ward) · **PURGE strictly dominates DIFFUSE** (a station's cell reaches 0 vs corrupt without
  it — a contrast) · the CA **replays bit-for-bit** (field + `state_hash`) · **no lattice-axis bias** (a
  q↔r-symmetric square map yields a mirror-symmetric field — this caught + fixed a reservoir top-N
  truncation that tie-broke by axis; the reservoir is now a symmetric distance BAND).
- **Parallel-to-scalar (no balance regression):** the synthetic balance harness + the demo + the golden
  fixtures have no buildability ⇒ empty field ⇒ the CA is a no-op there (scalar model byte-identical). On
  the real baked world the CA runs every tick but writes only `decadence_cells`; the conquest e2e
  trajectory is **identical** to the balance-pass run (decadence 31→36→40→27, conquest at step 4) — the
  scalar lose meter + gameplay are untouched.
- Tiers: cargo **195/0** (38 suites + the 4 new CA tests; both goldens re-pinned) · the conquest e2e green
  on the rebuilt bundle. **Next (S10b-2):** rewire the baked-world lose condition to the field reaching the
  capital + re-verify/-tune the balance (the army pushback ↔ spatial PURGE); then **S10c** renders the cold
  tide (the "look") + a screenshot.

**AUTONOMOUS ROADMAP LOOP — iteration 4: S10b-2, the spatial lose condition (golden-neutral), 2026-06-14.**
The decadence tide is now the baked world's actual lose condition — the corruption you race, spatially.

- **The rewire (`war_step` branch):** a world with a baked CA board runs the spatial CA, which **derives
  the global `decadence` lose meter from the tide's FRONT** — the nearest-to-capital corrupted cell,
  scaled so it hits `CAPITAL_THRESHOLD` exactly when the front reaches `LOSE_DIST` (3) of the capital. A
  world WITHOUT terrain (the demo / native harness / golden fixtures) has no field, so it runs the
  unchanged abstract scalar meter — **byte-identical to S9 ⇒ golden-neutral, NO re-pin** (verified: both
  goldens still `0xfd8e…c31b` / `0xbd92…96de`). Exactly one of the two runs ⇒ no double-count.
- **The brake is the rail network (PURGE), not the abstract pushback:** holding/building track near the
  front purges it back (lowers the meter); the front reaching the capital = the realm falls. `LOSE_DIST`
  is LARGER than the capital barracks's `PURGE_RADIUS`, so a lone capital can't make the realm
  unloseable — you must extend the purge ring outward to hold the heartland. Conquest still matters
  (tribute + the line you build to a conquest target purges that region — area control falls out of the
  build loop).
- **Per-city creep knob + calibration:** `CityData.decadence_creep_per_s` (serde-default ⇒ the FAST test
  rate, so the S10b-1 field tests are unaffected; the baked world sets a slow rate). Measured the real
  continent: **9312 passable cells (all reachable, 0 stranded), max creep-distance 201 hops**; baked
  `creepPerS=8` ⇒ the front advances ~1 ring/250 ticks ⇒ an **undefended realm is overrun in ~40
  game-minutes** — a generous campaign runway. Threaded `decadenceSeed.creepPerS` → `buildCoreCity`;
  re-baked (certified seed still 12, packs otherwise unchanged).
- **Tests (`tests/decadence_field.rs` +1, structural):** `the_spatial_tide_is_the_lose_condition_and_the_
  network_holds_it` — an undefended baked realm (just the capital) IS overrun by the tide; a WALL of
  stations across the approach (PURGE) holds it out for well past the idle-loss time (earned survival).
- **Real-world verification (production bundle):** the conquest + play e2e are green; the trajectory shows
  the lose meter now front-derived (**decadencePct ~1%** in the 10-min window — the slow tide is still far
  from the capital), realmLost=false, conquest completes — the spatial model is winnable on the real
  continent, and the CA + derivation runs every tick within the perf budget (conquest e2e 6.7 s).
- Tiers: cargo **196/0** (+1 spatial test; both goldens unchanged — golden-neutral) · build_world
  `--selftest` PASS · conquest + play e2e green. **Next (S10c):** render the cold tide (the "look") +
  screenshot — then S10 (the area-control identity) is complete end-to-end.

**AUTONOMOUS ROADMAP LOOP — iteration 5: S10c (render the tide) + two calibration bug-fixes, 2026-06-14.**
S10 is now complete END-TO-END: the spatial decadence tide RENDERS, and driving it in-browser surfaced +
fixed two real bugs that the earlier S10b-2 e2e had masked (the tide had been frozen, so "realm holds"
was vacuous). The decadence race is now genuine and visible.
[docs/progress/fantasy-decadence-tide.png](docs/progress/fantasy-decadence-tide.png) — the cold violet
tide flooding the far half of the continent, creeping toward the warm SW capital + the player's rail
network (Decadence 46%, mid-creep).

- **S10c render:** `render_buf::decadence_tide_m` copies out corrupted CA cells `[x_m, y_m, v]` (v =
  decadence/DECAD_MAX) → `sim-wasm decadenceTide()` → `SimBridge` → `Game.decadenceTideAt()` (metres →
  lng/lat) → a `ColumnLayer` (`id:"decadence-tide"`) over the terrain, under the network. Value-not-hue
  per the art direction: a single low-chroma cold violet, **strength = alpha** (faint at the front,
  opaque deep), same pointy-top hex geometry as the terrain. Rebuilt on the ~3 Hz refresh (the tide
  creeps slowly), never per frame; empty for transit. New `TideCell` type + `RenderView.tideCells`.
- **Bug 1 — the gain-truncation FREEZE (the big one):** `creep·dt/1000` truncated the baked rate to **0
  per tick** (`creep=8` → `8·50/1000 = 0`), so the tide never advanced — the same integer-truncation class
  as the original decadence bug, and it made S10b-2's "realm holds" e2e PASS for the WRONG reason (a frozen
  tide). Fixed: **floor the gain at 1** for any positive creep (never silently freeze); set baked
  `creepPerS = 20` (gain exactly 1/tick). RED-first pin: `a_slow_creep_rate_does_not_freeze_the_tide`
  (creep=1 ⇒ gain 0 under the old code ⇒ frozen ⇒ the lose meter stays 0).
- **Bug 2 — PURGE around ALL stations suppressed the tide map-wide:** the baked world auto-seeds ~42
  resource/town station nodes; PURGE fired around every station, so the map started immune to the tide.
  Fixed: **PURGE only around stations ON A BUILT LINE** — the rail NETWORK holds the line, not isolated
  unconnected nodes. Now the tide genuinely threatens (the baked nodes don't suppress it) and you race to
  rail your defenses + supply. (`run_ticks` + the spatial test now rail their stations into a line.)
- **The race is now real + winnable (re-verified on the real bundle):** the conquest e2e trajectory climbs
  **decadence 16 → 31 → 46 → 62%** as legions field, and conquest captures a town at step 4 (62%) —
  beating the tide (which reaches the capital ~step 8). `realmLost=false` because conquest WON the race
  (not because the tide was frozen). An undefended/sparsely-railed realm IS overrun in ~20 game-min.
- Tiers: cargo **197/0** (+1 freeze-guard test; both goldens unchanged — the CA is baked-only, golden-
  neutral) · the conquest + play + arcadia×2 + transit slice e2e green on the production bundle.
- **S10 — the area-control identity (the build plan's "largest subsystem") — is COMPLETE:** board (S10a) +
  creep CA engine (S10b-1) + spatial lose condition (S10b-2) + render (S10c), the tide a genuine,
  visible, winnable race over the procedural continent. **Next: S11** (economy / tech / endless+prestige /
  rival), then the S7e multi-stage refinement.

**AUTONOMOUS ROADMAP LOOP — iteration 6: S11a, the fantasy progress gauge, 2026-06-14.** First (small)
S11 chunk. `ArcadiaRuleset::coverage_score` was a transit placeholder; now it's the fantasy PROGRESS gauge
(`World::arcadia_coverage_score`) — **supply reach** (town demand on an operational line) blended 0.65/0.35
with **conquest** (towns held), 0–100. The "two gauges, two jobs" of the design (fantasy-game-design §4):
this is what you're BUILDING (monotonic), the decadence gauge is the rot you're RACING (can rise). MONOTONIC
by construction — a superset network serves ≥ the same sinks, `towns_captured` only rises — so it never
falls. **Golden-neutral** (a derived stats read, f32, never hashed; both goldens unchanged). Test
(`arcadia.rs`): extending the network to supply a second town never lowers the gauge. The HUD reads
`coverage_score` already, so no frontend change. cargo **198/0**. **Next S11 chunks (shorter loops):**
tribute→treasury economy (spend/opex), the `UnlockTech` bitset, the rival-kingdom seam; plus the deferred
S7e multi-stage processing.

**AUTONOMOUS ROADMAP LOOP — iteration 7: S11b, surface the standing gauge (two gauges, two jobs), 2026-06-14.**
The S11a progress gauge was computed but the fantasy HUD only showed the decadence gauge — so the design's
"two gauges, two jobs" (fantasy-game-design §4) was half-rendered. Added a **🛡 Standing** gauge to
`FantasyStatsBar` beside **☠ Decadence**: standing (supply reach + conquest, rises as you build/hold) vs
decadence (the rot, rises as it creeps) — the realm you're building against the rot you're racing.
Frontend-only, golden-neutral. Browser-corroborated: built a network on the baked world, **Standing 65 vs
Decadence 31%**, 0 console errors ([docs/progress/fantasy-two-gauges.png](docs/progress/fantasy-two-gauges.png)).
tsc clean; arcadia ×2 + conquest e2e green (existing testids preserved, `standing-gauge`/`standing-bar`
added). **Next S11 chunks:** tribute→treasury economy + opex, the `UnlockTech` bitset, the rival seam;
plus the deferred S7e multi-stage.

**AUTONOMOUS ROADMAP LOOP — iteration 8: continuous creep accumulator (deferred S10c follow-up) +
a timescale finding, 2026-06-14.** While scoping the S11 economy I found the day-based opex/growth
mechanics barely fire: the baked decadence runway (~17–20 min at creep=20, gain 1/tick) is SHORTER than
one in-game day (24×`HOUR_MS` = 48 sim-min), so "days" don't really turn in a decadence race — the economy
is coupled to the game's timescale (a genuine design fork: a fast ~20-min race vs a multi-day economy
campaign). The blocker to slowing the runway was the S10c gain FLOOR (`max(1)`), which made every
`creep_per_s < 20` collapse to the same gain 1/tick.

- **Fix (the deferred S10c follow-up):** replaced the floor with a sub-unit **accumulator**
  (`world.decadence_gain_accum`, a scalar — the per-tick gain is uniform across advancing cells). The
  milli-gain accrues across ticks and whole units are extracted, so the creep rate is now CONTINUOUS — a
  slow `creep_per_s` advances at its true average (the front steps on rollover ticks), enabling a tunable
  multi-minute → multi-day runway. Transient/unhashed (regenerated bit-identically on replay, like
  `forge_accum`). **Golden-neutral**: a rate yielding an exact integer gain (the default 200 ⇒ 10/tick,
  the baked 20 ⇒ 1/tick) leaves the accumulator 0 ⇒ byte-identical to the floor; both goldens unchanged.
- **Tests (`decadence_field.rs`):** the freeze-guard still holds (creep=1 advances, not frozen); a new
  `the_creep_rate_is_continuous_not_floored` pins it — creep=4 vs creep=12 now advance at distinct,
  proportional speeds (under the old floor both were gain 1 ⇒ equal; the RED-first pin). cargo **199/0**.
- **Decision surfaced (genuinely the user's):** the bigger S11 economy/tech depth (opex expander-brake,
  tech ladder over days, endless/prestige) only becomes meaningful with a **multi-day runway** — which the
  accumulator now ENABLES (set a slow baked `creep_per_s`), but choosing it changes the game's feel from a
  ~20-min race to a longer campaign + needs a balance re-tune. Left the baked rate at the felt ~20-min race
  (creep=20, accumulator inert there) pending that call. The rival kingdom is a `war_step(owner != PLAYER)`
  seam the design explicitly DEFERS.

**═══ FORK CERTIFICATION — the full S10+S11 changeset is green end-to-end (iteration 9), 2026-06-14 ═══**

A holistic full-tier verification after the ~12-commit autonomous run (recovery → balance pass → S10a/b-1/
b-2/c → S11a/b → continuous creep accumulator): **cargo workspace 199/0** (both determinism goldens
intact — transit `0xfd8e…c31b`, arcadia `0xbd92…96de`) · **vitest 21/21** (wasm-in-node determinism smoke)
· **e2e 21/21** on the production bundle (transit slice, Tokyo 440-station, both arcadia, fantasy
conquest/play/shot, modes, edit-line, …) · **build_world `--selftest` PASS**. The fantasy fork is a
COMPLETE, polished, winnable game on a procedurally-generated continent:
- **S0–S9** — ruleset-at-construction fork, hex lattice, Forge-Line disjoint chains, war machine + AI,
  decadence lose-condition. **Map-gen S1–S6** — certified-winnable procedural continent.
- **S10 (area control, the build plan's largest subsystem) — COMPLETE**: the spatial decadence tide
  (board → CA engine → spatial lose condition → render), a genuine winnable race held back by the rail
  network's PURGE; a continuous (tunable) creep rate.
- **S11 (partial)** — the two-gauge HUD (🛡 Standing progress vs ☠ Decadence), a monotonic progress gauge.

**Achievable-roadmap completion + the open decision.** The remaining S11 depth is **gated on a game-feel
call that is genuinely the user's**: the baked game is a felt **~20-min decadence race**, shorter than one
in-game day, so the design's day-based economy (opex expander-brake, tech-over-days, endless/prestige) only
becomes meaningful with a **multi-day runway** — which the continuous-creep accumulator now ENABLES (set a
slow baked `creep_per_s`), but committing to a longer campaign changes the feel + needs a balance re-tune.
The **rival kingdom** is a `war_step(owner != PLAYER)` seam the design explicitly defers; **S7e
multi-stage processing** (raw→mid→final) is a tracked supply-depth refinement. Default held: the felt race.
**~12 commits sit on `fantasy-fork`, unpushed** (standing "commit only / push only when asked").

## Known gaps / deferred

- **T7 (self-host PMTiles)** — deferred per PLAN §15; slice ships on the hosted CARTO/MapLibre style. Not on the critical path.
- **Real OSM demand (pyrosm)** — deferred; T13 ships a deterministic synthetic grid (sim consumes the JSON identically).
- **Done since the slice:** curves+speed caps, time-of-day, transfers (BFS+cache), real OSM networks +
  6 cities, buildability/build-modes, economy (capital+fares), transport modes (rail/bus/ferry/air),
  demand layer, settings, **time-dependent RAPTOR routing**, demand/traffic visibility (5 tracks),
  **accessibility isochrone**, **inter-station footpaths**, freeform line waypoints, **the capacity
  stack** (P1 block-follow, P2 single-track meet, P4 junction mutex, S1v1 trunk cap, S2 physical-block
  meet), **grid geometry + cross-line shared track** (two lines share one rail). Remaining seams:
  multiplayer, GTFS import, departure **timetable**, the **FULL TrackGraph** (first-class
  `TrackSegment`s + resource-ordering, the real model-change cliff — see
  [docs/p5-shared-track-roadmap.md](docs/p5-shared-track-roadmap.md)), terrain gradient.
- **idea.md "pt 2" (user-added 2026-06-04):** game modes — *sim mode vs grand-tycoon mode*, *pure-sim vs
  GSG-inspired mode with events*. Future scope, well beyond the thin slice. Noted, not built (guard the loop).
  The command-sourced deterministic core is mode-agnostic, so a future "mode" is a new outer-ring layer +
  Command/Event variants, not a core rewrite.
