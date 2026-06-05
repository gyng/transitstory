// THE coordinate boundary. All lng/lat <-> metres <-> mm and Web-Mercator-adjacent math
// lives here and nowhere else (AGENTS non-negotiable #4). The sim core only ever sees
// integer millimetres; the map only ever sees lng/lat. Equirectangular linearization around
// a fixed origin is sub-metre accurate over one city. The origin is settable so a session
// can run any city (setOrigin is called once at boot from the city manifest).
import { SG_ORIGIN } from "../config";

export type LngLat = [number, number];
export type Meters = [number, number];
export type Mm = [number, number];

const M_PER_DEG_LAT = 110540;
let originLng: number = SG_ORIGIN.lng;
let originLat: number = SG_ORIGIN.lat;
let mPerDegLng = 111320 * Math.cos((originLat * Math.PI) / 180);

/** Set the local-frame origin for this session (once, at boot). */
export function setOrigin(lng: number, lat: number): void {
  originLng = lng;
  originLat = lat;
  mPerDegLng = 111320 * Math.cos((originLat * Math.PI) / 180);
}

export function lngLatToMeters([lng, lat]: LngLat): Meters {
  return [(lng - originLng) * mPerDegLng, (lat - originLat) * M_PER_DEG_LAT];
}

export function metersToLngLat([x, y]: Meters): LngLat {
  return [originLng + x / mPerDegLng, originLat + y / M_PER_DEG_LAT];
}

/** Non-allocating metres→lng/lat: writes `[lng, lat]` into `out` at `off`. Same math as
 *  `metersToLngLat`, but for tight per-element fills (e.g. thousands of peep dots per frame) where
 *  allocating a 2-tuple per point would churn the GC. Keeps the projection inside this one module. */
export function metersToLngLatInto(x: number, y: number, out: Float32Array, off: number): void {
  out[off] = originLng + x / mPerDegLng;
  out[off + 1] = originLat + y / M_PER_DEG_LAT;
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
