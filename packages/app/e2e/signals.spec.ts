import { test, expect } from "@playwright/test";

// TTD L5c — player-placed block signals. A signal is a CONTEXTUAL per-line gesture: with a line selected
// in BUILD mode, a click near one of its SINGLE-track spans drops a block signal; a click on an existing
// post removes it. Camera-independent via __ot_test (placeSignalLngLat routes through Game → coords/geo.ts,
// the production click path), asserting the authoritative placed-signal store (the `signal-<line>-<span>-
// <atMm>` id contract) — a gameplay fact, not "page loaded". Screenshot corroborates the post renders.
test("signals: place on a single-track span, then remove — via the production gesture path", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // Lay a 3-stop line (same on-screen coords as track-then-service), make it SINGLE track, and select it
  // (the signal gesture is contextual on the selected line).
  const ids = await page.evaluate(() => {
    const t = window.__ot_test!;
    return [
      t.placeStationLngLat(103.845, 1.29),
      t.placeStationLngLat(103.86, 1.31),
      t.placeStationLngLat(103.84, 1.335),
    ] as unknown as number[];
  });
  await page.evaluate((sids) => {
    const t = window.__ot_test!;
    t.drawLine(sids as number[]);
    t.setLineTrack(0, 1); // whole line → single track (the placement precondition)
    t.selectLine(0);
  }, ids);

  // PLACE a signal on the span between stops 0 and 1 (a point near the rail between them).
  const placed1 = await page.evaluate(() => window.__ot_test!.placeSignalLngLat(103.8525, 1.3));
  expect(placed1).toBe(true);
  let signals = await page.evaluate(() => window.__ot_test!.placedSignalIds());
  expect(signals).toHaveLength(1);
  expect(signals[0]).toMatch(/^signal-0-\d+-\d+$/); // signal-<line>-<span>-<atMm>

  // PLACE a second on the span between stops 1 and 2.
  const placed2 = await page.evaluate(() => window.__ot_test!.placeSignalLngLat(103.85, 1.3225));
  expect(placed2).toBe(true);
  signals = await page.evaluate(() => window.__ot_test!.placedSignalIds());
  expect(signals).toHaveLength(2);

  // REMOVE the first: clicking near an existing post removes it (remove has priority in the gesture).
  const removed = await page.evaluate(() => window.__ot_test!.placeSignalLngLat(103.8525, 1.3));
  expect(removed).toBe(true);
  signals = await page.evaluate(() => window.__ot_test!.placedSignalIds());
  expect(signals).toHaveLength(1); // only the second remains

  // Corroboration: the post renders on the network (screenshot for a human look — not a pixel gate).
  await page.screenshot({ path: "signal-placement.png" });
});
