// Procedural LOWPOLY pine for the fantasy 3D diorama (#3d-trees). A flat-shaded, non-indexed triangle
// list — two stacked 6-gon cones (a layered evergreen) plus a short trunk — built ONCE and instanced
// across the forest hexes by a SimpleMeshLayer. Z is up (altitude); the mesh is ~1 unit tall and gets
// scaled to map-metres by the layer's sizeScale. Per-face normals give the faceted lowpoly read under
// the deck lighting. Tinted per-instance via the layer's getColor (so the mesh itself is white-ish).
import { Geometry } from "@luma.gl/engine";
import { bakeAO } from "./meshAO";

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

/** A cone of `seg` sides: base ring (centre z=z0, radius r) up to an apex at z=z1. Side faces only
 *  (the base is hidden against the tier below / the ground), wound CCW so normals face outward. */
function cone(pos: number[], nrm: number[], z0: number, z1: number, r: number, seg: number): void {
  const apex: V3 = [0, 0, z1];
  for (let i = 0; i < seg; i++) {
    const a0 = (i / seg) * Math.PI * 2;
    const a1 = ((i + 1) / seg) * Math.PI * 2;
    const p0: V3 = [Math.cos(a0) * r, Math.sin(a0) * r, z0];
    const p1: V3 = [Math.cos(a1) * r, Math.sin(a1) * r, z0];
    tri(pos, nrm, p0, p1, apex);
  }
}

/** A short square trunk (a 4-sided prism, z0→z1, half-width w) — side faces only. */
function trunk(pos: number[], nrm: number[], z0: number, z1: number, w: number): void {
  const ring: V3[] = [[-w, -w, 0], [w, -w, 0], [w, w, 0], [-w, w, 0]];
  for (let i = 0; i < 4; i++) {
    const a = ring[i], b = ring[(i + 1) % 4];
    const a0: V3 = [a[0], a[1], z0], a1: V3 = [a[0], a[1], z1];
    const b0: V3 = [b[0], b[1], z0], b1: V3 = [b[0], b[1], z1];
    tri(pos, nrm, a0, b0, b1);
    tri(pos, nrm, a0, b1, a1);
  }
}

let cached: Geometry | null = null;

/** The shared lowpoly pine geometry (built once). ~1 unit tall, base at z=0. */
export function pineGeometry(): Geometry {
  if (cached) return cached;
  const pos: number[] = [];
  const nrm: number[] = [];
  trunk(pos, nrm, 0, 0.18, 0.06); // stubby trunk
  cone(pos, nrm, 0.12, 0.62, 0.34, 6); // bottom tier (widest)
  cone(pos, nrm, 0.4, 0.86, 0.26, 6); // mid tier
  cone(pos, nrm, 0.66, 1.06, 0.17, 6); // top tier (a pointed crown)
  cached = new Geometry({
    topology: "triangle-list",
    attributes: {
      POSITION: { size: 3, value: new Float32Array(pos) },
      NORMAL: { size: 3, value: new Float32Array(nrm) },
      // Baked AO: the trunk/base sinks into shadow, the crown stays lit → real evergreen volume.
      COLOR_0: { size: 3, value: bakeAO(pos, nrm, { floor: 0.5 }) },
    },
  });
  return cached;
}
