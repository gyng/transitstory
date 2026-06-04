# onlytransits — Build Progress (flight recorder)

Live log of the autonomous overnight build of the Singapore vertical slice (per [PLAN.md](PLAN.md)).
Maintained at every checkpoint and every fallback taken.

## Milestone checklist

- [ ] **M0** Walking skeleton — repo + green tests in all 3 tiers (T1–T4, CP0)
- [ ] **M1** Singapore map renders & is interactive (T5, T6, CP3/CP4)
- [ ] **M2** WASM sim bridge proven (T8, T9, T13, CP2)
- [ ] **M3** Build tools — place stations, draw a line, assign a trainset (T10–T12, CP5)
- [ ] **M4** Live sim — vehicles run, passengers flow, ridership accrues (T14–T16b, CP6/CP7)
- [ ] **M5** Stats readout + full slice verified e2e (T17, T18, CP8)

### Task status
| Task | Title | Status |
|------|-------|--------|
| T1 | Scaffold monorepo + PROGRESS + lockfiles | in progress |
| T2 | Sim core + determinism replay test | pending |
| T3 | Vite shell + Vitest + Playwright load | pending |
| T4 | CI workflow | pending |
| T8 | sim-wasm wrapper (Sim::new/apply_command_json/tick/SoA) | pending |
| T9 | TS SimBridge + wasm-in-node smoke | pending |
| T13 | Synthetic demand grid + singapore_city.json | pending |
| T5 | MapLibre Singapore basemap + geo.ts | pending |
| T6 | deck.gl MapboxOverlay + test layer | pending |
| T10 | Place-station tool | pending |
| T11 | Draw-line tool | pending |
| T12 | Assign trainset + headway slider + SoA clamp | pending |
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

## Known gaps / deferred

- **T7 (self-host PMTiles)** — deferred per PLAN §15; slice ships on the hosted CARTO/MapLibre style. Not on the critical path.
- **Real OSM demand (pyrosm)** — deferred; T13 ships a deterministic synthetic grid (sim consumes the JSON identically).
- Multiplayer, GTFS import, other cities, transfers/RAPTOR K>1, junctions, fares, time-of-day — architectural seams only (PLAN §15).
