import { expect, test } from "@playwright/test";

// JUICE wave corroboration: with supply flowing on the baked world, the FX canvas shows the living-world
// juice — steam/dust trails off the moving carts and "+N⬢" gold floats where cargo lands. This drives a
// real running scenario (so the ~3 Hz stats throttle + rAF actually emit + draw the effects on wall-clock),
// then screenshots. The effects are purely client-side (no sim read), so this is corroboration, not a gate.
test("fantasy juice: steam trails + gold floats on a running supply line", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const nearestSrc = (t: any, comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => { if (KIND[r.kind] === comm) { const d = hex(t, r); if (d < bd) { bd = d; bi = ri; } } });
      return bi < 0 ? -1 : nt + bi;
    };
    const cap = sg.towns[sg.towns.findIndex((t: any) => t.kind === "capital")];
    const bread = sg.towns.map((t: any, i: number) => ({ t, i }))
      .filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2 && x.t.recipe.every((c: number) => c < 4))
      .sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    const tt = (window as any).__ot_test;
    tt.drawLine([nearestSrc(bread.t, bread.t.recipe[0]), bread.i, nearestSrc(bread.t, bread.t.recipe[1])]);
    tt.assignTrainset(0, 4);
    tt.setHeadwayMs(0, 60000);
  });

  // Warm up: dispatch carts + start tribute flowing. Then settle on the capital quadrant for the shot.
  for (let i = 0; i < 6; i++) {
    await page.evaluate(() => (window as any).__ot_test.tickMs(60000));
  }
  await page.evaluate(() => {
    const cap = (window as any).__ot.game.towns.find((t: any) => t.kind === "capital");
    if (cap) (window as any).__ot.map.easeTo({ center: [cap.lng, cap.lat], zoom: 12.6, duration: 0 });
  });

  const tribute = await page.evaluate(() => (window as any).__ot_test.stats().tribute);
  expect(tribute).toBeGreaterThan(0); // supply is flowing, so the gold floats have something to show

  // Enter REAL run mode and let the rAF loop + ~3 Hz juice throttle run on wall-clock for a couple seconds,
  // so the carts move continuously and lay a steam trail — far more reliable to catch in a still than
  // discrete tickMs steps (the puffs/floats are wall-clock-timed FX, not sim-tick state).
  await page.evaluate(() => (window as any).__ot_test.setSpeed(4));
  await page.evaluate(() => (window as any).__ot_test.setRunning(true));
  await page.waitForTimeout(2600);
  await page.screenshot({ path: "../../docs/progress/fantasy-juice.png" });
});
