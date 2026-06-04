import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

// Transport modes (#3): the chorded bottom bar selects a construction mode; lines drawn while
// a mode is active carry that mode through the Command path into the sim. Plus the demand
// map-layer (#4) and the settings mode toggles (#8).
test("chorded modes: build a ferry line + a rail line, toggle demand & settings", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // The chorded bar shows four big mode buttons + the build-controls popover above it.
  for (const id of [0, 1, 2, 3]) {
    await expect(page.locator(`[data-testid="mode-transport-${id}"]`)).toBeVisible();
  }
  await expect(page.locator('[data-testid="mode-controls"]')).toBeVisible();

  // Build a RAIL line (default mode 0) through three stations.
  const railIds = await page.evaluate(() => {
    const t = window.__ot_test!;
    return [
      t.placeStationLngLat(103.845, 1.29),
      t.placeStationLngLat(103.86, 1.31),
      t.placeStationLngLat(103.84, 1.335),
    ];
  });
  await page.evaluate((ids) => window.__ot_test!.drawLine(ids as number[]), railIds);

  // Switch to FERRY (mode 2) via the bar button, then draw a ferry route over the strait.
  await page.locator('[data-testid="mode-transport-2"]').click();
  const ferryIds = await page.evaluate(() => {
    const t = window.__ot_test!;
    return [
      t.placeStationLngLat(103.83, 1.24), // open water south of the island
      t.placeStationLngLat(103.88, 1.23),
    ];
  });
  await page.evaluate((ids) => window.__ot_test!.drawLine(ids as number[]), ferryIds);

  const lines = await page.evaluate(() => window.__ot!.bridge.linesView());
  expect(lines).toHaveLength(2);
  expect(lines[0].mode).toBe(0); // rail
  expect(lines[1].mode).toBe(2); // ferry
  // A ferry over water is NOT flagged as an illegal surface-water crossing (water is its road).
  expect(lines[1].crossesWaterSurface).toBe(false);

  // Demand map layer toggles on.
  await page.locator('[data-testid="layer-demand"]').click();
  expect(await page.evaluate(() => window.__ot!.game.showDemand)).toBe(true);

  // Settings: disabling a mode greys it out in the bar (can't be selected).
  await page.locator('[data-testid="open-settings"]').click();
  await expect(page.locator('[data-testid="settings-panel"]')).toBeVisible();
  await page.locator('[data-testid="setting-mode-3"]').click(); // turn Plane off
  expect(await page.evaluate(() => window.__ot!.game.enabledModes.has(3))).toBe(false);
  await page.evaluate(() => window.__ot_test!.setTransport(3)); // refused: mode disabled
  expect(await page.evaluate(() => window.__ot!.game.transport)).not.toBe(3);

  mkdirSync("../../docs/progress", { recursive: true });
  await page.screenshot({ path: "../../docs/progress/modes-chorded-bar.png" });
});

// A ferry route runs vehicles over open water (its placement gate allows water; the demand
// flows because terminals catch the same grid). Smoke: it dispatches and moves.
test("ferry line dispatches vehicles over water", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  const lineId = await page.evaluate(() => {
    const t = window.__ot_test!;
    const a = t.placeStationLngLat(103.83, 1.24);
    const b = t.placeStationLngLat(103.88, 1.23);
    t.setTransport(2); // ferry
    const id = t.drawLine([a, b]);
    t.assignTrainset(id, 2);
    t.setRunning(true);
    t.setSpeed(100);
    return id;
  });
  expect(lineId).toBeGreaterThanOrEqual(0);

  await page.waitForFunction(
    () => window.__ot!.bridge.vehiclePositions().length > 0,
    undefined,
    { timeout: 10_000 },
  );
  const mode = await page.evaluate((id) => window.__ot!.bridge.linesView()[id as number].mode, lineId);
  expect(mode).toBe(2);
});
