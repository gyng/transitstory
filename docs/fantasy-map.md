# The fantasy map — generation, topology, look

> **Status:** design, not yet built. The world/playfield for the hex 4X-logistics fantasy game.
> Companion to [fantasy-game-design.md](fantasy-game-design.md) (the game),
> [fantasy-build-plan.md](fantasy-build-plan.md) (the build), [fantasy-fork.md](fantasy-fork.md) (the
> architecture). Obeys [AGENTS.md](../AGENTS.md) + the [[dynamic-city-architecture]] frozen-bake rule.

## What it is

A **procedurally-baked sibling of the committed OSM city packs** — the same `cells[]` raster on the
same `i64`-mm grid, with the OpenStreetMap/Overpass source swapped for **layered noise** and the square
grid quantized onto a **hex lattice**. Generated from one `u64` **seed**, baked **offline**, **frozen
into the save** (`<world>_world.json`, a fourth pack beside `_city`/`_demand`/`_buildability`,
ingested by the same `Sim::new(seed, city_json)`). Never a live fetch, never a runtime generator →
replays stay bit-identical (terrain is static un-hashed data; only the decadence tide + armies are
simulated). **Prestige re-bakes a harder continent** (bigger radius = longer baseline supply lines,
aether farther/fewer, tide faster). The manifest carries `ruleset:"fantasy"` + `grid_cell_mm>0` (the
locked hex pitch) + a synthetic `originLngLat` (so MapLibre's camera has a frame; `geo.ts` collapses to
integer scale+offset) + the terrain in `buildability.cells[].c` + an additive `supply_graph`/`towns`
field (serde-ignored by transit).

**Terrain gates everything** (the disjoint-chain driver): ORE on mountain/hill, GRAIN on plain, FUEL on
forest/peat, AETHER only on rare ley-ridges. **BREAD** (GRAIN+FUEL → feeds towns) and **ARMS**
(ORE+AETHER → arms legions) draw **separate chains**, and the bake puts grain-country and ore-country in
*different directions* from the capital — you can't feed people and arm soldiers from one spur, and the
bottleneck **moves with the map** (ore-rich seed = fearsome legions / starving towns; grain-rich =
fed / under-armed).

## Generation = constrained procedural

Pure procedural bakes degenerate seeds that "succeed" (sealed aether, unwinnable start); a room-grammar
is overkill (towns/resources are *point* objects). So **noise for the continuous fields, inside five
authored scaffolds**:

1. **Capital anchored** to a corner quadrant and *carved* buildable, GRAIN+FUEL guaranteed in first-cart
   reach (BREAD bootstraps turn one).
2. **Passes carved** — deterministic A* between basins demotes the cheapest mountain corridor to hill,
   so every region is reachable through a narrow chokepoint *by construction*, never a sealed wall. *The
   single most load-bearing step.*
3. **Aether hard-capped** (3–6 nodes) placed by template (far quadrant, beyond a pass) — scarcity by
   construction, not a knife-edge noise threshold.
4. **Two attractor centers** (breadbasket lowland + ore highland) with an authored min-separation — the
   disjoint chains are geographically separable per seed.
5. **A solvability validator + a bounded relaxation ladder** (soften the softest constraint first,
   deterministic order) — the bake always terminates and stays a pure function of the seed.

What does NOT need authoring: the elevation/moisture/ley noise itself (layered value/simplex on the
axial grid — cheap, deterministic in pure Python, dialable; not an erosion/plate sim).

### The pipeline (`scripts/build_world.py`, deterministic, keyed `ChaCha8(seed ^ MAP_CONST)`)
- **S1 — terrain + continent + carved passes:** four decorrelated noise fields (ELEVATION, MOISTURE, a
  thresholded thin-ridge LEY field, a radial CONTINENT MASK biased to the capital corner). Mask → land
  vs WATER(=4, the free existing rail gate); **flood-fill from the capital and delete disconnected
  islands** (the contiguous-continent guarantee). Highest band = impassable RIDGE; **carve passes** (A*
  between basins). Trace steepest-descent RIVERS to sea (block rail) with a few FORD chokepoints. Biome
  = lookup on (elevation, moisture) → new class codes 6=MOUNTAIN/7=HILL/8=FOREST/9=LEY/10=PLAIN.
- **S2 — resources (terrain-gated):** stamp ORE/GRAIN/FUEL/AETHER only on qualifying biomes, then
  Poisson-disk rarefy to a baked budget; bias each toward its attractor center. Yields ride
  **i64-quantized** in `supply_graph.resources[]` + bump demand-grid `dest_w` (reuse catchment capture).
