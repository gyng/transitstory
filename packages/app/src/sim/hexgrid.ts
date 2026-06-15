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
