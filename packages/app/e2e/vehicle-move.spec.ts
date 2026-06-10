import { test, expect } from "@playwright/test";

// CP6 — the early visual win: vehicles dispatch and visibly move when Running. Camera-
// independent build via __ot_test; asserts a concrete fact (a vehicle position changed).
test("vehicles dispatch and move when running", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  await page.evaluate(() => {
    const t = window.__ot_test!;
    const a = t.placeStationLngLat(103.84, 1.27);
    const b = t.placeStationLngLat(103.86, 1.30);
    const c = t.placeStationLngLat(103.85, 1.34);
    const d = t.placeStationLngLat(103.83, 1.37);
    t.drawLine([a, b, c, d]);
    t.assignTrainset(0, 3);
    t.setRunning(true);
  });

  // Vehicles dispatch on the first sim TICK after Run — the rAF loop needs ~50 ms of wall time
  // to accumulate one step, so WAIT for it instead of racing it with a synchronous assert
  // (local machines won that race; CI runners lost it).
  await page.waitForFunction(() => window.__ot!.bridge.vehicleCount() > 0, undefined, {
    timeout: 10_000,
  });
  expect(await page.evaluate(() => window.__ot!.bridge.vehicleCount())).toBeGreaterThan(0);

  const p0 = await page.evaluate(() => Array.from(window.__ot!.bridge.vehiclePositions()));
  // Wait until a vehicle has advanced (>1 mm) — the rAF loop is ticking the sim.
  await page.waitForFunction(
    (prev) => {
      const cur = Array.from(window.__ot!.bridge.vehiclePositions());
      return cur.some((v, i) => Math.abs(v - (prev as number[])[i]) > 1);
    },
    p0,
    { timeout: 10_000 },
  );

  await page.screenshot({ path: "../../docs/progress/cp6-vehicles-running.png" });
});
