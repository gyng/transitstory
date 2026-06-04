import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

interface S {
  buildDifficulty: number;
  perLine: { disruption: number; crossesWater: boolean }[];
}

// G1/G2 — surface rail through the built environment costs "build impact"; grade-separating
// (Elevated/Tunnel) reduces it. Driven on the real Singapore MRT (lots of surface-in-city).
test("surface track has build impact; tunnelling a line reduces it", async ({ page }) => {
  await page.goto("/?city=singapore&network=1");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  const before = await page.evaluate(() => {
    const s = window.__ot_test!.stats() as S;
    return { difficulty: s.buildDifficulty, line0: s.perLine[0].disruption };
  });
  expect(before.difficulty).toBeGreaterThan(0); // a real surface network through built-up land
  expect(before.line0).toBeGreaterThan(0);

  // Tunnel line 0 → its disruption drops (grade separation removes the surface penalty).
  await page.evaluate(() => window.__ot_test!.setLineMode(0, 2));
  const after0 = await page.evaluate(() => (window.__ot_test!.stats() as S).perLine[0].disruption);
  expect(after0).toBeLessThan(before.line0);

  await page.screenshot({ path: "../../docs/progress/g2-buildability.png" });
  mkdirSync("../../docs/progress", { recursive: true });
});

// Live hazard feedback while drawing a surface line through the dense CBD (sandbox).
test("drawing surface track through built-up land accrues disruption", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });
  await page.evaluate(() => {
    const t = window.__ot_test!;
    const a = t.placeStationLngLat(103.851, 1.284); // Raffles Place (CBD, built)
    const b = t.placeStationLngLat(103.846, 1.299); // Dhoby Ghaut
    const c = t.placeStationLngLat(103.832, 1.304); // Orchard
    t.drawLine([a, b, c]);
  });
  const disr = await page.evaluate(() => (window.__ot_test!.stats() as S).perLine[0].disruption);
  expect(disr).toBeGreaterThan(0);
});
