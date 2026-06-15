#!/usr/bin/env python3
"""Build a FANTASY WORLD pack — the procedurally-baked sibling of the OSM city packs (docs/fantasy-map.md).

A deterministic OFFLINE bake (one u64 seed -> frozen JSON), the hex-4X-logistics playfield. Layered noise
on a pointy-top hex axial lattice, inside authored scaffolds (constrained-procedural, per the research
synthesis in docs/fantasy-map.md). Output is STATIC un-hashed terrain ingested by the same
`Sim::new(seed, city_json)` -> replays stay bit-identical (only the decadence tide + armies are simulated;
terrain never enters `state_hash`). Never a live fetch, never a runtime generator (the [[dynamic-city-
architecture]] frozen-bake rule).

This file is built in plan stages S1..S6 (docs/fantasy-map.md "The pipeline"). THIS COMMIT lands **S1 —
terrain + continent + carved passes**: four decorrelated noise/derived fields, a radial continent mask,
flood-fill island deletion (contiguous-continent guarantee), biome classification, and the load-bearing
**pass carve** (passable terrain is connected *by construction*). S2 resources, S3 towns, S4 decadence,
S6 validator attach at the marked hooks. The hex-quantize (S5) is folded in here (cells emit lon/lat that
re-project, via coords/geo.ts's equirectangular frame, back onto the exact `hexgrid::center_of` lattice).

Output (packages/app/public/data/):
  <id>_world.json         manifest {..., ruleset:"arcadia", gridCellMm, demandGridPath, networkPath?}
  <id>_buildability.json  {cellM, bbox, cells:[{lon,lat,c}]}  c: 4=WATER 6=MOUNTAIN 7=HILL 8=FOREST 9=LEY 10=PLAIN
  <id>_demand.json        {cellM, cells:[{x_mm,y_mm,origin_w,dest_w}]}  (S1: stub; S2/S3 populate from resources/towns)

Codes 6..10 are NEW biome classes: render tint only on the frontend; in crates/sim they hit world.rs's
`_ => 0` cost gate (block/cost nothing) until the additive fantasy cost/yield field lands (a golden re-pin,
RED-first) — see docs/fantasy-map.md "Two engineering catches". WATER=4 reuses the existing free rail gate.

Determinism: numpy PCG64 keyed by (seed ^ MAP_CONST); all float math is offline and quantized to i64-mm /
rounded lon/lat before freezing. Re-runnable: `python3 scripts/build_world.py [seed] [--selftest]`.
Dependency: numpy (already used by the OSM demand bake).
"""
import heapq
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "packages", "app", "public", "data")

# --- locked frame constants (mirror crates/sim/hexgrid.rs + coords/geo.ts so emitted lon/lat reproject
#     onto the exact lattice the sim quantizes back to) ---
SQRT3 = 1.7320508075688772          # hexgrid.rs SQRT3 (fixed literal, not a .sqrt() call)
M_PER_DEG_LAT = 110540.0            # geo.ts
M_PER_DEG_LNG = 111320.0            # geo.ts at lat 0 (cos 0 = 1); ORIGIN sits on the equator
ORIGIN_LNG, ORIGIN_LAT = 0.0, 0.0  # synthetic frame origin (manifest originLngLat); geo.ts collapses to scale+offset
MAP_CONST = 0x4D41_505F_5631       # "MAP_V1" — decorrelates the bake RNG from the sim seed

# --- bake parameters (provisional; docs/fantasy-map.md defers the grid_cell_mm freeze + continent scale
#     to S10's per-tick decadence-CA bench — size conservatively LOW until then) ---
W, H = 128, 128                     # axial extent (q in [0,W), r in [0,H)) -> 16384 cells, ~half land
GRID_CELL_MM = 250_000              # hex centre-to-corner size (= hexgrid size_mm); matches the arcadia demo
DEFAULT_SEED = 7

# biome class codes (docs/fantasy-map.md S1); 0=Open (unused here), 4=WATER reuses the existing rail gate
WATER, MOUNTAIN, HILL, FOREST, LEY, PLAIN = 4, 6, 7, 8, 9, 10
GLYPH = {WATER: "~", MOUNTAIN: "^", HILL: "n", FOREST: "T", LEY: "*", PLAIN: ".", 0: " "}

# --- S2 resources (docs/fantasy-map.md): terrain-gates the two DISJOINT supply chains. BREAD = GRAIN
#     (plains) + FUEL (forest) feeds towns; ARMS = ORE (hills) + AETHER (ley) arms legions. Each kind is
#     gated to a PASSABLE biome (so it's rail-reachable — never on impassable MOUNTAIN) and biased toward
#     one of two separated attractor centres (breadbasket lowland vs ore highland) so you can't feed people
#     and arm soldiers from one spur. Yields are i64 (the gate-blind-defect discipline — no f32 weights). ---
RESOURCES = [
    # name      biome    budget  min_spacing  base_yield  attractor   far_from_capital
    ("ore",     HILL,    8,      6,           100,        "ore",      False),
    ("grain",   PLAIN,   10,     7,           120,        "bread",    False),
    ("fuel",    FOREST,  8,      6,           90,         "bread",    False),
    ("aether",  LEY,     6,      2,           40,         "ore",      True),   # hard-capped ≤6, scarce, far
]
RES_GLYPH = {"ore": "O", "grain": "G", "fuel": "F", "aether": "A"}
# Forge-Line commodity index per resource kind — MUST match crates/sim/forge.rs (ORE=0, GRAIN=1, AETHER=2,
# FUEL=3). A source's demand cell carries this so the sim assigns the station's output commodity (S7e).
COMMODITY_IDX = {"ore": 0, "grain": 1, "aether": 2, "fuel": 3}
INGOT = 4  # a MID good (>= forge::FIRST_MID=4): a FORGE processes ORE → INGOT (S7e multi-stage)
# The two disjoint chains. BREAD = grain+fuel (2-stage) feeds towns. ARMS is now 3-STAGE (S7e multi-stage):
# ORE is mined → a FORGE forges it into INGOT → an ARMS town consumes INGOT + AETHER. So ARMS towns demand
# [INGOT, AETHER] (not raw ore), and the player must rail ore → forge → town. BREAD stays the 2-stage
# grain+fuel chain. Commodity indices MUST match crates/sim/forge.rs.
BREAD_RECIPE = [COMMODITY_IDX["grain"], COMMODITY_IDX["fuel"]]
ARMS_RECIPE = [INGOT, COMMODITY_IDX["aether"]]
# How many INGOT forges to site (in the corridor between the ore highland and the lowland ARMS towns).
FORGE_BUDGET = 4

# --- S3 towns (docs/fantasy-map.md): the supply SINKS (consume delivered goods → tribute) AND the
#     conquest targets. Suitability-sited near resource clusters (so they're worth taking + can be fed/
#     armed), Poisson-spread, graded by rail-distance from the capital into the EXPANSION ARC (near = easy
#     early prizes, far/aether-adjacent = late prizes). Town value i64. ---
TOWN_BUDGET = 9            # neutral towns (excl. the capital); the starter is the nearest of these
TOWN_MIN_SPACING = 9      # hex spacing between towns (room to build between them)
# Item #3 — the near-capital BOOTSTRAP cluster (FOR THE AI TO USE): the attractor-based deposits land ~90
# hexes from the SW-corner capital, so the realm can't mint manpower / field a legion before the decadence
# wins (and the #9 area-of-influence gate would soft-lock it). Force a small grain/fuel/ore cluster within
# first-cart reach of the citadel so a SHORT rail bootstraps the manpower the capital-barracks spends on its
# first legions. The far attractor deposits stay for the disjoint mid/late chains.
CLUSTER_MIN, CLUSTER_MAX = 8, 16  # the bootstrap ring (hexes): a satisfying FIRST rail, not jammed on the doorstep
CLUSTER_MAX_FALLBACK = 16         # if a biome is absent in the tight ring, search out to here (grain is essential)
CLUSTER_YIELD = {"grain": 120, "fuel": 90, "ore": 100}
TOWN_MIN_FROM_CAPITAL = 5  # don't spawn a neutral town on top of the capital
TOWN_MIN_FROM_RESOURCE = 3  # a town sits NEAR resources but never ON one — you must RAIL the goods in
TOWN_BASE_VALUE = 1000     # conquest reward floor
TOWN_ARC_VALUE = 50        # + per-hex-from-capital (the expansion-arc gradient: far = richer prize)
TOWN_GLYPH = {"capital": "@", "starter": "s", "neutral": "t"}

