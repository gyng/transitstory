import { expect, test } from "@playwright/test";

// Checkpoint corroboration (NOT a behavioural gate): the procedurally-baked fantasy continent
// (scripts/build_world.py, seed 7) renders as the map. Camera-independent: waits on the app/map
// ready flags, asserts the baked world constructed the arcadia ruleset, then captures the terrain.
test("fantasy baked world renders the terrain map", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY);
  const ruleset = await page.evaluate(() => (window as any).__ot_test.stats().ruleset);
  expect(ruleset).toBe("arcadia"); // the baked manifest selected the fantasy engine
  await page.waitForFunction(() => (window as any).__MAP_READY); // basemap + deck composited
  await page.waitForTimeout(1500); // let deck draw the 10k-hex terrain layer + tiles settle
  await page.screenshot({ path: "../../docs/progress/fantasy-baked-world.png" });
});
