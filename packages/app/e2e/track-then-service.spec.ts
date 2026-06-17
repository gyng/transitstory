import { test, expect } from "@playwright/test";

// TTD L6 "track + services": a drawn line with NO assigned stock is bare TRACK (trains 0 — rendered as
// grey infrastructure, kept out of the coloured `lines` layer + the service roster). Assigning stock
// PROMOTES it to a coloured SERVICE (trains > 0) that dispatches and carries riders. Camera-independent
// via __ot_test (routes through Game -> coords/geo.ts), asserting authoritative sim state + a gameplay fact.
test("track → service: bare track gains stock, then dispatches + carries riders", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // Lay TRACK: chain 3 stations into a line (drawLine commits a STOCKLESS line — bare track).
  const ids = await page.evaluate(() => {
    const t = window.__ot_test!;
    return [
      t.placeStationLngLat(103.845, 1.29),
      t.placeStationLngLat(103.86, 1.31),
      t.placeStationLngLat(103.84, 1.335),
    ] as unknown as number[];
  });
  await page.evaluate((sids) => window.__ot_test!.drawLine(sids as number[]), ids);

  // Bare TRACK: the line exists with its stops but carries NO trains yet.
  const bare = await page.evaluate(() => window.__ot!.bridge.stats().perLine);
  expect(bare).toHaveLength(1);
  expect(bare[0].stops).toBe(3);
  expect(bare[0].trains).toBe(0);

  // Assign stock → it becomes a SERVICE (trains > 0). This is the track→service promotion (#legion L6).
  await page.evaluate(() => window.__ot_test!.assignTrainset(0, 3));
  const serviced = await page.evaluate(() => window.__ot!.bridge.stats().perLine);
  expect(serviced[0].trains).toBeGreaterThan(0);

  // Run it: the service dispatches vehicles + carries riders (a gameplay fact, not "page loaded").
  await page.evaluate(() => {
    window.__ot_test!.setRunning(true);
    window.__ot_test!.setSpeed(100);
  });
  await page.waitForFunction(() => window.__ot!.bridge.vehiclePositions().length > 0, undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => (window.__ot_test!.stats() as { ridershipTotal: number }).ridershipTotal > 0,
    undefined,
    { timeout: 30_000 },
  );
  const ridership = await page.evaluate(() => (window.__ot_test!.stats() as { ridershipTotal: number }).ridershipTotal);
  expect(ridership).toBeGreaterThan(0);
});
