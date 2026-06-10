import { test, expect } from "@playwright/test";

// CP8 — the flagship end-to-end slice against the production preview bundle. Drives the full
// 5-step loop and asserts concrete behavioural facts: a vehicle moved AND ridership > 0 AND
// the coverage gauge changed. Camera-independent (fixed viewport + __ot_test placement hook
// through coords/geo.ts). Never a load-only green.
test("full vertical slice: place → draw → assign → run → ridership + coverage", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // 1–3) place stations, draw a line, assign a trainset (auto headway). The corridor must be a
  // route a player could commit through the UI: the core PARKS a surface line whose track crosses
  // water (the old coordinates clipped Marina Bay and would now never run), and the coverage
  // gauge is denominated against the whole city's ORIGIN demand — so the line links real home
  // clusters (Tiong Bahru / Holland / the north) to job cores (CBD / Orchard), not just offices.
  await page.evaluate(() => {
    const t = window.__ot_test!;
    const ids = [
      t.placeStationLngLat(103.84, 1.281), // CBD — jobs
      t.placeStationLngLat(103.826, 1.291), // Tiong Bahru — homes
      t.placeStationLngLat(103.832, 1.304), // Orchard — jobs
      t.placeStationLngLat(103.786, 1.321), // Holland — homes
      t.placeStationLngLat(103.79, 1.345), // north residential
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

  // Vehicles dispatch on the first tick after Run — wait for them to EXIST before sampling
  // positions. (Sampling an empty array made the movement predicate compare against undefined —
  // NaN — and never fire; under parallel-suite CPU contention the race went that way.)
  await page.waitForFunction(() => window.__ot!.bridge.vehiclePositions().length > 0, undefined, {
    timeout: 10_000,
  });
  const pos0 = await page.evaluate(() => Array.from(window.__ot!.bridge.vehiclePositions()));
  // a vehicle moves...
  await page.waitForFunction(
    (prev) => {
      const cur = Array.from(window.__ot!.bridge.vehiclePositions());
      const p = prev as number[];
      return cur.length > 0 && (cur.length !== p.length || cur.some((v, i) => Math.abs(v - p[i]) > 1));
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
