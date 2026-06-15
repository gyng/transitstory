import { expect, test } from "@playwright/test";

// 3D DIORAMA (#3d-trees): the fantasy camera tilts to a TTD-style view and the forest hexes grow lowpoly
// pines (a SimpleMeshLayer instanced across the forest). Render-only (zero sim state). Asserts the trees
// populated + the camera is pitched, then screenshots the diorama.
test("fantasy 3D diorama: lowpoly pines stand on a tilted continent", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const r = await page.evaluate(() => ({
    trees: (window as any).__ot.game.trees.length as number,
    pitch: (window as any).__ot.map.getPitch() as number,
  }));
  expect(r.trees).toBeGreaterThan(500); // the forest grew a stand of pines
  expect(r.pitch).toBeGreaterThan(20); // the camera is tilted into the diorama

  // Settle on the capital quadrant at a detail zoom so the pines + tilt read in the shot.
  await page.evaluate(() => {
    const cap = (window as any).__ot.game.towns.find((t: any) => t.kind === "capital");
    if (cap) (window as any).__ot.map.easeTo({ center: [cap.lng, cap.lat], zoom: 13.2, pitch: 48, duration: 0 });
  });
  await page.waitForTimeout(1200);
  await page.screenshot({ path: "../../docs/progress/fantasy-trees.png" });
});
