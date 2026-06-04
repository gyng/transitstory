<p align="center">
  <img src="docs/icon.png" alt="transitstory mascot — a pigeon in a transit conductor's cap" width="160" />
</p>

# transitstory

A web-based 2D top-down transit-network builder game on a real OpenStreetMap map
(NIMBY Rails / OpenTTD / A-Train / Mini Metro lineage). Deterministic Rust→WASM
simulation core + TypeScript/Vite frontend (MapLibre GL JS + deck.gl). First
playable target: a Singapore vertical slice.

See **[PLAN.md](PLAN.md)** for the build plan and **[AGENTS.md](AGENTS.md)** for the
engineering & design conventions. **[PROGRESS.md](PROGRESS.md)** is the live build log.

## Architecture (the spine)

Concentric rings, dependencies point strictly inward:

```
packages/app  (TS, MapLibre, deck.gl)   ->   crates/sim-wasm  (wasm-bindgen facade)   ->   crates/sim  (pure deterministic core)
```

- `crates/sim` — pure deterministic simulation. No IO, wall-clock, threads, wasm, float-Mercator, or HashMap iteration.
- `crates/sim-wasm` — the only wasm-aware crate; a thin translation membrane, no game logic.
- `packages/app` — the web frontend; `coords/geo.ts` is the single lng/lat ⇄ metres ⇄ mm boundary.

## Quickstart

```bash
# Prerequisites: Rust 1.94 (+ wasm32-unknown-unknown), wasm-pack, Node 24, pnpm 10
rustup target add wasm32-unknown-unknown

pnpm install
pnpm build:wasm     # compile the Rust sim to WASM into packages/wasm-sim
pnpm dev            # start the Vite dev server
```

## City data (baking an area)

Each playable city is **baked offline into committed JSON** — the game never calls Overpass at
runtime; it only reads these files (the `CityData` seam in AGENTS.md). One config drives everything:
`scripts/city_demand_config.json` (per-city `bbox` + demand `jobCenters`/`homeCenters`).

```bash
scripts/build_data.sh                  # bake every city in the config
scripts/build_data.sh istanbul dublin  # just these
DEMAND_ONLY=1 scripts/build_data.sh    # offline only (skip the OSM stages)
```

`build_data.sh` runs three stages, all writing to `packages/app/public/data/`:

| Stage | Script | Output | Source | Notes |
|------|--------|--------|--------|-------|
| 1 | `build_demand.py` | `<id>_demand.json` | synthetic (Gaussian bumps) | **offline**, deterministic (seeded) |
| 2 | `build_networks.py` | `networks/<id>.json` | OSM `route` relations | **online** (Overpass); keeps committed JSON on failure |
| 3 | `build_buildability.py` | `<id>_buildability.json` | OSM landuse/water/rail/road | **online** (Overpass); keeps committed JSON on failure |

Stages 2–3 are failure-safe: a network blip leaves the previously committed data in place, so the
build never breaks. **Add a new area:** add a `bbox` + demand-centre block to the config, run
`scripts/build_data.sh <id>`, then add a `<id>_city.json` manifest and a `CITIES` entry in
`packages/app/src/sim/cities.ts`. See **[docs/osm-data.md](docs/osm-data.md)** for what each OSM layer
contains and how it maps into the game.

Shipping cities: Singapore, Tokyo, Calgary, Istanbul, New York (Manhattan), Dublin.

## Tests (three tiers)

```bash
cargo test --workspace --release          # sim unit + determinism replay + property tests
pnpm --filter app run test                # Vitest: TS logic + wasm-in-node smoke
pnpm --filter app exec playwright test     # e2e against the production bundle
```

## License

MIT (code). Map data © OpenStreetMap contributors (ODbL) — see [ATTRIBUTION](ATTRIBUTION).
