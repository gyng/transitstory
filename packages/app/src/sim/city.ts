// Loads the committed city manifest + demand grid and builds the core city JSON that
// Sim::new consumes. The demand grid is stored in lng/lat; we convert each cell to local
// mm HERE (coords/geo.ts boundary) so the sim only ever sees mm (snake_case keys match the
// Rust serde field names of DemandGrid/DemandCell — NOT camelCase).
import { lngLatToMm, setOrigin } from "../coords/geo";
import { withBase } from "../config";

export interface RawCity {
  id: string;
  name: string;
  originLngLat: [number, number];
  bbox: [number, number, number, number];
  center: [number, number];
  zoom: number;
  seed: number;
  demandGridPath: string;
  /** Which engine ruleset to construct (fantasy-fork.md): absent/"transit" = the classic game;
   *  "arcadia" = the hex 4X-logistics fantasy campaign. Mirrors `CityData.ruleset`. */
  ruleset?: string;
  /** Hex-lattice cell size (mm) for the fantasy map — mirrors `CityData.grid_cell_mm`. Absent/0 =
   *  the continuous Catmull-Rom geometry (every transit city). >0 builds track on the hex lattice. */
  gridCellMm?: number;
  /** Optional committed real-world starting network (e.g. the MRT). */
  networkPath?: string;
  /** Optional committed buildability grid (surface-rail cost signal). */
  buildabilityPath?: string;
  /** Fantasy (arcadia) supply graph baked by scripts/build_world.py — an ADDITIVE manifest field
   *  (serde-ignored elsewhere; never copied into the core city JSON). S2: resource nodes that fork the
   *  two supply chains. Positions carried as axial (q,r) + i64 mm (= hexgrid::center_of); yields i64. */
  supplyGraph?: {
    resources: { kind: string; q: number; r: number; xMm: number; yMm: number; yield: number }[];
    /** S3 towns: supply sinks + conquest targets. kind = "capital"|"starter"|"neutral"; value = i64
     *  conquest reward; demands = nearest resource kinds; decadence = S4 per-town corruption floor. */
    towns?: { kind: string; q: number; r: number; xMm: number; yMm: number; value: number; demands: string[]; decadence: number; recipe?: number[] }[];
    /** S4 decadence seed: the far-edge reservoir (tide origin + raider anchors), the clean grace radius,
     *  and the realm's baked STARTING decadence (seeded into world.decadence). */
    decadenceSeed?: { capitalGraceHexes: number; reservoir: { q: number; r: number; xMm: number; yMm: number }[]; initialDecadence?: number; growthPerS?: number; armySpeedMmS?: number; creepPerS?: number; productionMicro?: number; capitalXMm?: number; capitalYMm?: number; influenceHops?: number; initialGold?: number; buildGoldDivisor?: number; goldUpkeepPerDay?: number; manpowerUpkeepPerLegionDay?: number; rivalEnabled?: boolean; walkBackstopMicro?: number };
  };
  /** Additive baked DRAINAGE topology (build_world.py flow-accumulation rivers) — render-only; never copied
   *  into the core city JSON (the sim never sees it). Each edge is a cell-centre→cell-centre segment (i64 mm
   *  endpoints) with a width class (1..4) + a ford flag (a cheap headwater crossing). */
  rivers?: { q: number; r: number; toQ: number; toR: number; x0Mm: number; y0Mm: number; x1Mm: number; y1Mm: number; wclass: number; ford: boolean }[];
  /** Optional per-city rider patience (sim-ms) — overrides the core's default. The globe sets an
   *  air-scale value (90_000 = 45 clock-min): air travellers arrive for a departure rather than
   *  drifting off after one missed metro interval, so its pressure is CAPACITY (denied boardings,
   *  aircraft choice), not schedule impatience. */
  patienceMs?: number;
}

export interface RawDemand {
  cellM: number;
  bbox: [number, number, number, number];
  /** `commodity` (fantasy S7e): the Forge-Line commodity a source cell produces (ORE=0 default). */
  cells: { lon: number; lat: number; originWeight: number; destWeight: number; commodity?: number }[];
}

export interface RawBuildability {
  cellM: number;
  bbox: [number, number, number, number];
  cells: { lon: number; lat: number; c: number }[];
}

export interface LoadedCity {
  raw: RawCity;
  seed: number;
  coreCityJson: string;
  demandCellCount: number;
  buildability?: RawBuildability;
  /** Demand grid as lng/lat heat points (origin+dest weight) for the demand map layer. */
  demandHeat: { lng: number; lat: number; weight: number }[];
  /** Demand grid cell pitch (m) — sizes the demand-heat hexagons so they tile the grid. */
  demandCellM: number;
}

