# onlytransits — Build Progress (flight recorder)

Live log of the autonomous overnight build of the Singapore vertical slice (per [PLAN.md](PLAN.md)).
Maintained at every checkpoint and every fallback taken.

## Milestone checklist

- [x] **M0** Walking skeleton — repo + green tests in all 3 tiers (T1–T4, CP0) ✅
- [x] **M1** Singapore map renders & is interactive (T5, T6, CP3/CP4) ✅
- [x] **M2** WASM sim bridge proven (T8, T9, T13, CP2) ✅
- [x] **M3** Build tools — place stations, draw a line, assign a trainset (T10–T12, CP5) ✅
- [ ] **M4** Live sim — vehicles run, passengers flow, ridership accrues (T14–T16b, CP6/CP7)
- [ ] **M5** Stats readout + full slice verified e2e (T17, T18, CP8)

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
| T14 | Vehicle movement (Rust) | pending |
| T15 | Fixed-timestep animation loop | pending |
| T16a | Catchment capture + spawn | pending |
| T16b | Board/ride/alight + stats + coverage | pending |
| T17 | Stats bar + coverage gauge + waiting dots | pending |
| T18 | End-to-end slice Playwright spec | pending |
| T7 | Self-host PMTiles (DEFERRED unless time) | deferred |

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

## Known gaps / deferred

- **T7 (self-host PMTiles)** — deferred per PLAN §15; slice ships on the hosted CARTO/MapLibre style. Not on the critical path.
- **Real OSM demand (pyrosm)** — deferred; T13 ships a deterministic synthetic grid (sim consumes the JSON identically).
- Multiplayer, GTFS import, other cities, transfers/RAPTOR K>1, junctions, fares, time-of-day — architectural seams only (PLAN §15).
- **idea.md "pt 2" (user-added 2026-06-04):** game modes — *sim mode vs grand-tycoon mode*, *pure-sim vs
  GSG-inspired mode with events*. Future scope, well beyond the thin slice. Noted, not built (guard the loop).
  The command-sourced deterministic core is mode-agnostic, so a future "mode" is a new outer-ring layer +
  Command/Event variants, not a core rewrite.
