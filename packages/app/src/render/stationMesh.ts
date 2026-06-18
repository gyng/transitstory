// Procedural LOWPOLY station DEPOT for the fantasy 3D diorama (#3d-stations). A flat-shaded, non-indexed
// triangle list — a raised platform slab + a little pitched-roof station house — built ONCE and instanced
// across the player's stations by a SimpleMeshLayer (mirrors treeMesh/legionMesh). Z is up; the mesh is
// ~1 unit footprint, ~0.7 tall, scaled to map-metres by the layer. Per-face normals give the faceted
// lowpoly read under the deck lighting; baked AO sinks the platform underside + the eaves into shadow so
// the depot reads as real volume. The walls are white-ish; the layer tints per-instance (served vs idle).
import { Geometry } from "@luma.gl/engine";
import { bakeAO } from "./meshAO";

type V3 = [number, number, number];

/** Push one flat-shaded triangle (computes + repeats the outward face normal across its 3 verts). */
function tri(pos: number[], nrm: number[], a: V3, b: V3, c: V3): void {
  const ux = b[0] - a[0], uy = b[1] - a[1], uz = b[2] - a[2];
  const vx = c[0] - a[0], vy = c[1] - a[1], vz = c[2] - a[2];
  let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
  const len = Math.hypot(nx, ny, nz) || 1;
  nx /= len; ny /= len; nz /= len;
  for (const p of [a, b, c]) {
    pos.push(p[0], p[1], p[2]);
    nrm.push(nx, ny, nz);
  }
}

/** A quad as two triangles (a,b,c,d wound CCW for an outward normal). */
function quad(pos: number[], nrm: number[], a: V3, b: V3, c: V3, d: V3): void {
  tri(pos, nrm, a, b, c);
  tri(pos, nrm, a, c, d);
}

/** An axis-aligned box [x0,x1]×[y0,y1]×[z0,z1] — 4 walls + a top (the bottom is hidden on the ground/slab). */
function box(pos: number[], nrm: number[], x0: number, x1: number, y0: number, y1: number, z0: number, z1: number): void {
  quad(pos, nrm, [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]); // -Y wall
  quad(pos, nrm, [x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]); // +Y wall
  quad(pos, nrm, [x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]); // +X wall
  quad(pos, nrm, [x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]); // -X wall
  quad(pos, nrm, [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]); // top
}

let cached: Geometry | null = null;

/** The shared lowpoly depot geometry (built once). Footprint ~1×0.7, base at z=0. A platform slab with a
 *  small house set back on it, capped by a pitched roof ridged along Y (the platform edge = the berth a
 *  train pulls up to; the player's vehicles + peeps animate there, so the static depot reads as "working"). */
export function stationMesh(): Geometry {
  if (cached) return cached;
  const pos: number[] = [];
  const nrm: number[] = [];
  // 1) PLATFORM slab — wide + low, the berth face along +X (where the track/train sits).
  box(pos, nrm, -0.5, 0.5, -0.35, 0.35, 0.0, 0.12);
  // 2) station HOUSE — a smaller block set toward the back (−X) so the front edge stays open as the berth.
  const hx0 = -0.46, hx1 = 0.04, hy0 = -0.26, hy1 = 0.26, hz0 = 0.12, hz1 = 0.46;
  box(pos, nrm, hx0, hx1, hy0, hy1, hz0, hz1);
  // 3) pitched ROOF over the house — a ridge running along Y at the peak, eaves overhanging a touch.
  const ex0 = hx0 - 0.05, ex1 = hx1 + 0.05; // eave overhang in X
  const ey0 = hy0 - 0.06, ey1 = hy1 + 0.06; // gable overhang in Y
  const eaveZ = hz1, peakZ = hz1 + 0.22, mx = (ex0 + ex1) / 2;
  // the two pitched faces, wound for up-facing normals (−X slope faces −X/+Z, +X slope faces +X/+Z).
  quad(pos, nrm, [ex0, ey0, eaveZ], [mx, ey0, peakZ], [mx, ey1, peakZ], [ex0, ey1, eaveZ]); // −X slope
  quad(pos, nrm, [mx, ey0, peakZ], [ex1, ey0, eaveZ], [ex1, ey1, eaveZ], [mx, ey1, peakZ]); // +X slope
  // the two gable triangles (the Y ends), wound for ∓Y normals.
  tri(pos, nrm, [ex0, ey0, eaveZ], [ex1, ey0, eaveZ], [mx, ey0, peakZ]); // −Y gable
  tri(pos, nrm, [ex1, ey1, eaveZ], [ex0, ey1, eaveZ], [mx, ey1, peakZ]); // +Y gable
  cached = new Geometry({
    topology: "triangle-list",
    attributes: {
      POSITION: { size: 3, value: new Float32Array(pos) },
      NORMAL: { size: 3, value: new Float32Array(nrm) },
      // Baked AO: the platform underside + under the eaves sink into shadow, the roof ridge stays lit.
      COLOR_0: { size: 3, value: bakeAO(pos, nrm, { floor: 0.55 }) },
    },
  });
  return cached;
}
