import { expect, test } from "@playwright/test";

// MAPGEN pass corroboration: the baked flow-accumulation RIVERS (build_world.py drainage trees) render as
// cold water threading the ash continent. Asserts the additive `rivers` manifest field reached the frontend
// (game.rivers populated), then zooms in for a legible screenshot. Render-only (rivers add no rail cost yet).
test("fantasy baked rivers render on the continent", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const nRivers = await page.evaluate(() => (window as any).__ot.game.rivers.length as number);
  expect(nRivers).toBeGreaterThan(10); // the drainage network reached the render layer

  // Zoom into the capital quadrant so the river threads + fords are legible in the shot.
  await page.evaluate(() => (window as any).__ot.map.easeTo({ zoom: 11.4, duration: 0 }));
  await page.waitForTimeout(1500); // let deck redraw the terrain + rivers
  await page.screenshot({ path: "../../docs/progress/fantasy-rivers.png" });
});
