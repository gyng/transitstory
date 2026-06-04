import { defineConfig } from "vitest/config";
import wasm from "vite-plugin-wasm";

// vite-plugin-wasm here so the wasm-in-node smoke (T9) can import the wasm-sim package.
export default defineConfig({
  plugins: [wasm()],
  test: {
    environment: "node",
    include: ["test/**/*.test.ts"],
  },
});
