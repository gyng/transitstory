# What's extractable from OpenStreetMap (transitstory)

Inventory of transit-relevant OSM data + Overpass patterns + how each maps into the game.
Everything is pulled **offline** into committed JSON (no runtime Overpass), like
`scripts/build_networks.py` / `build_buildability.py` / `build_demand.py`. Keep builders
dependency-light (urllib + json) and re-runnable (per-city try/except, keep-existing-on-fail).

## Networks (lines + stations) — `route` relations
| Mode | OSM | Game mapping |
|------|-----|--------------|
| **Rail** (done) | `relation[type=route][route~subway\|light_rail\|monorail\|tram]`, grouped by `route_master`/ref; ordered `role=stop` node members = stations; `colour` tag = line colour | one `NetLine` per route (longest variant); stations deduped by name → shared index = interchange |
| **Bus** | `route=bus` (+ `trolleybus`/`minibus`); same member structure; `interval` (ISO8601 `PT15M`) → headway | same path — just widen the importer regex + tag `mode:"bus"`. Often no colour (palette). Huge in big cities → cap by bbox/operator |
| **Ferry** | `route=ferry`; stops = `amenity=ferry_terminal` / `man_made=pier` nodes; standalone `way[route=ferry]` for short hops | `NetLine mode:"ferry"`; terminals deduped → interchanges; runs over water |
| **Air** | `aeroway=aerodrome` (`out center`) with `iata` present; `aeroway=terminal` centroid = the station | **not a route relation** — synthesize 2-stop `mode:"air"` lines between airports in the build script |

**Loops:** `roundtrip=yes` / `circular=yes`, or geometric `stops[0]==stops[-1]` (roundtrip is often unset — use the geometric heuristic). → `loop=true` (drop the duplicate last stop).
**Filtering (in-service only):** skip `route=construction|proposed`, and any element whose tag **key** matches `^(construction|proposed|planned|disused|abandoned|razed|demolished):` — including individual `construction:railway=station` stop nodes inside an otherwise-open route.

## Demand proxies (origin/dest weights)
- **Landuse polygons** (primary, cheapest, uniform): `landuse=residential` → originWeight; `commercial`/`retail`/`office` → destWeight; `industrial` → light dest. Area-in-cell accumulation.
- **POIs** (sharpen job centres): `amenity` (school/university/hospital/mall…), `shop`, `office`, `tourism=hotel/museum` — type-weighted count per cell → destWeight. Cap per-cell so an over-mapped block can't dominate.
- **Buildings** (higher fidelity, heavier): `building` + `building:levels`/`height` → floor-area ≈ footprint × levels; residential→origin, else→dest. 100k+ ways in a metro → tile or gate behind a small bbox; OPTIONAL refinement over the landuse base.
- **Population** (`place`/`boundary` `population=*`): a scalar normalizer only (don't spread uniformly). Sparse/stale; fall back to per-city max-cell=1.0 relative scaling (the sim's `DEMAND_RATE_PER_MS` scales globally).
- Output the existing `{cellM,bbox,cells:[{lon,lat,originWeight,destWeight}]}` schema (a drop-in superset of the synthetic grid). Don't pre-apply decay — `demand.rs::prepare` does gravity catchment + normalization.

## Coastline / sea (the surface-rail water gate) — **done**
- `natural=coastline` is a **directed open way** (land on the LEFT, sea on the RIGHT), not a closed polygon — don't point-in-poly it. `natural=water`/`water=*` polygons are separate (inland water).
- Approach (implemented in `build_buildability.py`): rasterize coastline as a wall, seed sea cells on the right of each segment, BFS flood open cells → `WATER`. Landlocked cities (no coastline) skip the flood. Islands keep their enclosed interior as land. (`PROGRESS.md` sea-gap: closed.)

## Placement gates (per mode) — generalizes the buildability grid
Keep `WATER(4)` as the **rail** hard-gate; the same coarse 120 m grid serves all modes:
- **Bus**: widen the ROAD layer to `motorway..residential|service|busway`; on-road = legal/cheap, off-road = penalized.
- **Ferry**: invert — WATER **required**, stops must touch a `ferry_terminal`/`pier` cell.
- **Air**: an aerodrome point set; endpoints must be airports; exempt from water/road gates.
`crosses_water_surface` in the sim is just the first instance of a general per-mode illegal-cell check.

## Heatmap / map layers
A deck.gl `HeatmapLayer`/`GridCellLayer` fed by the committed demand cells (stable identity, built once — never per frame), `getWeight = originWeight + destWeight`, or two toggleable origin/dest layers. No runtime Overpass.

## Recommended architecture for modes
Make mode **additive**: a single `mode` field on `NetLine` + sim `Line` (`rail|bus|ferry|air`, default `rail`) + a per-mode `TrainsetSpec` preset (v_max/accel/dwell/capacity). Board/ride/alight, catchment, dispatch, routing, and the Catmull-Rom polyline stay identical — only the preset and the placement gate vary by mode. Bus/ferry import through the existing `build_networks.py` (widen the regex, tag the mode); air is a separate synth pass.
