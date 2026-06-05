// Frontend buildability lookup (mirrors the sim's grid) for LIVE blueprint coloring while
// drawing. Same Math.floor cell indexing as the Rust div_euclid lookup so TS and sim agree.
import { lngLatToMm } from "../coords/geo";
import type { RawBuildability } from "./city";

export const BUILD = { OPEN: 0, ROAD: 1, RAIL: 2, BUILT: 3, WATER: 4, PARK: 5 } as const;

export class Buildability {
  private cells = new Map<string, number>();
  readonly cellMm: number;

  constructor(grid?: RawBuildability) {
    this.cellMm = grid && grid.cellM > 0 ? grid.cellM * 1000 : 120_000;
    if (grid) {
      for (const c of grid.cells) {
        const [x, y] = lngLatToMm([c.lon, c.lat]);
        this.cells.set(this.key(x, y), c.c);
      }
    }
  }

  private key(xMm: number, yMm: number): string {
    return `${Math.floor(xMm / this.cellMm)},${Math.floor(yMm / this.cellMm)}`;
  }

  classifyMm(xMm: number, yMm: number): number {
    return this.cells.get(this.key(xMm, yMm)) ?? BUILD.OPEN;
  }

  classifyLngLat(lng: number, lat: number): number {
    const [x, y] = lngLatToMm([lng, lat]);
    return this.classifyMm(x, y);
  }

  /** Cell-centre mm positions of every cell of class `klass` (e.g. BUILD.ROAD) — for the map
   *  overlay that shows where buses are cheap + fast. Computed once, memoized by the caller. */
  cellsMm(klass: number): [number, number][] {
    const half = this.cellMm / 2;
    const out: [number, number][] = [];
    for (const [k, c] of this.cells) {
      if (c !== klass) continue;
      const comma = k.indexOf(",");
      const cx = Number(k.slice(0, comma));
      const cy = Number(k.slice(comma + 1));
      out.push([cx * this.cellMm + half, cy * this.cellMm + half]);
    }
    return out;
  }

  get loaded(): boolean {
    return this.cells.size > 0;
  }
}
