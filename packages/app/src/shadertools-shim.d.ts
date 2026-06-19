// #7 Ambient shim for the TRANSITIVE @luma.gl/shadertools (not a direct app dependency, so moduleResolution
// 'bundler' can't resolve its types — the last of the 4 round-3 tsc errors). This file is a PURE ambient
// declaration (no top-level import/export), so `declare module` truly DECLARES the module rather than augmenting
// an already-resolved one. postfx.ts imports only the ShaderPass TYPE and casts acesfx to it via `as unknown as`,
// so this minimal generic declaration satisfies tsc without touching pins or lockfiles; the real types ship with
// luma 9.3.3 at runtime.
declare module "@luma.gl/shadertools" {
  export interface ShaderPass<UniformsT = Record<string, unknown>> {
    name: string;
    fs?: string;
    uniformTypes?: Record<string, string>;
    defaultUniforms?: UniformsT;
    passes?: { filter?: string | boolean; sampler?: boolean | string }[];
  }
}
