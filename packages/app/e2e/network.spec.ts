import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

// F4 — existing real network: boot Singapore with the MRT pre-seeded, run it, and confirm
// multi-line ridership develops (transfers across interchanges).
test("Singapore boots with the real MRT and carries riders", async ({ page }) => {
  await page.goto("/?city=singapore&network=1");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // The real MRT is pre-seeded: several lines + many interchange-sharing stations.
  const net = await page.evaluate(() => ({
    lines: window.__ot!.bridge.linesView().length,
    stations: window.__ot!.bridge.stationsView().length,
  }));
  expect(net.lines).toBeGreaterThanOrEqual(3);
  expect(net.stations).toBeGreaterThanOrEqual(25);

  // Run fast; ridership develops across the multi-line network.
  await page.evaluate(() => {
    window.__ot_test!.setRunning(true);
    window.__ot_test!.setSpeed(100);
  });
  await page.waitForFunction(
    () => (window.__ot_test!.stats() as { ridershipTotal: number }).ridershipTotal > 0,
    undefined,
    { timeout: 15_000 },
  );

  mkdirSync("../../docs/progress", { recursive: true });
  await page.screenshot({ path: "../../docs/progress/f4-singapore-mrt.png" });
});

// Perf/scale: Tokyo's full OSM network is ~32 lines / ~440 stations. It must still boot and
// carry riders (route cache keeps BFS-per-spawn cheap).
test("Tokyo (full OSM network, ~440 stations) boots and runs", async ({ page }) => {
  await page.goto("/?city=tokyo&network=1");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 45_000 });
  const net = await page.evaluate(() => ({
    lines: window.__ot!.bridge.linesView().length,
    stations: window.__ot!.bridge.stationsView().length,
  }));
  expect(net.lines).toBeGreaterThanOrEqual(10);
  expect(net.stations).toBeGreaterThanOrEqual(200);

  await page.evaluate(() => {
    window.__ot_test!.setRunning(true);
    window.__ot_test!.setSpeed(100);
  });
  await page.waitForFunction(
    () => (window.__ot_test!.stats() as { ridershipTotal: number }).ridershipTotal > 0,
    undefined,
    { timeout: 25_000 },
  );
  await page.screenshot({ path: "../../docs/progress/f6-tokyo-osm.png" });
});
