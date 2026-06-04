import { defineConfig } from "@playwright/test";

// e2e is camera-independent (AGENTS testing): fixed viewport; specs wait on window flags,
// never sleeps. Dev server locally, the production preview bundle on CI.
const PORT = 4173;
const CI = !!process.env.CI;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: CI,
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: `http://localhost:${PORT}`,
    viewport: { width: 1280, height: 800 },
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  webServer: {
    command: CI
      ? `pnpm preview --port ${PORT} --strictPort`
      : `pnpm dev --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