# --- S4 decadence seed (docs/fantasy-map.md): the corruption that makes conquest URGENT. Neutral towns
#     start at a decadence FLOOR graded by frontier depth (distance from the capital); the capital + a grace
#     ring start clean; the far edge opposite the capital is the high-decadence RESERVOIR + raider-spawn
#     anchors (the tide origin). The full per-tick creep CA is S10 — S4 just seeds the values + anchors and
#     asserts the capital is reachable from the reservoir (a walled-off capital would be unloseable). ---
CAPITAL_GRACE_HEXES = 6    # the clean buffer around the capital (no decadence floor here)
DECADENCE_BASE = 200       # a neutral town's floor just past the grace ring
DECADENCE_PER_HEX = 30     # + per hex of frontier depth (far towns are more corrupt)
RESERVOIR_ANCHORS = 5      # far-edge coastal cells = the tide origin + raider spawns
# Decadence GROWTH per sim-second on the baked continent — FAR gentler than the demo's 50/s (decadence.rs
# BASE_GROWTH): the continent is large (long supply lines, slow two-chain Liebig tribute), so the lose meter
# (threshold 20000) must fill over ~thousands of sim-sec to leave room to ramp up. Balance knob; the headless
# probe (e2e fantasy-conquest) certifies winnable. One captured town (pushback 300/s) then crushes the rot.
DECADENCE_GROWTH_PER_S = 6
# Legion MARCH speed (mm per sim-second) on the baked continent — FAR faster than the demo's 50 000 (army.rs
# ARMY_SPEED_MM_S). The continent's towns sit 60+ km from the capital; at the demo pace a legion takes ~21
# sim-min to reach the nearest town (just past a playable window), so conquest never lands. 200 000 (200 m/s)
# crosses 60 km in ~5 min — a few-minute legion for an epic continent, well inside the ~40-min decadence
# runway. Balance knob; tests/balance.rs `fantasy_baked_continent_is_winnable` certifies it conquers + holds.
ARMY_SPEED_MM_S = 200_000
# Decadence-tide CREEP rate (diffuse gain per sim-second) for the S10 spatial CA on the baked continent —
# FAR slower than the test default (200): at 20/s the integer gain floors to 1/tick, so the tide front
# advances one hex-ring per ADVANCE_THRESHOLD(=100) ticks; over the continent's ~200-ring span an
# undefended realm is overrun in ~17 game-minutes — an urgent-but-winnable campaign runway you race with
# rail-network defenses. (Below 20/s the gain still floors to 1 — never freezes; a finer/slower runway
# would need a per-cell remainder accumulator, deferred.) Balance knob; the conquest e2e certifies winnable.
DECADENCE_CREEP_PER_S = 20
# Raw PRODUCTION rate (µ-units per source-weight per ms) for the baked continent — the pace of the whole
# economy (gold/manpower/mana). The native default is 2 (a slow trickle that left tech/spells unreachable
# on the large baked map); 10 (5×) makes the 3-channel economy snappy so the tech tree + spell arm are
# reachable in a sane window. A per-city knob (CityData.production_micro); the demo/golden/native worlds
# keep the slow default (golden-neutral). Playtest-calibrated balance knob.
PRODUCTION_MICRO = 10

# --- S6 solvability validator (docs/fantasy-map.md): a baked world is CERTIFIED winnable or it's re-rolled
#     (deterministic seed sequence) — the bake is a pure function of the requested seed that always emits a
#     playable map. Constraints assert: both chains well-supplied (a forest-poor seed silently starves
#     BREAD); aether scarce but reachable + FAR (the arcane is a late prize); the two chains spatially apart
#     (no cornucopia hex — you can't get everything from one spur); the capital reachable from every resource
#     AND from the decadence reservoir (loseable, not walled); the start not a 1-hex funnel. ---
V_AETHER_MIN, V_AETHER_MAX = 3, 6   # the arcane: enough to arm legions, capped scarce
V_MIN_PER_KIND = 4                  # grain / fuel / ore each ≥ this (chains well-supplied)
V_ATTRACTOR_SEP = 20                # ore highland ↔ breadbasket min hex separation
V_CORNUCOPIA_RADIUS = 8             # no single cell has all 4 resource kinds within this
V_AETHER_MIN_DIST = 15              # every aether node ≥ this many hexes from the capital
V_FUNNEL_RADIUS, V_FUNNEL_MIN = 3, 12  # ≥ this many passable cells within R of the capital (not a funnel)
V_MAX_REROLL = 48                   # bounded strict re-roll before the relaxation ladder softens a constraint

# The RELAXATION LADDER: when no seed certifies STRICT, soften one RELAXABLE threshold a notch at a time
# (softest-first, cumulative, deterministic) and re-sweep — so the bake is a TOTAL function of the seed (it
# always terminates with a playable map), not generate-and-pray. The HARD constraints (aether floor, chains
# supplied, capital↔resources + capital↔reservoir reachability, source≠sink, aether-before-the-tide) are
# NEVER on the ladder. Strict-first means a winnable strict seed (the committed seed 12) is byte-identical.
RELAXABLE_DEFAULT = {
    "aether_max": V_AETHER_MAX,
    "attractor_sep": V_ATTRACTOR_SEP,
    "cornucopia_radius": V_CORNUCOPIA_RADIUS,
    "aether_min_dist": V_AETHER_MIN_DIST,
    "funnel_min": V_FUNNEL_MIN,
}
RELAX_LADDER = [
    ("cornucopia_radius", 6), ("cornucopia_radius", 4),
    ("attractor_sep", 16), ("attractor_sep", 12),
    ("aether_min_dist", 12), ("aether_min_dist", 10),
    ("aether_max", 7),
    ("funnel_min", 10), ("funnel_min", 8),
]

# pointy-top axial 6-neighbourhood (matches the hex adjacency the sim walks)
AXIAL_DIRS = ((+1, 0), (+1, -1), (0, -1), (-1, 0), (-1, +1), (0, +1))


# ---------------------------------------------------------------------------
# noise
# ---------------------------------------------------------------------------
def value_noise(rng, octaves, base_freq, persistence):
    """Deterministic fractal value noise on the (H,W) grid, smoothstep-interpolated, normalised [0,1].
    Cheap + dialable (docs/fantasy-map.md: "layered value/simplex on the axial grid ... not an erosion sim").
    Float, but the bake freezes only i64-quantized / rounded outputs, so determinism is preserved."""
    field = np.zeros((H, W), dtype=np.float64)
    amp, total, freq = 1.0, 0.0, base_freq
    for _ in range(octaves):
        gw, gh = freq + 2, freq + 2
        lattice = rng.random((gh, gw))
        xs = np.linspace(0.0, gw - 1.0001, W)
        ys = np.linspace(0.0, gh - 1.0001, H)
        x0 = np.floor(xs).astype(int)
        y0 = np.floor(ys).astype(int)
        fx = (xs - x0)[None, :]
        fy = (ys - y0)[:, None]
        sx = fx * fx * (3 - 2 * fx)   # smoothstep
        sy = fy * fy * (3 - 2 * fy)
        v00 = lattice[np.ix_(y0, x0)]
        v01 = lattice[np.ix_(y0, x0 + 1)]
        v10 = lattice[np.ix_(y0 + 1, x0)]
        v11 = lattice[np.ix_(y0 + 1, x0 + 1)]
        top = v00 * (1 - sx) + v01 * sx
        bot = v10 * (1 - sx) + v11 * sx
        field += amp * (top * (1 - sy) + bot * sy)
        total += amp
        amp *= persistence
        freq *= 2
    field /= total
    field -= field.min()
    field /= max(field.max(), 1e-9)
    return field


# ---------------------------------------------------------------------------
# S1 — terrain + continent + carved passes
# ---------------------------------------------------------------------------
def in_bounds(q, r):
    return 0 <= q < W and 0 <= r < H


def hex_dist(a, b):
    """Axial (cube) hex distance in cells — mirrors hexgrid.rs::distance."""
    (aq, ar), (bq, br) = a, b
    return (abs(aq - bq) + abs(aq + ar - bq - br) + abs(ar - br)) // 2


def hex_flood(land, start):
    """Connected component of truthy `land[r,q]` reachable from `start=(q,r)` via hex adjacency."""
    seen = np.zeros((H, W), dtype=bool)
    sq, sr = start
    if not land[sr, sq]:
        return seen
    stack = [start]
    seen[sr, sq] = True
    while stack:
        q, r = stack.pop()
        for dq, dr in AXIAL_DIRS:
            nq, nr = q + dq, r + dr
            if in_bounds(nq, nr) and land[nr, nq] and not seen[nr, nq]:
                seen[nr, nq] = True
                stack.append((nq, nr))
    return seen


def largest_land_component(land):
    """Bool mask of the BIGGEST connected land component (the continent). Scans every land cell, floods
    each unvisited component, keeps the largest. Deterministic (fixed (r,q) scan order)."""
    seen = np.zeros((H, W), dtype=bool)
    best = np.zeros((H, W), dtype=bool)
    best_n = 0
    for r in range(H):
        for q in range(W):
            if land[r, q] and not seen[r, q]:
                comp = hex_flood(land, (q, r))
                seen |= comp
                n = int(comp.sum())
                if n > best_n:
                    best_n, best = n, comp
    return best


def carve_passes(biome, capital):
    """THE load-bearing step (docs/fantasy-map.md): guarantee PASSABLE terrain is one connected region by
    construction. Passable = land and not MOUNTAIN. For each passable component disconnected from the
    capital's, Dijkstra a min-cost corridor over all land (MOUNTAIN dear, else cheap) from the reached
    region to it, demoting MOUNTAIN cells on the path to HILL — a narrow pass, never a sealed wall.
    Deterministic: components processed in (r,q) index order, ties broken by the heap's (cost,r,q)."""
    land = biome != WATER
    passes_carved = 0
    while True:
        passable = land & (biome != MOUNTAIN)
        reached = hex_flood(passable, capital)
        # the next unreached passable cell, in fixed (r,q) order -> deterministic target choice
        target = None
        for r in range(H):
            for q in range(W):
                if passable[r, q] and not reached[r, q]:
                    target = (q, r)
                    break
            if target:
                break
        if target is None:
            return passes_carved
        # Dijkstra from the whole reached region to `target` over land; MOUNTAIN costs MUCH more so the
        # corridor crosses the cheapest (narrowest/lowest) ridge — the carved pass.
        MOUNT_COST, FLAT_COST = 1000, 1
        dist = np.full((H, W), 1 << 60, dtype=np.int64)
        prev = {}
        pq = []
        for r in range(H):
            for q in range(W):
                if reached[r, q]:
                    dist[r, q] = 0
                    pq.append((0, r, q))
        heapq.heapify(pq)
        while pq:
            d, r, q = heapq.heappop(pq)
            if d > dist[r, q]:
                continue
            if (q, r) == target:
                break
            for dq, dr in AXIAL_DIRS:
                nq, nr = q + dq, r + dr
                if not in_bounds(nq, nr) or not land[nr, nq]:
                    continue
                step = MOUNT_COST if biome[nr, nq] == MOUNTAIN else FLAT_COST
                nd = d + step
                if nd < dist[nr, nq]:
                    dist[nr, nq] = nd
                    prev[(nq, nr)] = (q, r)
                    heapq.heappush(pq, (nd, nr, nq))
        # walk the path back, demoting mountains to a hill pass
        node = target
        while node in prev:
            q, r = node
            if biome[r, q] == MOUNTAIN:
                biome[r, q] = HILL
            node = prev[node]
        passes_carved += 1
        if passes_carved > 64:   # safety: a degenerate seed; the validator (S6) will re-roll instead
            return passes_carved


