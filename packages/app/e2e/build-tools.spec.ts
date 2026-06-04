import { test, expect } from "@playwright/test";

// Build tools (T10/T11) driven camera-independently via __ot_test (routes through Game ->
// coords/geo.ts). Asserts authoritative sim state, not just rendering.
test("place stations and draw a line through them", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // Place 3 stations across central Singapore.
  const ids = await page.evaluate(() => {
    const t = window.__ot_test!;
    return [
      t.placeStationLngLat(103.845, 1.29),
      t.placeStationLngLat(103.86, 1.31),
      t.placeStationLngLat(103.84, 1.335),
    ] as unknown as number[];
  });
  expect(await page.evaluate(() => window.__ot!.bridge.stationsView().length)).toBe(3);

  // Draw a line through them and verify the committed line geometry.
  await page.evaluate((sids) => window.__ot_test!.drawLine(sids as number[]), ids);
  const lines = await page.evaluate(() => window.__ot!.bridge.linesView());
  expect(lines).toHaveLength(1);
  expect(lines[0].stops).toEqual([0, 1, 2]);
  expect(lines[0].polylineMm.length).toBeGreaterThan(3); // dense smoothed curve (F1)

  await page.screenshot({ path: "../../docs/progress/cp5-stations-and-line.png" });
});
