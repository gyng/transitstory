import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

// PLAN §0.8: build.target='esnext' supports top-level await natively, so we use
// vite-plugin-wasm ONLY (no vite-plugin-top-level-await — redundant failure surface).
// Vite dev already transforms with an esnext-class target, so TLA works in dev too.
export default defineConfig({
  plugins: [wasm()],
  build: { target: "esnext" },
});
