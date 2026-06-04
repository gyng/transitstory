import { test, expect } from "@playwright/test";

// CP8 — the flagship end-to-end slice against the production preview bundle. Drives the full
// 5-step loop and asserts concrete behavioural facts: a vehicle moved AND ridership > 0 AND
// the coverage gauge changed. Camera-independent (fixed viewport + __ot_test placement hook
// through coords/geo.ts). Never a load-only green.
test("full vertical slice: place → draw → assign → run → ridership + coverage", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // 1–3) place stations, draw a line, assign a trainset (auto headway).
  await page.evaluate(() => {
    const t = window.__ot_test!;
    const ids = [
      t.placeStationLngLat(103.84, 1.265),
      t.placeStationLngLat(103.855, 1.29),
      t.placeStationLngLat(103.85, 1.32),
      t.placeStationLngLat(103.83, 1.355),
      t.placeStationLngLat(103.845, 1.39),
    ];
    t.drawLine(ids);
    t.assignTrainset(0, 4);
  });

  // 4) run the sim at high speed so ridership develops quickly (catchment capture runs once
  //    Running, so coverage is asserted AFTER this — initial build-mode coverage is 0).
  await page.evaluate(() => {
    window.__ot_test!.setRunning(true);
    window.__ot_test!.setSpeed(100);
  });

  const pos0 = await page.evaluate(() => Array.from(window.__ot!.bridge.vehiclePositions()));
  // a vehicle moves...
  await page.waitForFunction(
    (prev) => {
      const cur = Array.from(window.__ot!.bridge.vehiclePositions());
      return cur.some((v, i) => Math.abs(v - (prev as number[])[i]) > 1);
    },
    pos0,
    { timeout: 10_000 },
  );
  // ...and ridership develops.
  await page.waitForFunction(
    () => (window.__ot_test!.stats() as { ridershipTotal: number }).ridershipTotal > 0,
    undefined,
    { timeout: 15_000 },
  );

  // 5) assert the concrete facts.
  const stats = await page.evaluate(() => window.__ot_test!.stats() as { ridershipTotal: number; coverageScore: number });
  expect(stats.ridershipTotal).toBeGreaterThan(0);
  expect(stats.coverageScore).toBeGreaterThan(0);

  // DOM reflects ridership (handle BigInt-safe number rendering).
  await expect(page.locator('[data-testid="ridership"]')).not.toHaveText("0");

  await page.screenshot({ path: "../../docs/progress/cp8-slice-running.png" });
});