def pick_attractors(biome, capital):
    """The two attractor centres (docs/fantasy-map.md scaffold #4) — separated by construction so the two
    chains are geographically distinct. ORE highland = the MOUNTAIN-mass centroid (snapped to a passable
    cell near it); BREADBASKET = the PLAIN cell FARTHEST from the ore highland (max separation, deterministic
    argmax). Returns (ore_attractor (q,r), bread_attractor (q,r))."""
    mtn = [(q, r) for r in range(H) for q in range(W) if biome[r, q] == MOUNTAIN]
    if mtn:
        cq = sum(q for q, _ in mtn) / len(mtn)
        cr = sum(r for _, r in mtn) / len(mtn)
    else:                                            # mountainless seed: fall back to the grid centre
        cq, cr = W / 2, H / 2
    # snap the (float) centroid to the nearest passable land cell (ore sits in the reachable hills/plains)
    ore_att, best = None, 1e18
    for r in range(H):
        for q in range(W):
            if biome[r, q] in (HILL, PLAIN, FOREST):
                d = (q - cq) ** 2 + (r - cr) ** 2
                if d < best:
                    best, ore_att = d, (q, r)
    if ore_att is None:
        ore_att = capital
    # breadbasket = the plain farthest (hex) from the ore highland — guarantees the chains pull apart
    bread_att, far = ore_att, -1
    for r in range(H):
        for q in range(W):
            if biome[r, q] == PLAIN:
                d = hex_dist((q, r), ore_att)
                if d > far:
                    far, bread_att = d, (q, r)
    return ore_att, bread_att


def poisson_select(candidates, scores, min_spacing, budget):
    """Greedy farthest-first Poisson-disk thinning: take the highest-scored candidates subject to a
    hex min-spacing, up to `budget`. Deterministic — sorted by (-score, r, q)."""
    order = sorted(candidates, key=lambda c: (-scores[c], c[1], c[0]))
    chosen = []
    for c in order:
        if len(chosen) >= budget:
            break
        if all(hex_dist(c, ch) >= min_spacing for ch in chosen):
            chosen.append(c)
    return chosen


def select_arc(candidates, capital, scores, min_spacing, budget, bands=3):
    """Pick ~budget cells SPREAD across `bands` distance-from-capital rings (near/mid/far) so the result
    forms an expansion ARC instead of one far cluster (the old attractor-only placement bunched everything in
    one corner — trial feedback: 'good but varied distribution'). Greedy by score within each band; min-spacing
    enforced across ALL picks. Deterministic (each band sorted by (-score,r,q); fixed band boundaries)."""
    cl = list(candidates)
    if not cl or budget <= 0:
        return []
    d = {c: hex_dist(c, capital) for c in cl}
    lo, hi = min(d.values()), max(d.values())
    span = max(1, hi - lo)
    chosen = []
    for b in range(bands):
        b0 = lo + span * b // bands
        b1 = hi if b == bands - 1 else lo + span * (b + 1) // bands
        quota = budget // bands + (1 if b < budget % bands else 0)
        band = sorted((c for c in cl if b0 <= d[c] <= b1), key=lambda c: (-scores[c], c[1], c[0]))
        taken = 0
        for c in band:
            if taken >= quota:
                break
            if all(hex_dist(c, ch) >= min_spacing for ch in chosen):
                chosen.append(c)
                taken += 1
    return chosen


def place_capital_cluster(biome, capital, avoid):
    """Item #3 — carve a near-capital BOOTSTRAP VALLEY (FOR THE AI TO USE). The attractor deposits land ~90+
    hexes out, so nothing mints manpower before the decadence wins. RE-BIOME a few passable cells just outside
    the citadel to PLAIN/FOREST/HILL and stamp GRAIN (→ manpower → the capital-barracks's first legions),
    FUEL (→ mana) and ORE (→ the arms chain) on them; pick a STARTER-town cell within reach. A short rail
    capital→grain→starter then bootstraps the manpower the AI fields legions with. Returns (resources,
    starter_cell); MUTATES `biome`. Deterministic: nearest passable ring cells in (dist,r,q) order. The far
    deposits (place_resources) stay for the mid/late disjoint chains."""
    ring = sorted(
        ((hex_dist((q, r), capital), r, q) for r in range(H) for q in range(W)
         if biome[r, q] not in (WATER, MOUNTAIN) and (q, r) not in avoid
         and CLUSTER_MIN <= hex_dist((q, r), capital) <= CLUSTER_MAX),
        key=lambda t: (t[0], t[1], t[2]))
    out, used = [], set()
    for kind, gate in (("grain", PLAIN), ("fuel", FOREST), ("ore", HILL)):
        for _d, r, q in ring:
            if (q, r) in used:
                continue
            biome[r, q] = gate  # re-biome: the citadel's home valley supports the bootstrap chain by construction
            out.append({"kind": kind, "q": int(q), "r": int(r), "yield": int(CLUSTER_YIELD[kind])})
            used.add((q, r))
            break
    # the STARTER-town cell: the nearest remaining ring cell ≥ TOWN_MIN_FROM_RESOURCE from the new sources
    # (the goods must be RAILED in, not sit under the town) — a short capital→grain→starter manpower chain.
    starter = None
    for _d, r, q in ring:
        if (q, r) not in used and all(hex_dist((q, r), c) >= TOWN_MIN_FROM_RESOURCE for c in used):
            starter = (q, r)
            break
    return out, starter


def place_resources(biome, capital, rough):
    """S2: stamp each resource kind on its gated biome. The two CHAIN inputs (grain/fuel/ore) spread across an
    EXPANSION ARC — near/mid/far distance bands from the capital — so supply exists at EVERY range instead of
    bunching in one far corner (trial feedback: 'good but varied distribution of towns etc'). AETHER stays
    SCARCE + FAR (the arcane late prize). The biome gates still keep the chains terrain-separated (grain=plains,
    ore=hills). i64 yields. Pure + deterministic."""
    out = []
    for name, gate_biome, budget, spacing, base, _att_key, far_cap in RESOURCES:
        cands = [(q, r) for r in range(H) for q in range(W) if biome[r, q] == gate_biome]
        if not cands:
            continue
        if far_cap:  # AETHER: scarce + far (poisson by distance-from-capital — the arcane is a late prize)
            scores = {(q, r): float(hex_dist((q, r), capital)) + 2.0 * float(rough[r, q]) for (q, r) in cands}
            picks = poisson_select(cands, scores, spacing, budget)
        else:  # GRAIN / FUEL / ORE: spread across the distance arc (roughness as the scatter score)
            scores = {(q, r): float(rough[r, q]) for (q, r) in cands}
            picks = select_arc(cands, capital, scores, spacing, budget)
        for (q, r) in picks:
            # i64 yield: base + a deterministic per-site variation from the roughness field (no f32 weight)
            yld = int(base + round(40.0 * float(rough[r, q])))
            out.append({"kind": name, "q": int(q), "r": int(r), "yield": yld})
    return out


def place_forges(biome, resources, towns):
    """S7e multi-stage: site INGOT forges ON THE ORE → ARMS-TOWN CORRIDOR, so the player rails ORE → forge →
    ARMS town over two reasonable legs (a forge sited toward the capital instead would strand it ~40 km from
    the ARMS towns it feeds — the chain would never prime). One forge per ARMS town (the towns whose recipe
    needs INGOT), placed at the passable cell nearest the midpoint of that town and its NEAREST ore resource.
    Deterministic; a forge is a PROCESSOR (origin INGOT + dest ORE) the sim classifies from its demand cells.
    Returns dicts {kind:"forge", q, r, yield}."""
    ore_res = [(x["q"], x["r"]) for x in resources if x["kind"] == "ore"]
    arms_towns = [t for t in towns if t.get("recipe") and INGOT in t["recipe"]]
    forges, used = [], set()
    for t in arms_towns:
        if len(forges) >= FORGE_BUDGET or not ore_res:
            break
        tq, tr = t["q"], t["r"]
        oq, orr = min(ore_res, key=lambda o: (hex_dist(o, (tq, tr)), o[1], o[0]))  # ore nearest THIS town
        mq, mr = (oq + tq) // 2, (orr + tr) // 2  # corridor midpoint ore ↔ arms town
        best, bd = None, 1 << 30
        for r in range(H):
            for q in range(W):
                if biome[r, q] in (WATER, MOUNTAIN) or (q, r) in used:
                    continue
                d = hex_dist((q, r), (mq, mr))
                if d < bd:
                    bd, best = d, (q, r)
        if best is not None:
            used.add(best)
            forges.append({"kind": "forge", "q": int(best[0]), "r": int(best[1]), "yield": 80})
    return forges


