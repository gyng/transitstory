// A custom luma.gl ShaderPass for deck's PostProcessEffect: ACES filmic tone-mapping + a subtle film
// grain + a gentle vignette, in ONE fullscreen pass over the deck overlay canvas. luma 9.3.3 ships no
// image-processing passes, so this is authored to its exact ShaderPass contract (deck's
// PostProcessEffect generates an fs that calls `<name>_filterColor`, signature
// `vec4 fn(vec4 color, vec2 texSize, vec2 texCoord)`; uniforms come from a std140 block named
// `<name>Uniforms` whose fields match `uniformTypes`). Applied in arcadia only (its whole scene is
// deck-drawn, so the pass covers everything; transit's ground is MapLibre tiles the overlay can't touch).
// Kept subtle so the Okabe-Ito line hues stay readable.
import type { ShaderPass } from "@luma.gl/shadertools";

const fs = /* glsl */ `\
uniform acesfxUniforms {
  float exposure;   // pre-tonemap exposure
  float strength;   // how far to blend toward the tonemapped result (0..1)
  float grain;      // film-grain amplitude
  float vignette;   // vignette darkening at the corners
} acesfx;

// Cheap hash → static fine grain (no time uniform, so no per-frame work).
float acesfx_hash(vec2 p) {
  p = fract(p * vec2(443.8975, 397.2973));
  p += dot(p, p.yx + 19.19);
  return fract((p.x + p.y) * p.x);
}

// Narkowicz ACES filmic curve.
vec3 acesfx_tonemap(vec3 x) {
  const float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
  return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

vec4 acesfx_filterColor(vec4 color, vec2 texSize, vec2 texCoord) {
  // The overlay is composited over the basemap with premultiplied alpha; leave (near-)transparent
  // pixels untouched so empty areas keep showing the map through, un-tinted.
  if (color.a < 0.004) return color;
  vec3 c = color.rgb;
  vec3 mapped = acesfx_tonemap(c * acesfx.exposure);
  c = mix(c, mapped, acesfx.strength);
  // subtle grain (signed, zero-mean)
  float n = acesfx_hash(texCoord * texSize) - 0.5;
  c += n * acesfx.grain;
  // gentle vignette
  vec2 v = texCoord - 0.5;
  c *= 1.0 - acesfx.vignette * dot(v, v);
  return vec4(clamp(c, 0.0, 1.0), color.a);
}
`;

export const acesfx: ShaderPass<{ exposure: number; strength: number; grain: number; vignette: number }> = {
  name: "acesfx",
  uniformTypes: {
    exposure: "f32",
    strength: "f32",
    grain: "f32",
    vignette: "f32",
  },
  // defaults (also the values App passes explicitly when constructing the PostProcessEffect)
  defaultUniforms: { exposure: 1.05, strength: 0.85, grain: 0.05, vignette: 0.22 },
  fs,
  passes: [{ filter: "acesfx_filterColor" }],
} as unknown as ShaderPass<{ exposure: number; strength: number; grain: number; vignette: number }>;

/** The props the PostProcessEffect is constructed with (subtle: tone-map + a whisper of grain). */
export const ACESFX_PROPS = { exposure: 1.06, strength: 0.85, grain: 0.045, vignette: 0.2 };