- **S3 — capital + starter + neutral towns:** capital anchored+carved (validated buildable +
  grain/fuel-reachable); starter town SOUTH at first-cart reach on the near side of the first pass;
  neutral "good"/decadent towns Poisson-scattered, snapped near resource clusters, each a 2–3-good
  demand set, graded by rail-distance into the **expansion arc** (near = easy, aether-adjacent far =
  late prizes). Town value **i64-quantized**.
- **S4 — decadence seed:** neutral towns start at a baked decadence floor; the far edge opposite the
  capital is the high-decadence **reservoir** (pointed the same way as the richest aether → "race the
  tide to the arcane"); pre-bake a coarse **creep-distance-to-capital potential** (BFS) so runtime
  diffusion has a cheap gradient toward the capital (the telegraph + lose condition) without per-tick
  pathfinding; capital baked clean with a grace buffer; a passable corridor to the capital is guaranteed
  (a walled-off capital = unloseable = asserted against). Edge cells = raider spawn anchors.
- **S5 — hex-quantize:** axial (q,r) → `i64` mm once at bake (pointy-top), assert lattice alignment.
  Each terrain hex = a `BuildCell{x_mm,y_mm,c}` → the buildability grid IS the terrain map, rendered by
  the existing `ColumnLayer diskResolution:6` honeycomb. Nothing new crosses `geo.ts`.
- **S6 — solvability validator + relaxation ladder:** assert + re-roll (bounded, deterministic order):
  capital pocket buildable; **GRAIN+FUEL reachable with aether-grade rigor** (a forest-poor seed
  silently starves the BREAD/manpower loop); every resource reachable through a pass; ore/grain
  spatially separated (tied to the supply-distance governor); no cornucopia hex; aether beyond a min
  start-distance; a decadence corridor exists; the start isn't a 1-hex funnel (min-corridor-width BFS).

## Two engineering catches
- **New biome class codes alone do nothing** — they hit `world.rs`'s `_ => 0` (block/cost nothing). An
  **additive sibling field carries all fantasy cost/impassable/yield**; class codes are render tint +
  the free `WATER=4` reuse. Any touch to the `world.rs` cost gate is a golden-hash re-pin, RED-first.
- **i64-quantize** all yields/town-values at bake (the gate-blind-defect-6 discipline — an f32 weight is
  a cross-build divergence channel).

## Open decisions (deferred to S10 / the harness)
- **Decadence CA grid:** share the terrain hex grid (cheapest) vs a finer sub-grid (if the creep looks
  blocky). Gates the `grid_cell_mm` freeze → decide at S10's per-tick bench, not before.
- **`grid_cell_mm` + continent scale:** size conservatively **low** (~12–25k passable hexes) until S10
  — the static render budget is proven (Singapore 62k), the dynamic decadence-bloom CA cost is not;
  freezing generously then hitting the cliff forces re-baking every world pack.
- **Coupled balance knobs** (ore↔grain separation, expansion-arc gradient, creep rate, town
  resolve/garrison gradient): co-tuned by the **headless balance harness** — the bake guarantees
  *structure*; only the sweep certifies *feel*.

## Research-grounded pipeline refinement (deep-research, 2026-06-14 — sources below)

A sourced, adversarially-verified survey (Red Blob Games/Patel, Martin O'Leary/mewo2, Musgrave SIGGRAPH '89,
mapgen4, AAAI'12 path-constraints) **validated the constrained-procedural split in the canon's own words** (Patel
mapgen2: a graph for gameplay-constrained features + noise only for cosmetic variety) and sharpens the S1–S6 bake:

- **Elevation backbone = a distance-from-coast field** (ADAPT), not raw noise. Monotonic away from the coast ⇒ **no
  interior local minima ⇒ downhill-to-sea is free** (what makes rivers work). It *also* hands us the expansion-arc
  topology for free: capital corner low/coastal, interior mountainous, aether pushable to the far high-distance
  interior. (BFS/Dijkstra distance transform on the hex graph — integer, pure-deterministic.)
- **+ redistribution (power curve `pow(e,exp)`) + heterogeneous/multifractal roughness** (BRING-IN/ADAPT) for shape —
  the power curve **tunes how much flat buildable land exists** (logistics needs flat corridors + ridge chokepoints);
  heterogeneity makes the capital corner gentle and the far interior jagged. The biggest "looks real vs designed"
  lever after rivers. (Replaces plain fBm in the backbone-perturbation step.)
- **Light erosion + iterated sink-fill** (ADAPT — mewo2-style `√flux·slope` fluvial + `slope²` thermal + Planchon-
  Darboux fill; **NOT a heavy PDE sim — refuted as impractical**). Carves believable valleys (rail corridors) AND the
  sink-fill **guarantees** the downhill-to-sea property. Float offline, quantize the heightfield to i64-mm before
  freezing; fixed seeded iteration count.
- **Rivers = flow-accumulation drainage TREES** (BRING-IN — the lynchpin): route water downhill, accumulate flux
  child→parent to the sea, draw where flux > a land-fraction-normalized threshold, width ∝ √flux; tributaries MERGE,
  never split. Gives **fords** (low-flux = cheap crossing), **valleys** (rail corridors), **rail-routing cost**
  (high-flux = expensive to cross), **city sites** (confluences), and a **connected barrier** the decadence front +
  chains must respect — believability cue and gameplay chokepoint are the *same* near-free feature. Flux is float
  offline → quantize the river-width/flow class to i64-mm; *which edge is a river* is a discrete deterministic fact.
  Render as a static layer + a **smooth polyline over the cells** (not hex-stepped). Compute AFTER elevation+sink-fill,
  BEFORE siting.
- **Biomes = Whittaker elevation×moisture** (BRING-IN), moisture partly **river/coast/rain-shadow-derived**, not pure
  independent noise. Biome×elevation IS the deterministic resource-placement mask that geographically separates the
  two chains around the attractor centers, and "wet near rivers, dry behind mountains" explains *why a resource is here*.
- **Solvability = the AAAI "path constraints" approach** (BRING-IN) — **upgrade the validator from generate-and-reroll
  to a *constructive* guarantee**: encode "capital reachable," "a pass exists," "both chains completable," "aether
  obtainable before the decadence front arrives" as path/reachability constraints the solver satisfies *before*
  emitting a map (the relaxation ladder remains the fallback). Pair with suitability-scored siting + named regions for
  lived-in coherence.
- **Noise is cosmetic-only** (SKIP as backbone): it's purely local and cannot guarantee a global feature (a river
  peaks-to-ocean, a reachable capital). Use fBm only for coastline wiggle / moisture texture / scatter — the LAST
  dressing layer.
- **Hex diverges from the canon's Voronoi mesh but is a STRENGTH** for our genre (uniform rail cost, clean adjacency)
  and determinism (no point-jitter to reproduce — a fixed integer lattice is bit-exact-reproducible). The graph
  algorithms (distance field, downhill, flux, sink-fill) port to hex unchanged. **Mitigate hex-regularity-looks-
  artificial** with: heterogeneous noise + erosion (break grid alignment), smooth-polyline coastlines/rivers (not
  hex-stepped edges), and never aligning features to the lattice axes.
- **SKIP**: a heavy PDE hydraulic-erosion sim (refuted — ~100 params, impractical) and a Dwarf-Fortress
  tectonics→civ→history sim (overkill — suitability siting + names get "lived-in" far cheaper).

**Sources (primary, 3-0 verified):** [polygon-map-generation](http://www-cs-students.stanford.edu/~amitp/game-programming/polygon-map-generation/)
· [terrain-from-noise](https://www.redblobgames.com/maps/terrain-from-noise/) · [mapgen4](https://www.redblobgames.com/maps/mapgen4/)
· [mewo2 terrain](http://mewo2.com/notes/terrain/) · Musgrave et al. SIGGRAPH '89 erosion · Horswill & Foged, AAAI'12 path-constraints.

## The look (art direction, recap)
Muted ash-grey hex vellum; terrain in **value not color** (plains→hills→mountains rise; passes are
visible gaps; ridges near-black; ley a faint violet — the only ground chroma, the aether prize). Your
**capital + dominion = the only warmth** on a dead world; neutral "good" towns read as *sickly
cold-bright* (benevolent-looking, corrupt); the **decadence tide** is a low-chroma cold creep (strength
= alpha, CB-safe) advancing from the far edge toward your gold. Captured ground tints warm but terrain
value reads through it; the frontier is a dashed animated boundary; the rear stays calm. Menacing in the
chrome, benevolent in the motion: a dead grey world with a warm network metastasizing out of one corner,
racing a cold tide to the arcane.