def place_towns(biome, capital, resources, rough, ore_att, forced_starter=None):
    """S3: the capital + Poisson-spread neutral towns, suitability-sited near resource clusters and graded
    by distance from the capital into the expansion arc. Each town carries an i64 value and a 2–3-good
    demand set (its nearest distinct resource kinds). `forced_starter` (item #3) pins the STARTER town at the
    near-capital bootstrap cell so the AI has an early manpower sink. Returns a list of dicts {kind,q,r,...}."""
    res_qr = [(x["q"], x["r"], x["kind"]) for x in resources]
    cands = [(q, r) for r in range(H) for q in range(W)
             if biome[r, q] not in (WATER, MOUNTAIN)
             and hex_dist((q, r), capital) >= TOWN_MIN_FROM_CAPITAL
             # NEAR resources but never ON one (≥ TOWN_MIN_FROM_RESOURCE): the goods must be RAILED in,
             # not sitting under the town — else source==sink and there's nothing to transport.
             and all(hex_dist((q, r), (rq, rr)) >= TOWN_MIN_FROM_RESOURCE for (rq, rr, _) in res_qr)
             # keep the Poisson neutrals clear of the forced near-capital starter (it's added explicitly).
             and (forced_starter is None or hex_dist((q, r), forced_starter) >= TOWN_MIN_SPACING)]
    scores = {}
    for (q, r) in cands:
        # suitability: close to resources (sum of inverse hex distance) + a mild far-from-capital spread
        near = sum(1.0 / (1.0 + hex_dist((q, r), (rq, rr))) for (rq, rr, _) in res_qr)
        scores[(q, r)] = near + 0.02 * hex_dist((q, r), capital) + 0.3 * float(rough[r, q])
    # Spread the neutral towns across the EXPANSION ARC (near/mid/far bands from the capital) so they form a
    # graded journey outward, not one far cluster (trial: 'good but varied distribution of towns').
    chosen = select_arc(cands, capital, scores, TOWN_MIN_SPACING, TOWN_BUDGET)
    # Item #3: the near-capital bootstrap cell is THE starter (pinned); else the nearest Poisson town.
    if forced_starter is not None:
        chosen = [forced_starter] + chosen
        starter = forced_starter
    else:
        starter = min(chosen, key=lambda c: (hex_dist(c, capital), c[1], c[0])) if chosen else None

    def demand_set(q, r):
        kinds = {k for (_, _, k) in res_qr}
        return sorted(kinds, key=lambda k: min((hex_dist((q, r), (rq, rr))
                                                for (rq, rr, kk) in res_qr if kk == k), default=9999))[:3]

    # the capital is the barracks (neutral, no consume recipe); neutral + starter towns are BREAD/ARMS sinks
    towns = [{"kind": "capital", "q": int(capital[0]), "r": int(capital[1]), "value": 0,
              "demands": demand_set(*capital), "recipe": []}]
    for (q, r) in chosen:
        kind = "starter" if (q, r) == starter else "neutral"
        towns.append({"kind": kind, "q": int(q), "r": int(r),
                      "value": int(TOWN_BASE_VALUE + TOWN_ARC_VALUE * hex_dist((q, r), capital)),
                      "demands": demand_set(q, r), "recipe": []})

    # Recipe split: the ~1/3 of towns NEAREST the ore highland demand ARMS (ore+aether), the rest BREAD
    # (grain+fuel). A fixed fraction (not a nearer-attractor test) because towns can only site in the
    # passable lowland — the ore attractor is in the impassable massif, so all towns are "nearer" the
    # breadbasket. This guarantees BOTH chains have consumers (the disjoint-chain payoff). Each town
    # consumes BOTH its chain's inputs by Liebig. Commodity indices match crates/sim/forge.rs.
    sinks = [t for t in towns if t["kind"] != "capital"]
    sinks.sort(key=lambda t: (hex_dist((t["q"], t["r"]), ore_att), t["r"], t["q"]))
    n_arms = max(1, len(sinks) // 3)
    for i, t in enumerate(sinks):
        # ARMS towns are 3-stage (need INGOT from a forge + aether); BREAD towns are 2-stage (grain+fuel).
        t["recipe"] = list(ARMS_RECIPE) if i < n_arms else list(BREAD_RECIPE)
        t["chain"] = "arms" if i < n_arms else "bread"
    return towns


def seed_decadence(biome, capital, towns):
    """S4: seed the corruption. Mutates each town to add a `decadence` floor (0 at the capital + within the
    grace ring; rising with frontier depth past it), and returns the decadence seed: the far-edge RESERVOIR
    anchor cells (tide origin + raider spawns) opposite the capital + the grace radius. Pure + deterministic."""
    for t in towns:
        d = hex_dist((t["q"], t["r"]), capital)
        if t["kind"] in ("capital", "starter") or d <= CAPITAL_GRACE_HEXES:
            t["decadence"] = 0
        else:
            t["decadence"] = int(DECADENCE_BASE + DECADENCE_PER_HEX * (d - CAPITAL_GRACE_HEXES))
    # reservoir = the coastal land cells FARTHEST from the capital (the far edge opposite it) — the tide
    # origin. (Coastal = adjacent to water/edge.) Deterministic sort by (-dist, r, q).
    coastal = []
    for r in range(H):
        for q in range(W):
            if biome[r, q] in (WATER, MOUNTAIN):
                continue
            if any(not in_bounds(q + dq, r + dr) or biome[r + dr, q + dq] == WATER for dq, dr in AXIAL_DIRS):
                coastal.append((hex_dist((q, r), capital), q, r))
    coastal.sort(key=lambda t: (-t[0], t[2], t[1]))
    reservoir = []
    for _, q, r in coastal[:RESERVOIR_ANCHORS]:
        x_mm = round(float(GRID_CELL_MM) * (SQRT3 * q + SQRT3 / 2.0 * r))
        y_mm = round(float(GRID_CELL_MM) * (1.5 * r))
        reservoir.append({"q": int(q), "r": int(r), "xMm": x_mm, "yMm": y_mm})
    # the realm's STARTING decadence = the mean neutral-town floor (the ambient corruption the player
    # inherits). i64. World::new seeds world.decadence from it (a more-corrupt continent = more urgency).
    floors = [t["decadence"] for t in towns if t["kind"] == "neutral"]
    initial = int(round(sum(floors) / len(floors))) if floors else 0
    # The capital cell in mm (S10): the seat the decadence tide races toward — the lose target + the
    # creep-gradient origin for the in-core DecadenceField. Same hex transform as the reservoir.
    cap_x_mm = round(float(GRID_CELL_MM) * (SQRT3 * capital[0] + SQRT3 / 2.0 * capital[1]))
    cap_y_mm = round(float(GRID_CELL_MM) * (1.5 * capital[1]))
    # Item #9 — the AREA-OF-INFLUENCE build radius (hexes). Derived from the town arc so the realm is
    # WINNABLE BY CONSTRUCTION: rail is buildable only within `influenceHops` of the capital + each captured
    # town, and conquering extends it. Set to cover the nearest neutral (so the first target is reachable)
    # AND the largest consecutive town gap (so each conquest opens the next), + a grace margin. Bigger gaps
    # ⇒ a more generous radius — the gate still walls off the DEEP frontier until you expand into it.
    nd = sorted(hex_dist((t["q"], t["r"]), capital) for t in towns if t["kind"] == "neutral")
    if nd:
        gaps = [nd[0]] + [nd[i] - nd[i - 1] for i in range(1, len(nd))]
        influence_hops = int(round(max(gaps) * 1.25)) + CAPITAL_GRACE_HEXES
    else:
        influence_hops = 9999  # no neutral towns ⇒ no gate
    return {"capitalGraceHexes": CAPITAL_GRACE_HEXES, "reservoir": reservoir,
            "initialDecadence": initial, "growthPerS": DECADENCE_GROWTH_PER_S,
            "armySpeedMmS": ARMY_SPEED_MM_S, "creepPerS": DECADENCE_CREEP_PER_S,
            "productionMicro": PRODUCTION_MICRO, "influenceHops": influence_hops,
            "capitalXMm": int(cap_x_mm), "capitalYMm": int(cap_y_mm)}


def validate(biome, capital, fields, relax=None):
    """S6: list the solvability constraints a baked world VIOLATES (empty list = certified winnable). Pure.
    `relax` overrides the RELAXABLE thresholds (the relaxation ladder); the HARD constraints always use the
    strict globals. Default (relax=None) = fully strict — the byte-identical original behaviour."""
    from collections import Counter
    rx = relax or RELAXABLE_DEFAULT
    res, dec = fields["resources"], fields["decadence"]
    by = Counter(x["kind"] for x in res)
    fails = []
    # HARD — the arcane floor + both chains supplied (a starved chain is unwinnable, never relax).
    if by["aether"] < V_AETHER_MIN:
        fails.append(f"aether {by['aether']} < {V_AETHER_MIN} (can't arm legions)")
    for k in ("grain", "fuel", "ore"):
        if by[k] < V_MIN_PER_KIND:
            fails.append(f"{k} {by[k]} < {V_MIN_PER_KIND} (chain under-supplied)")
    # RELAXABLE — aether scarcity ceiling.
    if by["aether"] > rx["aether_max"]:
        fails.append(f"aether {by['aether']} > {rx['aether_max']} (not scarce)")
    # RELAXABLE — the two chains pulled apart.
    if hex_dist(fields["ore_att"], fields["bread_att"]) < rx["attractor_sep"]:
        fails.append("attractor centres too close (chains not separable)")
    passable = (biome != WATER) & (biome != MOUNTAIN)
    reach = hex_flood(passable, capital)
    # HARD — every resource rail-reachable from the capital.
    if not all(bool(reach[x["r"], x["q"]]) for x in res):
        fails.append("a resource is unreachable from the capital")
    # RELAXABLE — aether is a LATE prize (far from the capital).
    if any(x["kind"] == "aether" and hex_dist((x["q"], x["r"]), capital) < rx["aether_min_dist"] for x in res):
        fails.append(f"aether closer than {rx['aether_min_dist']} hexes (not a late prize)")
    # RELAXABLE — no cornucopia hex (all 4 kinds clustered).
    if len(set(by)) == 4 and any(
            len({y["kind"] for y in res if hex_dist((x["q"], x["r"]), (y["q"], y["r"])) <= rx["cornucopia_radius"]}) == 4
            for x in res):
        fails.append("cornucopia hex (all 4 resources clustered)")
    # HARD — capital reachable FROM the reservoir (loseable, not walled off).
    if not all(bool(reach[a["r"], a["q"]]) for a in dec["reservoir"]):
        fails.append("capital walled off from the reservoir (unloseable)")
    # HARD — you can REACH THE ARCANE BEFORE THE TIDE REACHES YOU: the nearest aether is closer to the
    # capital than the nearest reservoir anchor. Else SPELLCRAFT/the arms chain is unreachable in time —
    # a structurally-unwinnable race. Never relaxed (it's the win-condition's load-bearing geometry).
    aether_d = [hex_dist((x["q"], x["r"]), capital) for x in res if x["kind"] == "aether"]
    rsv_d = [hex_dist((a["q"], a["r"]), capital) for a in dec["reservoir"]]
    if aether_d and rsv_d and min(aether_d) >= min(rsv_d):
        fails.append("aether farther than the reservoir (can't reach the arcane before the tide)")
    # HARD — a town never sits ON a resource (source==sink → nothing to transport).
    towns = fields["towns"]
    res_cells = {(x["q"], x["r"]) for x in res}
    if any((t["q"], t["r"]) in res_cells for t in towns):
        fails.append("a town coincides with a resource (source==sink — nothing to transport)")
    # RELAXABLE — the capital isn't a build funnel.
    near_pass = sum(1 for r in range(max(0, capital[1] - V_FUNNEL_RADIUS), min(H, capital[1] + V_FUNNEL_RADIUS + 1))
                    for q in range(max(0, capital[0] - V_FUNNEL_RADIUS), min(W, capital[0] + V_FUNNEL_RADIUS + 1))
                    if passable[r, q] and hex_dist((q, r), capital) <= V_FUNNEL_RADIUS)
    if near_pass < rx["funnel_min"]:
        fails.append(f"capital is a {near_pass}-cell funnel (< {rx['funnel_min']})")
    return fails


def generate_valid(seed):
    """S6: the first CERTIFIED-winnable world at/after `seed`. STRICT-FIRST (a winnable strict seed is
    byte-identical to the old behaviour); only if NO seed certifies strict does the RELAXATION LADDER soften
    one RELAXABLE constraint a notch at a time (softest-first) and re-sweep — so the bake is a TOTAL function
    of the seed (always terminates with a playable map). Returns (seed, biome, capital, fields, fails,
    relaxations) — `relaxations` lists which thresholds were softened ([] = certified strict)."""
    relax = dict(RELAXABLE_DEFAULT)
    relaxations = []
    rung = 0
    while True:
        last = None
        for i in range(V_MAX_REROLL):
            s = seed + i
            biome, capital, fields = generate(s)
            fails = validate(biome, capital, fields, relax)
            last = (s, biome, capital, fields, fails)
            if not fails:
                return (s, biome, capital, fields, fails, relaxations)
        if rung >= len(RELAX_LADDER):
            assert last is not None  # V_MAX_REROLL ≥ 1 so the sweep always ran
            return (*last, relaxations)  # ladder exhausted: best-effort world + its violations (main warns)
        key, val = RELAX_LADDER[rung]
        relax[key] = val
        relaxations.append(f"{key}->{val}")
        rung += 1


def generate(seed):
    """Run the S1 pipeline -> (biome [H,W] int8, capital (q,r), fields dict). Pure function of `seed`."""
    rng = np.random.default_rng(np.uint64(seed) ^ np.uint64(MAP_CONST))

    # four decorrelated fields (distinct rng draws)
    warp = value_noise(rng, octaves=4, base_freq=2, persistence=0.55)      # continent-shape warp
    moisture = value_noise(rng, octaves=5, base_freq=3, persistence=0.55)  # Whittaker moisture axis
    ridge = value_noise(rng, octaves=5, base_freq=4, persistence=0.5)      # thin-ridge LEY field source
    rough = value_noise(rng, octaves=6, base_freq=5, persistence=0.5)      # heterogeneous elevation roughness

    # --- continent mask: a warped radial island (sea all round) so the capital corner is genuinely COASTAL
    #     and the far quadrant is the frontier. Land fraction ~0.5. ---
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float64)
    cx, cy = 0.5 * W, 0.5 * H
    dx = (xx - cx) / (0.60 * W)
    dy = (yy - cy) / (0.60 * H)
    radial = 1.0 - np.sqrt(dx * dx + dy * dy)        # 1 at centre, ~0 at the inscribed edge
    land_field = radial + 0.42 * (warp - 0.5) * 2.0  # ragged coastline
    land = land_field > 0.18

    # --- elevation backbone = distance-from-coast (research refinement): monotonic away from the coast =>
    #     no interior local minima (downhill-to-sea is free), capital corner low/coastal, interior high. ---
    elev_dist = coast_distance(land)
    elev = elev_dist / max(elev_dist.max(), 1e-9)
    elev = np.power(elev, 1.15)                      # redistribution power curve (tune flat-land fraction)
    elev = 0.82 * elev + 0.18 * rough                # heterogeneous roughness (capital gentle, far jagged)
    elev[~land] = 0.0

    # --- biome classification: Whittaker-ish on (elevation, moisture); LEY a rare thin ridge on high ground ---
    biome = np.full((H, W), WATER, dtype=np.int16)
    ley = land & (ridge > 0.85) & (elev > 0.52)      # rare arcane ridges, biased high (far interior); S2 rarefies to nodes
    biome[land & (elev >= 0.78)] = MOUNTAIN
    biome[land & (elev >= 0.55) & (elev < 0.78)] = HILL
    biome[land & (elev < 0.55) & (moisture >= 0.55)] = FOREST
    biome[land & (elev < 0.55) & (moisture < 0.55)] = PLAIN
    biome[ley] = LEY

    # --- contiguous-continent guarantee: keep the LARGEST land component, delete disconnected islands.
    #     (Done BEFORE capital choice so the capital is always on the main continent — else picking a cell
    #     on a small nub would make island-deletion nuke the real landmass.) ---
    main = largest_land_component(biome != WATER)
    biome[(biome != WATER) & ~main] = WATER

    # --- capital: the most-coastal buildable land cell in the SW (0,0)-ward quadrant (low elev = coastal) ---
    capital = pick_capital(biome, elev)

    # --- THE pass carve: passable terrain connected by construction ---
    passes = carve_passes(biome, capital)

    # S2: stamp ORE/GRAIN/FUEL/AETHER on qualifying biomes, Poisson-rarefy to a budget, bias to attractors.
    resources = place_resources(biome, capital, rough)
    # Item #3: carve a near-capital bootstrap valley (grain/fuel/ore + a starter-town cell) so the AI can
    # mint manpower + field legions early. Avoids existing deposit cells (no re-biome clobber). Far deposits stay.
    cluster_res, starter_cell = place_capital_cluster(biome, capital, {(x["q"], x["r"]) for x in resources})
    resources = cluster_res + resources
    ore_att, bread_att = pick_attractors(biome, capital)
    # S3: capital + Poisson neutral towns + the PINNED near-capital starter (the early manpower sink).
    towns = place_towns(biome, capital, resources, rough, ore_att, forced_starter=starter_cell)
    # S7e multi-stage: INGOT forges on the ore→ARMS-town corridor (placed AFTER towns so each ARMS town's
    # forge sits between it and its nearest ore — kept in a separate list: infrastructure, not an attractor).
    forges = place_forges(biome, resources, towns)
    # S4: seed the decadence (per-town floor + far-edge reservoir + capital grace).
    decadence = seed_decadence(biome, capital, towns)
    # RIVERS: sink-fill the (non-monotone) elevation, then flow-accumulation drainage trees on the FINAL
    # land mask. A discrete, deterministic edge topology — render-only (additive manifest field).
    land_final = biome != WATER
    elev_filled = fill_sinks(elev, land_final)
    rivers = compute_rivers(elev_filled, land_final)[0]
    return biome, capital, {"elev": elev, "elev_filled": elev_filled, "moisture": moisture, "passes": passes,
                            "land": land, "resources": resources, "forges": forges, "towns": towns,
                            "decadence": decadence, "rivers": rivers, "ore_att": ore_att, "bread_att": bread_att}


