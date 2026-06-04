// THE coordinate boundary. All lng/lat <-> metres <-> mm and Web-Mercator-adjacent math
// lives here and nowhere else (AGENTS non-negotiable #4). The sim core only ever sees
// integer millimetres; the map only ever sees lng/lat. Equirectangular linearization
// around the fixed Singapore origin is sub-metre accurate over one city.
import { SG_ORIGIN } from "../config";

export type LngLat = [number, number];
export type Meters = [number, number];
export type Mm = [number, number];

const M_PER_DEG_LAT = 110540;
const cosLat0 = Math.cos((SG_ORIGIN.lat * Math.PI) / 180);
const M_PER_DEG_LNG = 111320 * cosLat0;

export function lngLatToMeters([lng, lat]: LngLat): Meters {
  return [(lng - SG_ORIGIN.lng) * M_PER_DEG_LNG, (lat - SG_ORIGIN.lat) * M_PER_DEG_LAT];
}

export function metersToLngLat([x, y]: Meters): LngLat {
  return [SG_ORIGIN.lng + x / M_PER_DEG_LNG, SG_ORIGIN.lat + y / M_PER_DEG_LAT];
}

export function metersToMm([x, y]: Meters): Mm {
  return [Math.round(x * 1000), Math.round(y * 1000)];
}

export function mmToMeters([x, y]: Mm): Meters {
  return [x / 1000, y / 1000];
}

export function lngLatToMm(ll: LngLat): Mm {
  return metersToMm(lngLatToMeters(ll));
}

export function mmToLngLat(mm: Mm): LngLat {
  return metersToLngLat(mmToMeters(mm));
}
