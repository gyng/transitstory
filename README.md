# onlytransits

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

## Tests (three tiers)

```bash
cargo test --workspace --release          # sim unit + determinism replay + property tests
pnpm --filter app run test                # Vitest: TS logic + wasm-in-node smoke
pnpm --filter app exec playwright test     # e2e against the production bundle
```

## License

MIT (code). Map data © OpenStreetMap contributors (ODbL) — see [ATTRIBUTION](ATTRIBUTION).
