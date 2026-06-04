import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

test("Singapore basemap renders, attributes OSM, and constructs the sim", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__APP_READY === true, undefined, { timeout: 30_000 });
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // Map canvas present and ODbL attribution visible (release gate).
  await expect(page.locator(".maplibregl-canvas")).toBeVisible();
  await expect(page.locator(".maplibregl-ctrl-attrib")).toContainText("OpenStreetMap");

  // The sim was constructed from the committed city (demand grid loaded).
  const cells = await page.evaluate(() => window.__ot?.city.demandCellCount ?? 0);
  expect(cells).toBeGreaterThan(200);

  mkdirSync("../../docs/progress", { recursive: true });
  await page.screenshot({ path: "../../docs/progress/cp3-singapore-basemap.png" });
});
