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
  /** Optional committed real-world starting network (e.g. the MRT). */
  networkPath?: string;
  /** Optional committed buildability grid (surface-rail cost signal). */
  buildabilityPath?: string;
}

export interface RawDemand {
  cellM: number;
  bbox: [number, number, number, number];
  cells: { lon: number; lat: number; originWeight: number; destWeight: number }[];
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
}

/** Pure: build the core (mm) city JSON from the raw manifest + lng/lat grids. */
export function buildCoreCity(
  raw: RawCity,
  demand: RawDemand,
  buildability?: RawBuildability,
): { json: string; cellCount: number } {
  const cells = demand.cells.map((c) => {
    const [x_mm, y_mm] = lngLatToMm([c.lon, c.lat]);
    return { x_mm, y_mm, origin_w: c.originWeight, dest_w: c.destWeight };
  });
  const core: Record<string, unknown> = { seed: raw.seed, demand: { cell_m: demand.cellM, cells } };
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
  return { raw, seed: raw.seed, coreCityJson: json, demandCellCount: cellCount, buildability, demandHeat };
}
