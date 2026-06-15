// Frontend mirror of crates/sim/src/hexgrid.rs — pointy-top axial (q,r) over the same i64-mm planar
// space. Ported EXACTLY (same SQRT3 literal, same cube_round) so TS and the core agree on cell
// identity: a station snapped here lands on center_of(axial_of(p)), the very lattice vertex the core's
// grid_walk uses, so lines connect cleanly. Used only for the fantasy hex map (grid_cell_mm > 0).

const SQRT3 = 1.7320508075688772;

export type Axial = [q: number, r: number];

/** Cube-round a fractional cube coord to the nearest integer hex (Red Blob Games), repairing the axis
 *  with the largest delta so x+y+z==0 holds. */
function cubeRound(fx: number, fy: number, fz: number): [number, number, number] {
  let rx = Math.round(fx), ry = Math.round(fy), rz = Math.round(fz);
  const dx = Math.abs(rx - fx), dy = Math.abs(ry - fy), dz = Math.abs(rz - fz);
  if (dx > dy && dx > dz) rx = -ry - rz;
  else if (dy > dz) ry = -rx - rz;
  else rz = -rx - ry;
  return [rx, ry, rz];
}

/** Pixel (mm) → axial cell. `sizeMm` is the hex centre-to-corner size (= grid_cell_mm). */
export function axialOf(xMm: number, yMm: number, sizeMm: number): Axial {
  const s = sizeMm;
  const fq = (SQRT3 / 3.0 * xMm - yMm / 3.0) / s;
  const fr = (2.0 / 3.0 * yMm) / s;
  const [rx, , rz] = cubeRound(fq, -fq - fr, fr);
  return [rx, rz];
}

/** Axial cell → its centre in mm (the lattice vertex). Inverse of axialOf on cell centres. */
export function centerOf(q: number, r: number, sizeMm: number): [number, number] {
  const s = sizeMm;
  const x = s * (SQRT3 * q + (SQRT3 / 2.0) * r);
  const y = s * (1.5 * r);
  return [Math.round(x), Math.round(y)];
}

// The six pointy-top axial neighbour directions in rotational order — EXACTLY mirrors hexgrid.rs DIRS,
// so the ghost's one-bend route is byte-identical to what the core commits (same cells ⇒ same vertices).
const DIRS: Axial[] = [[1, 0], [1, -1], [0, -1], [-1, 0], [-1, 1], [0, 1]];

/** Cheapest one-BEND minimal hex line a→b (inclusive), scored by `cost` over interior cells — the exact
 *  TS mirror of `hexgrid.rs::line_costed`. Canonical (computed from the smaller endpoint, reversed for
 *  the larger) so the ghost matches the committed track. */
export function lineCosted(a: Axial, b: Axial, cost: (c: Axial) => number): Axial[] {
  const swap = a[0] > b[0] || (a[0] === b[0] && a[1] > b[1]);
  const lo = swap ? b : a;
  const hi = swap ? a : b;
  const dq = hi[0] - lo[0];
  const dr = hi[1] - lo[1];
  if (dq === 0 && dr === 0) return [lo];
  let k = 0, m = 0, d1: Axial = DIRS[0], d2: Axial = DIRS[1];
  for (let i = 0; i < 6; i++) {
    const e1 = DIRS[i], e2 = DIRS[(i + 1) % 6];
    const det = e1[0] * e2[1] - e1[1] * e2[0];
    if (det === 0) continue;
    const kk = (dq * e2[1] - dr * e2[0]) / det;
    const mm = (e1[0] * dr - e1[1] * dq) / det;
    if (kk >= 0 && mm >= 0) { k = kk; m = mm; d1 = e1; d2 = e2; break; }
  }
  const build = (first: Axial, nf: number, second: Axial, ns: number): Axial[] => {
    const v: Axial[] = [lo];
    let cur: Axial = lo;
    for (let i = 0; i < nf; i++) { cur = [cur[0] + first[0], cur[1] + first[1]]; v.push(cur); }
    for (let i = 0; i < ns; i++) { cur = [cur[0] + second[0], cur[1] + second[1]]; v.push(cur); }
    return v;
  };
  const score = (v: Axial[]): number => {
    let s = 0;
    for (let i = 1; i < v.length - 1; i++) s += cost(v[i]);
    return s;
  };
  const c1 = build(d1, k, d2, m);
  let v = c1;
  if (k > 0 && m > 0) {
    const c2 = build(d2, m, d1, k);
    if (score(c2) < score(c1)) v = c2;
  }
  if (swap) v.reverse();
  return v;
}
