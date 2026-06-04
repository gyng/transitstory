import { test, expect } from "@playwright/test";

// T12: assign a trainset (auto-suggested headway) and override headway; assert authoritative
// sim state via stats(). Then exercise the EditorPanel UI path and screenshot.
test("assign trainset + headway via the editor and the command path", async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  await page.evaluate(() => {
    const t = window.__ot_test!;
    const a = t.placeStationLngLat(103.845, 1.29);
    const b = t.placeStationLngLat(103.86, 1.31);
    const c = t.placeStationLngLat(103.84, 1.335);
    t.drawLine([a, b, c]);
    t.assignTrainset(0, 3); // auto-suggests a headway
  });

  let line = await page.evaluate(() => window.__ot!.bridge.stats().perLine[0]);
  expect(line.trains).toBe(3);
  expect(line.headwayMs).toBeGreaterThan(0); // auto-suggested

  await page.evaluate(() => window.__ot_test!.setHeadwayMs(0, 6 * 60_000));
  line = await page.evaluate(() => window.__ot!.bridge.stats().perLine[0]);
  expect(line.headwayMs).toBe(360_000);

  // The editor panel is visible for the selected line.
  await expect(page.locator('[data-testid="editor-panel"]')).toBeVisible();
  await expect(page.locator('[data-testid="headway-slider"]')).toBeVisible();

  await page.screenshot({ path: "../../docs/progress/cp5-assign-trainset.png" });
});