/** Pure: build the core (mm) city JSON from the raw manifest + lng/lat grids. */
export function buildCoreCity(
  raw: RawCity,
  demand: RawDemand,
  buildability?: RawBuildability,
): { json: string; cellCount: number } {
  const cells = demand.cells.map((c) => {
    const [x_mm, y_mm] = lngLatToMm([c.lon, c.lat]);
    // commodity (fantasy S7e): which Forge-Line good a source cell produces (omitted ⇒ 0=ORE, serde default).
    return { x_mm, y_mm, origin_w: c.originWeight, dest_w: c.destWeight, commodity: c.commodity ?? 0 };
  });
  const core: Record<string, unknown> = { seed: raw.seed, demand: { cell_m: demand.cellM, cells } };
  if (raw.patienceMs !== undefined) core.patience_ms = raw.patienceMs; // per-city demand knob (city.rs)
  if (raw.ruleset) core.ruleset = raw.ruleset; // the fantasy-fork seam (World::new selects the mode)
  if (raw.ruleset === "arcadia") core.force_single_track = true; // fantasy: all track SINGLE (readability + forces meets/signals)
  if (raw.gridCellMm) core.grid_cell_mm = raw.gridCellMm; // hex lattice for the fantasy map
  const dec = raw.supplyGraph?.decadenceSeed;
  if (dec?.initialDecadence) core.initial_decadence = dec.initialDecadence; // baked starting corruption (S4)
  if (dec?.growthPerS) core.decadence_growth_per_s = dec.growthPerS; // baked lose-meter fill rate (balance)
  if (dec?.armySpeedMmS) core.army_speed_mm_s = dec.armySpeedMmS; // baked legion march speed (continent scale)
  if (dec?.capitalXMm) core.capital_x_mm = dec.capitalXMm; // baked capital cell (S10 decadence-tide target)
  if (dec?.capitalYMm) core.capital_y_mm = dec.capitalYMm;
  if (dec?.creepPerS) core.decadence_creep_per_s = dec.creepPerS; // baked tide creep rate (S10b-2)
  if (dec?.productionMicro) core.production_micro = dec.productionMicro; // baked economy pace (S11 — snappy 3-channel flow)
  if (dec?.influenceHops) core.influence_hops = dec.influenceHops; // baked area-of-influence radius (#9 — build gate)
  if (dec?.initialGold) core.initial_gold = dec.initialGold; // baked starting gold treasury (#economy)
  if (dec?.buildGoldDivisor) core.build_gold_divisor = dec.buildGoldDivisor; // baked gold build-cost scale (#economy)
  if (dec?.goldUpkeepPerDay) core.gold_upkeep_per_day = dec.goldUpkeepPerDay; // baked per-day gold upkeep (#economy opex)
  if (dec?.manpowerUpkeepPerLegionDay) core.manpower_upkeep_per_legion_day = dec.manpowerUpkeepPerLegionDay; // baked per-legion-day manpower upkeep (#daynight)
  if (dec?.rivalEnabled) core.rival_enabled = dec.rivalEnabled; // #13: seed a baked rival realm at construction
  if (dec?.walkBackstopMicro) core.walk_backstop_micro = dec.walkBackstopMicro; // baked off-rail goods backstop (#11)
  if (buildability) {
    core.buildability = {
      cell_m: buildability.cellM,
      cells: buildability.cells.map((c) => {
        const [x_mm, y_mm] = lngLatToMm([c.lon, c.lat]);
        return { x_mm, y_mm, c: c.c };
      }),
    };
  }
  return { json: JSON.stringify(core), cellCount: cells.length };
}

/** Fetch the manifest + grids, set the session's coordinate origin, and assemble the core
 *  city JSON. setOrigin MUST happen before buildCoreCity so lng/lat -> mm is correct. */
export async function loadCity(manifestPath: string): Promise<LoadedCity> {
  const raw: RawCity = await (await fetch(withBase(manifestPath))).json();
  setOrigin(raw.originLngLat[0], raw.originLngLat[1]);
  const demand: RawDemand = await (await fetch(withBase(raw.demandGridPath))).json();
  let buildability: RawBuildability | undefined;
  if (raw.buildabilityPath) {
    try {
      buildability = await (await fetch(withBase(raw.buildabilityPath))).json();
    } catch {
      buildability = undefined; // graceful: no penalties without the grid
    }
  }
  const { json, cellCount } = buildCoreCity(raw, demand, buildability);
  const demandHeat = demand.cells.map((c) => ({ lng: c.lon, lat: c.lat, weight: c.originWeight + c.destWeight }));
  return { raw, seed: raw.seed, coreCityJson: json, demandCellCount: cellCount, buildability, demandHeat, demandCellM: demand.cellM };
}