def coast_distance(land):
    """Hex BFS distance (in cells) from the nearest WATER/edge for each land cell; 0 off-land. The elevation
    backbone — monotonic inland, so downhill-to-sea is free (research: distance-from-coast field)."""
    INF = 1 << 30
    dist = np.full((H, W), INF, dtype=np.int64)
    from collections import deque
    dq = deque()
    for r in range(H):
        for q in range(W):
            if not land[r, q]:
                continue
            # coast = a land cell adjacent to water or off the grid edge
            edge = False
            for ddq, ddr in AXIAL_DIRS:
                nq, nr = q + ddq, r + ddr
                if not in_bounds(nq, nr) or not land[nr, nq]:
                    edge = True
                    break
            if edge:
                dist[r, q] = 0
                dq.append((q, r))
    while dq:
        q, r = dq.popleft()
        for ddq, ddr in AXIAL_DIRS:
            nq, nr = q + ddq, r + ddr
            if in_bounds(nq, nr) and land[nr, nq] and dist[nr, nq] > dist[r, q] + 1:
                dist[nr, nq] = dist[r, q] + 1
                dq.append((nq, nr))
    out = dist.astype(np.float64)
    out[~land] = 0.0
    out[out > (1 << 29)] = 0.0
    return out


# ---------------------------------------------------------------------------
# S-rivers — flow-accumulation drainage (research: rivers = the believability cue AND the gameplay
# chokepoint, the SAME feature). Offline float math, frozen as a discrete i64-mm edge topology. Render-only
# for now (additive `rivers` manifest field); the rail-cost coupling is a separate, balance-gated follow-up.
# ---------------------------------------------------------------------------
RIVER_FILL_EPS = 1e-6


