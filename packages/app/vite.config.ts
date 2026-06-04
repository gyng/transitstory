import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import react from "@vitejs/plugin-react";

// PLAN §0.8: build.target='esnext' supports top-level await natively, so we use
// vite-plugin-wasm ONLY (no vite-plugin-top-level-await — redundant failure surface).
// Vite dev already transforms with an esnext-class target, so TLA works in dev too.
// React drives the UI chrome (the chorded bar, panels, menu); the map/deck.gl overlay and
// the rAF render loop stay imperative and OUTSIDE React (AGENTS render-hot-path rule).
export default defineConfig({
  plugins: [react(), wasm()],
  build: { target: "esnext" },
});
