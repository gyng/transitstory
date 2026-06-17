import { defineConfig } from "@playwright/test";

// e2e is camera-independent (AGENTS testing): fixed viewport; specs wait on window flags,
// never sleeps. Dev server locally, the production preview bundle on CI.
const PORT = 4173;
const CI = !!process.env.CI;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: CI,
  // The fantasy "*-shot" specs render a WebGL/3D diorama on (headless) SOFTWARE GL, which is slow — when
  // ~16 of them queue back-to-back the browser falls behind and individually-passing specs trip the 30s
  // default. Give every spec headroom (60s) and ONE retry (a retried spec gets a fresh context + clears the
  // accumulated GPU load), so the full serial suite is reliably green; a REAL regression still fails twice.
  timeout: 60_000,
  retries: 1,
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