def fill_sinks(elev, land):
    """Priority-flood sink-fill (Barnes 2014): raise every interior PIT just above its lowest rim so each
    land cell has a STRICTLY-DESCENDING path to the sea. REQUIRED before flow routing — the elevation
    backbone (0.82·coast-dist + 0.18·roughness) is NOT monotone, so raw downhill parents would cycle / run
    uphill. Deterministic: a min-heap keyed (elev, r, q); coast/edge land cells are the outlets. Returns the
    filled float elevation (land only; water left as-is)."""
    filled = elev.astype(np.float64).copy()
    closed = np.zeros((H, W), dtype=bool)
    pq = []
    for r in range(H):
        for q in range(W):
            if not land[r, q]:
                continue
            outlet = False
            for dq, dr in AXIAL_DIRS:
                nq, nr = q + dq, r + dr
                if not in_bounds(nq, nr) or not land[nr, nq]:
                    outlet = True
                    break
            if outlet:
                closed[r, q] = True
                heapq.heappush(pq, (float(filled[r, q]), r, q))
    while pq:
        e, r, q = heapq.heappop(pq)
        for dq, dr in AXIAL_DIRS:
            nq, nr = q + dq, r + dr
            if not in_bounds(nq, nr) or not land[nr, nq] or closed[nr, nq]:
                continue
            closed[nr, nq] = True
            ne = max(float(filled[nr, nq]), e + RIVER_FILL_EPS)  # at least a hair above the outlet rim
            filled[nr, nq] = ne
            heapq.heappush(pq, (ne, nr, nq))
    return filled


