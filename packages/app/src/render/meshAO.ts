// Bake a cheap per-vertex AMBIENT-OCCLUSION darkening into a COLOR_0 attribute (size 3, 0..1 grayscale)
// for the lowpoly meshes. deck's SimpleMeshLayer MULTIPLIES this mesh `colors` by the per-instance
// getColor in its vertex shader (`vColor = colors * instanceColors.rgb`), so a darker value at a vertex
// darkens the line colour THERE — free at render, no extra pass. This is the cost-effective stand-in for
// screen-space SSAO (which deck's overlay can't do: PostProcessEffect gets no depth/normal G-buffer, and
// interleaved:false can't read basemap depth).
//
// Z is up in every mesh (base near z=0): vertices LOWER to the ground read more occluded, and DOWNWARD-
// facing faces (undersides) darker still — so the bases sink into contact shadow and the boxy forms gain
// crevice depth. Baked once at geometry-build time (the meshes are module-cached singletons).
export function bakeAO(pos: number[], nrm: number[], opts: { floor?: number; normMix?: number } = {}): Float32Array {
  const floor = opts.floor ?? 0.55; // brightness at the very base (1 = no darkening)
  const normMix = opts.normMix ?? 0.82; // 1 = ignore face normals; lower = undersides darker
  const n = pos.length / 3;
  let zmin = Infinity;
  let zmax = -Infinity;
  for (let i = 0; i < n; i++) {
    const z = pos[i * 3 + 2];
    if (z < zmin) zmin = z;
    if (z > zmax) zmax = z;
  }
  const span = zmax - zmin || 1;
  const col = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const hfrac = (pos[i * 3 + 2] - zmin) / span; // 0 at the base, 1 at the top
    const heightAO = floor + (1 - floor) * hfrac;
    const normFrac = Math.max(0, Math.min(1, 0.5 + 0.5 * nrm[i * 3 + 2])); // underside 0 → top 1
    const ao = Math.max(0.32, Math.min(1, heightAO * (normMix + (1 - normMix) * normFrac)));
    col[i * 3] = ao;
    col[i * 3 + 1] = ao;
    col[i * 3 + 2] = ao;
  }
  return col;
}
