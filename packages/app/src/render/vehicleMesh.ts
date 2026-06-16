// Procedural LOWPOLY vehicle for the 3D world (#3d-vehicles). A flat-shaded, non-indexed triangle list —
// a boxy car/cart with a sloped front (so its heading reads) on a low chassis — built ONCE and instanced
// across the live vehicles by a SimpleMeshLayer. FORWARD is +X (the layer yaws it to each vehicle's
// heading); Z is up (altitude). ~1 unit long; the layer scales it to map-metres. Tinted per-instance via
// getColor (the mesh itself is white-ish so the line colour reads). Mirrors render/treeMesh.ts.
import { Geometry } from "@luma.gl/engine";

type V3 = [number, number, number];

/** Push one flat-shaded triangle (computes + repeats the face normal across its 3 verts). */
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

/** A quad (two tris, CCW from a→b→c→d) — for the boxy faces. */
function quad(pos: number[], nrm: number[], a: V3, b: V3, c: V3, d: V3): void {
  tri(pos, nrm, a, b, c);
  tri(pos, nrm, a, c, d);
}

/** A box from (x0,y0,z0) to (x1,y1,z1) — 6 faces, outward normals. The FRONT (+X) top can be pulled in by
 *  `frontTaper` (0 = a plain box, >0 = a sloped windscreen/prow so the heading reads). */
function box(pos: number[], nrm: number[], x0: number, y0: number, z0: number, x1: number, y1: number, z1: number, frontTaper = 0): void {
  const t = frontTaper * (x1 - x0); // how far the front-top edge is pulled back (toward -X)
  // 8 corners; the two FRONT-TOP corners (x1) are pulled back by t to slope the prow.
  const A: V3 = [x0, y0, z0], B: V3 = [x1, y0, z0], C: V3 = [x1, y1, z0], D: V3 = [x0, y1, z0]; // bottom
  const E: V3 = [x0, y0, z1], F: V3 = [x1 - t, y0, z1], G: V3 = [x1 - t, y1, z1], H: V3 = [x0, y1, z1]; // top
  quad(pos, nrm, A, D, C, B); // bottom (normal -Z)
  quad(pos, nrm, E, F, G, H); // top
  quad(pos, nrm, A, B, F, E); // -Y side
  quad(pos, nrm, D, H, G, C); // +Y side
  quad(pos, nrm, A, E, H, D); // -X (back) face
  quad(pos, nrm, B, C, G, F); // +X (front) face — sloped when t>0
}

let cached: Geometry | null = null;

/** The shared lowpoly vehicle geometry (built once). ~1 unit long (X = forward), base near z=0. A chassis
 *  + a cabin box with a sloped prow, so an instance reads as a little car/cart pointing along its travel. */
export function vehicleMesh(): Geometry {
  if (cached) return cached;
  const pos: number[] = [];
  const nrm: number[] = [];
  // Chassis: a thin low slab the full footprint (reads as the cart bed / wheels' line).
  box(pos, nrm, -0.5, -0.26, 0.0, 0.5, 0.26, 0.12);
  // Cabin/body: a taller box, slightly inset, with a sloped front prow (the heading tell).
  box(pos, nrm, -0.44, -0.22, 0.12, 0.46, 0.22, 0.46, 0.42);
  cached = new Geometry({
    topology: "triangle-list",
    attributes: {
      POSITION: { size: 3, value: new Float32Array(pos) },
      NORMAL: { size: 3, value: new Float32Array(nrm) },
    },
  });
  return cached;
}

let wagonCached: Geometry | null = null;

/** The shared CARGO-WAGON geometry (built once) — a flatcar the locomotive pulls (#multi-car): a low bed
 *  slab with short end-walls, no cabin/prow (it's hauled, not driven), so a string of them reads as a
 *  freight consist behind the loco. FORWARD is +X; the layer yaws each to its track tangent so the train
 *  curves. Slightly shorter than the loco (~0.84 long) so cars read as separate units. Line-coloured via
 *  getColor; the load lump (cargoMesh) rides on top, commodity-coloured. */
export function wagonMesh(): Geometry {
  if (wagonCached) return wagonCached;
  const pos: number[] = [];
  const nrm: number[] = [];
  // Flatbed: a thin low slab (the wagon floor + underframe).
  box(pos, nrm, -0.42, -0.24, 0.0, 0.42, 0.24, 0.14);
  // Low end-walls (front + back) so the bed reads as a freight wagon, not a plank.
  box(pos, nrm, -0.42, -0.24, 0.14, -0.34, 0.24, 0.3);
  box(pos, nrm, 0.34, -0.24, 0.14, 0.42, 0.24, 0.3);
  wagonCached = new Geometry({
    topology: "triangle-list",
    attributes: {
      POSITION: { size: 3, value: new Float32Array(pos) },
      NORMAL: { size: 3, value: new Float32Array(nrm) },
    },
  });
  return wagonCached;
}

let cargoCached: Geometry | null = null;

/** The shared CARGO geometry (built once) — a unit box (footprint 0.7×0.36, height 1) sitting on the car
 *  bed, scaled in Z by the layer per-instance so its HEIGHT reads the load (#in-world-cargo). Flat-shaded. */
export function cargoMesh(): Geometry {
  if (cargoCached) return cargoCached;
  const pos: number[] = [];
  const nrm: number[] = [];
  box(pos, nrm, -0.35, -0.18, 0.0, 0.35, 0.18, 1.0);
  cargoCached = new Geometry({
    topology: "triangle-list",
    attributes: {
      POSITION: { size: 3, value: new Float32Array(pos) },
      NORMAL: { size: 3, value: new Float32Array(nrm) },
    },
  });
  return cargoCached;
}