def compute_rivers(elev_filled, land):
    """Flow-accumulation drainage trees on the FILLED elevation. Each land cell drains to its LOWEST strictly-
    lower land neighbour (its parent); flux (catchment size) accumulates downstream (processing high→low, so a
    cell's flux is complete before it feeds its parent). An edge (cell→parent) becomes a RIVER above a flux
    threshold (scaled to map size); width class 1..4 ∝ √flux; a FORD marks the thin headwater band (a cheap
    crossing). The filled elevation guarantees the parent forest is ACYCLIC. Deterministic (index-ordered ties).
    Returns (edges, parent, flux, threshold) where edges = [(q,r,toQ,toR,wclass,ford)]."""
    order = [(float(elev_filled[r, q]), r, q) for r in range(H) for q in range(W) if land[r, q]]
    order.sort(key=lambda t: (-t[0], t[1], t[2]))  # descending elevation, deterministic ties
    parent = {}
    for _, r, q in order:
        here = float(elev_filled[r, q])
        best, best_e = None, here
        for dq, dr in AXIAL_DIRS:
            nq, nr = q + dq, r + dr
            if not in_bounds(nq, nr) or not land[nr, nq]:
                continue
            ne = float(elev_filled[nr, nq])
            if ne < best_e or (ne == best_e and best is not None and (nr, nq) < (best[1], best[0])):
                best, best_e = (nq, nr), ne
        if best is not None and best_e < here:
            parent[(q, r)] = best
    flux = np.zeros((H, W), dtype=np.float64)
    flux[land] = 1.0
    for _, r, q in order:  # high→low: a cell's flux is final when reached, then folds into its parent
        p = parent.get((q, r))
        if p is not None:
            flux[p[1], p[0]] += flux[r, q]
    n_land = int(land.sum())
    threshold = max(40.0, 0.012 * n_land)
    edges = []
    for (q, r), (pq_, pr_) in sorted(parent.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        f = float(flux[r, q])
        if f >= threshold:
            wclass = int(min(4, 1 + round(2.0 * (f / threshold) ** 0.5)))
            ford = bool(threshold <= f < 2.0 * threshold)
            edges.append((q, r, pq_, pr_, wclass, ford))
    return edges, parent, flux, threshold


def pick_capital(biome, elev):
    """Most-coastal buildable land cell in the SW (0,0)-ward quadrant: minimise elev (coastal) + a gentle
    pull toward the corner. Deterministic argmin over a fixed scan."""
    best, best_score = None, 1e18
    for r in range(H // 2):
        for q in range(W // 2):
            c = biome[r, q]
            if c == WATER or c == MOUNTAIN:
                continue
            corner_pull = (q + r) / float(W + H)         # 0 at the corner, ~1 far
            score = elev[r, q] + 0.5 * corner_pull
            if score < best_score:
                best_score, best = score, (q, r)
    if best is None:                                      # degenerate: no SW land — fall back to any land
        for r in range(H):
            for q in range(W):
                if biome[r, q] not in (WATER, MOUNTAIN):
                    return (q, r)
        return (0, 0)                                     # fully degenerate (no land at all) — unreachable in practice
    return best


# ---------------------------------------------------------------------------
# hex-quantize (S5) + emit
# ---------------------------------------------------------------------------
def cell_lonlat(q, r):
    """Axial (q,r) -> mm via hexgrid::center_of -> lon/lat via geo.ts's equirectangular frame. The emitted
    lon/lat reprojects (in the frontend) back onto the exact lattice the sim quantizes (axial_of)."""
    s = float(GRID_CELL_MM)
    x_mm = round(s * (SQRT3 * q + SQRT3 / 2.0 * r))
    y_mm = round(s * (1.5 * r))
    lng = ORIGIN_LNG + (x_mm / 1000.0) / M_PER_DEG_LNG
    lat = ORIGIN_LAT + (y_mm / 1000.0) / M_PER_DEG_LAT
    return round(lng, 6), round(lat, 6)


def emit(cid, seed, biome, capital, resources, towns, decadence, forges=None, rivers=None):
    """Serialize the world + buildability + demand packs. Returns the cells list (for the self-test)."""
    forges = forges or []
    rivers = rivers or []
    land = biome != WATER
    # emit every land cell + a 1-ring water margin (the coastline); skip deep ocean to bound size
    near_land = land.copy()
    for r in range(H):
        for q in range(W):
            if land[r, q]:
                for dq, dr in AXIAL_DIRS:
                    nq, nr = q + dq, r + dr
                    if in_bounds(nq, nr):
                        near_land[nr, nq] = True
    cells = []
    lons, lats = [], []
    for r in range(H):
        for q in range(W):
            if not near_land[r, q]:
                continue
            lng, lat = cell_lonlat(q, r)
            cells.append({"lon": lng, "lat": lat, "c": int(biome[r, q])})
            lons.append(lng)
            lats.append(lat)
    bbox = [min(lons), min(lats), max(lons), max(lats)]
    center = [round((bbox[0] + bbox[2]) / 2, 6), round((bbox[1] + bbox[3]) / 2, 6)]

    build = {"cellM": GRID_CELL_MM / 1000.0, "bbox": bbox, "cells": cells}
    json.dump(build, open(os.path.join(OUT, f"{cid}_buildability.json"), "w"), separators=(",", ":"))

    # Demand grid (FRONTEND RawDemand shape — {lon,lat,originWeight,destWeight}; loadCity converts to mm).
    # Capital + a dest_w bump at every resource (docs/fantasy-map.md S2: "reuse catchment capture"), so the
    # supply nodes are visible demand and the coverage gauge sees them. S3 adds town demand on top.
    cap_lng, cap_lat = cell_lonlat(*capital)
    dcells = [{"lon": cap_lng, "lat": cap_lat, "originWeight": 1.0, "destWeight": 1.0}]
    for res in resources:                            # resources = sources (origin pull), each its commodity
        rlng, rlat = cell_lonlat(res["q"], res["r"])
        dcells.append({"lon": rlng, "lat": rlat, "originWeight": float(res["yield"]) / 10.0,
                       "destWeight": 1.0, "commodity": COMMODITY_IDX[res["kind"]]})
    for town in towns:                               # towns = sinks: ONE dest cell PER recipe input, tagged
        if town["kind"] == "capital":                #   with its commodity → station_recipe = the 2 inputs
            continue                                  #   → the sim consumes them by Liebig (needs BOTH).
        tlng, tlat = cell_lonlat(town["q"], town["r"])
        dw = float(town["value"]) / 100.0
        for c in town["recipe"]:
            dcells.append({"lon": tlng, "lat": tlat, "originWeight": 1.0, "destWeight": dw, "commodity": c})
    for forge in forges:                             # S7e: a forge is a PROCESSOR — it CONSUMES ore (dest
        flng, flat = cell_lonlat(forge["q"], forge["r"])  # ORE) and PRODUCES ingot (origin INGOT) shipped on.
        dcells.append({"lon": flng, "lat": flat, "originWeight": float(forge["yield"]) / 10.0,
                       "destWeight": 1.0, "commodity": INGOT})            # output: it makes INGOT
        dcells.append({"lon": flng, "lat": flat, "originWeight": 1.0,
                       "destWeight": float(forge["yield"]) / 10.0, "commodity": COMMODITY_IDX["ore"]})  # input: ore
    demand = {"cellM": GRID_CELL_MM / 1000.0, "bbox": bbox, "cells": dcells}
    json.dump(demand, open(os.path.join(OUT, f"{cid}_demand.json"), "w"), separators=(",", ":"))

    # supply_graph.resources[] — an ADDITIVE manifest field (serde-ignored by transit; buildCoreCity never
    # copies it into the core JSON, so it never reaches Sim::new's CityData — frontend-render + future-sim
    # data). Positions carried as both axial (q,r) and i64 mm (= hexgrid::center_of), yields i64.
    def to_mm(q, r):
        return (round(float(GRID_CELL_MM) * (SQRT3 * q + SQRT3 / 2.0 * r)),
                round(float(GRID_CELL_MM) * (1.5 * r)))

    sg_resources = []
    for res in resources + forges:  # forges ride the resources path → networkFromSupplyGraph places them
        x_mm, y_mm = to_mm(res["q"], res["r"])
        sg_resources.append({"kind": res["kind"], "q": res["q"], "r": res["r"],
                             "xMm": x_mm, "yMm": y_mm, "yield": res["yield"]})
    sg_towns = []
    for town in towns:
        x_mm, y_mm = to_mm(town["q"], town["r"])
        sg_towns.append({"kind": town["kind"], "q": town["q"], "r": town["r"], "xMm": x_mm, "yMm": y_mm,
                         "value": town["value"], "demands": town["demands"], "decadence": town["decadence"],
                         "recipe": town["recipe"]})

    # Rivers — an ADDITIVE manifest field (like supplyGraph: never copied into the core city JSON, so it
    # never reaches Sim::new — frontend-render data only). Each edge carries axial (q,r)->(toQ,toR) AND its
    # i64-mm cell-centre endpoints; the frontend draws cell-centre polylines via coords/geo.ts.
    sg_rivers = []
    for (q, r, tq, tr, wclass, ford) in rivers:
        x0, y0 = to_mm(q, r)
        x1, y1 = to_mm(tq, tr)
        sg_rivers.append({"q": int(q), "r": int(r), "toQ": int(tq), "toR": int(tr),
                          "x0Mm": x0, "y0Mm": y0, "x1Mm": x1, "y1Mm": y1,
                          "wclass": int(wclass), "ford": bool(ford)})

    manifest = {
        "id": cid, "name": "Arcadia (baked)", "originLngLat": [ORIGIN_LNG, ORIGIN_LAT],
        "bbox": bbox, "center": center, "zoom": 10, "seed": int(seed),   # the continent is large — frame the whole domain
        "ruleset": "arcadia",            # the sim's canon tag (docs say "fantasy"; select() recognises "arcadia")
        "gridCellMm": GRID_CELL_MM,
        "demandGridPath": f"/data/{cid}_demand.json",
        "buildabilityPath": f"/data/{cid}_buildability.json",   # the terrain raster (the frontend renders it as the map)
        # additive (serde-safe): S2 resources + S3 towns (w/ per-town S4 decadence floor) + the S4 seed
        "supplyGraph": {"resources": sg_resources, "towns": sg_towns, "decadenceSeed": decadence},
        "rivers": sg_rivers,  # additive render-only drainage topology (flow-accumulation; never enters the core)
    }
    json.dump(manifest, open(os.path.join(OUT, f"{cid}_world.json"), "w"), indent=2)
    return cells


def ascii_preview(biome, capital, cols=72, resources=None, towns=None):
    """Downsampled glyph map — corroboration ("look at the terrain", not a pixel gate). Resources (O/G/F/A),
    towns (t / starter s / capital @) overlaid on their downsampled cell (towns drawn over resources)."""
    step = max(1, W // cols)
    overlay = {}
    for res in (resources or []):
        overlay[(res["q"] // step, res["r"] // step)] = RES_GLYPH[res["kind"]]
    for town in (towns or []):
        overlay[(town["q"] // step, town["r"] // step)] = TOWN_GLYPH[town["kind"]]
    overlay[(capital[0] // step, capital[1] // step)] = "@"
    lines = []
    for r in range(0, H, step):
        row = []
        for q in range(0, W, step):
            row.append(overlay.get((q // step, r // step)) or GLYPH.get(int(biome[r, q]), "?"))
        lines.append("".join(row))
    return "\n".join(lines)


def biome_hist(biome):
    from collections import Counter
    return Counter(int(x) for x in biome.flatten())


# ---------------------------------------------------------------------------
# self-test (determinism + connectivity + structure)
# ---------------------------------------------------------------------------
def selftest(seed):
    ok = True

    def check(name, cond):
        nonlocal ok
        ok = ok and cond
        print(f"  [{'PASS' if cond else 'FAIL'}] {name}")

    # --- synthetic carve unit-test: a MOUNTAIN wall splitting two plains halves must be pierced by a pass
    #     (real island seeds rarely wall a region off — the coast ring connects everything — so prove the
    #     load-bearing step on a constructed adversarial grid). ---
    wall = np.full((H, W), PLAIN, dtype=np.int16)
    wall[:, W // 2] = MOUNTAIN                   # a full vertical ridge across the grid
    carved = carve_passes(wall, (1, 1))
    wall_passable = (wall != MOUNTAIN)
    wall_reached = hex_flood(wall_passable, (1, 1))
    check(f"pass-carve pierces a full mountain wall ({carved} pass(es) carved)",
          carved >= 1 and int((wall_passable & ~wall_reached).sum()) == 0)

    b1, cap1, f1 = generate(seed)
    b2, cap2, f2 = generate(seed)               # determinism: same seed, twice, in one process
    check("determinism: biome rasters identical", np.array_equal(b1, b2))
    check("determinism: capital identical", cap1 == cap2)

    land = b1 != WATER
    hist = biome_hist(b1)
    n_land = int(land.sum())
    frac = n_land / (W * H)
    check(f"land fraction sane (0.30..0.85): {frac:.2f}", 0.30 <= frac <= 0.85)
    check(f"capital is buildable land (not water/mountain): c={int(b1[cap1[1], cap1[0]])}",
          b1[cap1[1], cap1[0]] not in (WATER, MOUNTAIN))

    # contiguous continent: one land component (capital's), no orphan islands left
    keep = hex_flood(land, cap1)
    check(f"single contiguous continent (no orphan islands): {int(land.sum() - keep.sum())} orphan cells",
          int((land & ~keep).sum()) == 0)

    # passable connectivity by construction (the pass carve worked)
    passable = land & (b1 != MOUNTAIN)
    preached = hex_flood(passable, cap1)
    check(f"all passable land reachable from capital (passes carved: {f1['passes']}): "
          f"{int((passable & ~preached).sum())} stranded",
          int((passable & ~preached).sum()) == 0)

    codes = set(hist.keys())
    check(f"biome codes subset of {{4,6,7,8,9,10}}: {sorted(codes)}",
          codes <= {WATER, MOUNTAIN, HILL, FOREST, LEY, PLAIN})
    check(f"has mountains (ridges exist): {hist[MOUNTAIN]}", hist[MOUNTAIN] > 0)
    check(f"has plains+forest (buildable interior): plain={hist[PLAIN]} forest={hist[FOREST]}",
          hist[PLAIN] + hist[FOREST] > n_land * 0.10)

    # --- S2 resources: gating, scarcity cap, separation, reachability ---
    res = f1["resources"]
    from collections import Counter as _C
    by = _C(x["kind"] for x in res)
    GATE = {"ore": HILL, "grain": PLAIN, "fuel": FOREST, "aether": LEY}
    check(f"every resource sits on its gated PASSABLE biome: {dict(by)}",
          all(int(b1[x["r"], x["q"]]) == GATE[x["kind"]] for x in res) and all(x["kind"] != "" for x in res))
    check("no resource on impassable MOUNTAIN (all rail-reachable)",
          all(int(b1[x["r"], x["q"]]) != MOUNTAIN for x in res))
    check(f"aether hard-capped 1..6 (scarce by construction): {by['aether']}", 1 <= by["aether"] <= 6)
    check("both chains have inputs: BREAD(grain&fuel) + ARMS(ore&aether)",
          by["grain"] > 0 and by["fuel"] > 0 and by["ore"] > 0 and by["aether"] > 0)
    check("yields are integers (no f32 weight crosses the freeze)",
          all(isinstance(x["yield"], int) for x in res))
    # attractor separation — the disjoint-chain driver: ore highland vs breadbasket pulled apart
    sep = hex_dist(f1["ore_att"], f1["bread_att"])
    check(f"attractor centres separated (ore↔bread hex dist {sep} ≥ 12)", sep >= 12)
    # all resources reachable from the capital over passable land (the player can route to every node)
    preach = hex_flood(land & (b1 != MOUNTAIN), cap1)
    check("every resource reachable from the capital over passable land",
          all(bool(preach[x["r"], x["q"]]) for x in res))
    check("determinism: resource placement identical across two bakes",
          res == f2["resources"])

    # --- S3 towns: capital present, suitability/spacing, reachability, expansion-arc grading ---
    towns = f1["towns"]
    tby = _C(t["kind"] for t in towns)
    check(f"towns placed (capital + starter + neutrals): {dict(tby)}",
          tby["capital"] == 1 and tby["starter"] == 1 and tby["neutral"] >= 3)
    check("every town on passable land, reachable from the capital",
          all(int(b1[t["r"], t["q"]]) not in (WATER, MOUNTAIN) and bool(preach[t["r"], t["q"]]) for t in towns))
    neutrals = [t for t in towns if t["kind"] != "capital"]
    check("towns Poisson-spread (pairwise hex spacing ≥ budget)",
          all(hex_dist((a["q"], a["r"]), (b["q"], b["r"])) >= TOWN_MIN_SPACING
              for i, a in enumerate(neutrals) for b in neutrals[i + 1:]))
    check("town values are integers", all(isinstance(t["value"], int) for t in towns))
    check("expansion arc: the farthest town outvalues the nearest (distance-graded prize)",
          max(neutrals, key=lambda t: hex_dist((t["q"], t["r"]), cap1))["value"]
          >= min(neutrals, key=lambda t: hex_dist((t["q"], t["r"]), cap1))["value"])
    check("every town has a 2–3 good demand set", all(1 <= len(t["demands"]) <= 3 for t in towns))
    check("determinism: town placement identical across two bakes", towns == f2["towns"])
    # S7e-2 + S7e multi-stage: every sink town has a 2-input chain recipe — BREAD = grain+fuel (2-stage) or
    # ARMS = INGOT+aether (3-STAGE: ore→forge→INGOT). BOTH chains have consumers (the disjoint-chain payoff).
    sink_recipes = [sorted(t["recipe"]) for t in towns if t["kind"] != "capital"]
    n_bread = sum(1 for r in sink_recipes if r == sorted(BREAD_RECIPE))
    n_arms = sum(1 for r in sink_recipes if r == sorted(ARMS_RECIPE))
    check(f"every sink town demands a full chain (BREAD×{n_bread} + ARMS×{n_arms}); both chains consumed",
          all(sorted(r) in (sorted(BREAD_RECIPE), sorted(ARMS_RECIPE)) for r in sink_recipes) and n_bread > 0 and n_arms > 0)
    # S7e multi-stage: the ARMS chain needs forges (ore→INGOT) — at least one, reachable from the capital.
    check(f"S7e: forges sited for the 3-stage ARMS chain ({len(f1['forges'])})", len(f1["forges"]) >= 1)

    # --- S4 decadence seed: clean capital + grace, corrupt frontier, loseable reservoir ---
    dec = f1["decadence"]
    cap_town = next(t for t in towns if t["kind"] == "capital")
    starter = next((t for t in towns if t["kind"] == "starter"), None)
    check("capital + starter start CLEAN (decadence 0)",
          cap_town["decadence"] == 0 and (starter is None or starter["decadence"] == 0))
    far_neutrals = [t for t in neutrals
                    if t["kind"] == "neutral" and hex_dist((t["q"], t["r"]), cap1) > CAPITAL_GRACE_HEXES]
    check(f"the frontier is corrupt (NEUTRAL towns past the grace ring have a decadence floor): "
          f"{[t['decadence'] for t in far_neutrals]}",
          all(t["decadence"] > 0 for t in far_neutrals))
    check("decadence floor rises with frontier depth (deeper town ≥ shallower)",
          (max(neutrals, key=lambda t: hex_dist((t["q"], t["r"]), cap1))["decadence"]
           >= min(neutrals, key=lambda t: hex_dist((t["q"], t["r"]), cap1))["decadence"]))
    check("decadence values are integers", all(isinstance(t["decadence"], int) for t in towns))
    rsv = dec["reservoir"]
    check(f"reservoir anchors exist ({len(rsv)}) on passable land", len(rsv) == RESERVOIR_ANCHORS
          and all(int(b1[a["r"], a["q"]]) not in (WATER, MOUNTAIN) for a in rsv))
    check("LOSEABLE: every reservoir anchor reaches the capital over passable land (capital not walled off)",
          all(bool(preach[a["r"], a["q"]]) for a in rsv))
    rmin = min(hex_dist((a["q"], a["r"]), cap1) for a in rsv)
    check(f"reservoir is the FAR edge opposite the capital (nearest anchor {rmin} hexes out > grace {CAPITAL_GRACE_HEXES})",
          rmin > CAPITAL_GRACE_HEXES * 2)
    check(f"baked starting decadence seeded (0 < {dec['initialDecadence']} < capital threshold 20000)",
          0 < dec["initialDecadence"] < 20000 and isinstance(dec["initialDecadence"], int))
    check("determinism: decadence seed identical across two bakes", dec == f2["decadence"])

    # --- S6 solvability validator + re-roll: the bake certifies a winnable world ---
    cert, cb, ccap, cf = generate_valid(seed)[:4]
    cert_fails = validate(cb, ccap, cf)
    check(f"generate_valid({seed}) yields a CERTIFIED-winnable world (seed {cert}): {cert_fails or 'OK'}",
          cert_fails == [])
    cert2 = generate_valid(seed)[0]
    check(f"determinism: certified seed reproducible ({cert} == {cert2})", cert == cert2)
    # the validator has teeth — it must REJECT a deliberately starved world (no resources at all)
    starved = {**f1, "resources": [], "decadence": {"reservoir": [], "capitalGraceHexes": 6},
               "ore_att": f1["ore_att"], "bread_att": f1["bread_att"]}
    check("validator rejects a starved world (has teeth)", validate(b1, cap1, starved) != [])
    by_cert = _C(x["kind"] for x in cf["resources"])
    check(f"certified world: aether in [{V_AETHER_MIN},{V_AETHER_MAX}] + chains supplied: {dict(by_cert)}",
          V_AETHER_MIN <= by_cert["aether"] <= V_AETHER_MAX
          and all(by_cert[k] >= V_MIN_PER_KIND for k in ("grain", "fuel", "ore")))

    # --- S6 relaxation ladder: total-function termination guarantee + the disjoint hard/relaxable sets ---
    relaxations = generate_valid(seed)[5]
    check(f"the committed seed certifies STRICT (no relaxation needed): {relaxations or 'strict'}",
          relaxations == [])
    check("relaxation-ladder keys are all RELAXABLE (disjoint from the HARD constraints)",
          set(k for k, _ in RELAX_LADDER) <= set(RELAXABLE_DEFAULT))
    # the new HARD 'aether before the tide' constraint has teeth: a world whose aether sits FARTHER from the
    # capital than its reservoir must be rejected (the arcane is unreachable before the realm falls).
    far_aether = {**cf, "resources": [{"kind": "aether", "q": ccap[0], "r": ccap[1] + 40, "yield": 40}],
                  "decadence": {**cf["decadence"], "reservoir": [{"q": ccap[0], "r": ccap[1] + 5}]}}
    check("validator HARD-rejects aether-farther-than-the-reservoir (reach-the-arcane-first has teeth)",
          any("arcane before the tide" in f for f in validate(cb, ccap, far_aether)))

    check("determinism: forge placement identical across two bakes (S7e multi-stage)", f1["forges"] == f2["forges"])

    # --- RIVERS: sink-fill leaves no pits, drainage forest is acyclic + downhill, flux conserved, deterministic ---
    land1 = b1 != WATER
    ef = f1["elev_filled"]

    def _has_outlet(q, r):
        for dq, dr in AXIAL_DIRS:
            nq, nr = q + dq, r + dr
            if not in_bounds(nq, nr) or not land1[nr, nq]:
                return True  # drains off the continent (sea/edge)
            if float(ef[nr, nq]) < float(ef[r, q]):
                return True  # has a strictly-lower land neighbour
        return False

    pits = sum(1 for r in range(H) for q in range(W) if land1[r, q] and not _has_outlet(q, r))
    check(f"rivers: priority-flood leaves 0 interior pits ({pits})", pits == 0)
    redges, rparent, rflux, _rt = compute_rivers(ef, land1)
    acyclic = True
    for r in range(H):
        for q in range(W):
            if not land1[r, q] or (q, r) not in rparent:
                continue
            node, seen_n, steps = (q, r), set(), 0
            while node in rparent and steps <= W * H:
                if node in seen_n:
                    acyclic = False
                    break
                seen_n.add(node)
                node = rparent[node]
                steps += 1
            if not acyclic:
                break
        if not acyclic:
            break
    check("rivers: drainage forest is acyclic (every cell reaches an outlet)", acyclic)
    root_flux = sum(float(rflux[r, q]) for r in range(H) for q in range(W)
                    if land1[r, q] and (q, r) not in rparent)
    check(f"rivers: flux conserved (root flux {root_flux:.0f} == land {int(land1.sum())})",
          abs(root_flux - float(int(land1.sum()))) < 0.5)
    downhill = all(float(ef[r, q]) >= float(ef[tr, tq]) for (q, r, tq, tr, _w, _f) in redges)
    check(f"rivers: every edge flows downhill ({len(redges)} river edges, ≥1 trunk)", downhill and len(redges) >= 1)
    check("rivers: identical across two bakes (deterministic)", f1["rivers"] == f2["rivers"])

    # determinism through serialization (the frozen artifact is byte-stable)
    c1 = emit("__selftest_a", seed, b1, cap1, res, towns, dec, f1["forges"], f1["rivers"])
    c2 = emit("__selftest_b", seed, b2, cap2, f1["resources"], f1["towns"], f2["decadence"], f2["forges"], f2["rivers"])
    check("determinism: serialized cells byte-identical",
          json.dumps(c1, sort_keys=True) == json.dumps(c2, sort_keys=True))
    for suf in ("a", "b"):
        for kind in ("world", "buildability", "demand"):
            p = os.path.join(OUT, f"__selftest_{suf}_{kind}.json")
            if os.path.exists(p):
                os.remove(p)

    print(f"\n  biome histogram: {dict(sorted(hist.items()))}  (land cells: {n_land})")
    print(f"  resources: {dict(by)}  towns: {dict(tby)}  attractors ore={f1['ore_att']} bread={f1['bread_att']}")
    print(f"\n{ascii_preview(b1, cap1, resources=res, towns=towns)}\n")
    print("SELFTEST:", "PASS" if ok else "FAIL")
    return ok


def main():
    seed = DEFAULT_SEED
    do_test = False
    cid = "fantasy"
    for a in sys.argv[1:]:
        if a == "--selftest":
            do_test = True
        elif a.isdigit():
            seed = int(a)
        else:
            cid = a
    if do_test:
        sys.exit(0 if selftest(seed) else 1)
    # S6: certify the world (re-roll from the requested seed until solvability passes) so the committed bake
    # is always winnable; the manifest records the CERTIFIED seed (so loading reproduces it).
    cert, biome, capital, fields, fails, relaxations = generate_valid(seed)
    if fails:
        print(f"  !! WARNING: ladder exhausted; world NOT certified — violations: {fails}")
    elif relaxations:
        print(f"  [S6] requested seed {seed} not strictly winnable; certified seed {cert} with relaxations: {relaxations}")
    elif cert != seed:
        print(f"  [S6] requested seed {seed} not winnable; certified seed {cert} (strict)")
    seed = cert
    cells = emit(cid, seed, biome, capital, fields["resources"], fields["towns"], fields["decadence"], fields["forges"], fields["rivers"])
    hist = biome_hist(biome)
    from collections import Counter
    rby = Counter(x["kind"] for x in fields["resources"])
    tby = Counter(t["kind"] for t in fields["towns"])
    print(f"[build_world] {cid}: seed={seed} {len(cells)} cells, "
          f"land={int((biome != WATER).sum())}, passes_carved={fields['passes']}, capital(q,r)={capital}")
    print(f"  biomes: {dict(sorted(hist.items()))}  resources: {dict(rby)}  towns: {dict(tby)}")
    print(f"\n{ascii_preview(biome, capital, resources=fields['resources'], towns=fields['towns'])}")


if __name__ == "__main__":
    main()
