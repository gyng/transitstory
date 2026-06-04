import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

test("deck.gl overlay renders over the basemap", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // Overlaid mode adds deck's own canvas on top of MapLibre's -> at least two canvases.
  expect(await page.locator("canvas").count()).toBeGreaterThanOrEqual(2);

  // The overlay handle is live.
  expect(await page.evaluate(() => !!window.__ot?.overlay)).toBe(true);

  await page.screenshot({ path: "../../docs/progress/cp4-deck-overlay.png" });
  mkdirSync("../../docs/progress", { recursive: true });
});
