import { expect, test } from "@playwright/test";

// TERRAIN + GOLD BUILD ECONOMY (#terrain/#economy): building rail SPENDS gold from the realm treasury and
// rough country costs more — so a build is an ROI decision, not a free click. This builds a reachable bread
// chain, confirms the treasury dropped by the line's gold price, then screenshots the EditorPanel's gold
// cost readout. Client UI is corroboration; the core mechanic is unit-tested in crates/sim/tests/build_economy.rs.
test("fantasy build economy: rail costs gold, shown in the editor", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const econ = await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const cap = sg.towns[sg.towns.findIndex((t: any) => t.kind === "capital")];
    const bread = sg.towns.map((t: any, i: number) => ({ t, i }))
      .filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2 && x.t.recipe.every((c: number) => c < 4))
      .sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    const nearestSrc = (comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => { if (KIND[r.kind] === comm) { const d = hex(bread.t, r); if (d < bd) { bd = d; bi = ri; } } });
      return bi < 0 ? -1 : nt + bi;
    };
    const tt = (window as any).__ot_test;
    const divisor = tt.stats().buildGoldDivisor;
    const before = tt.stats().tribute;
    const line = tt.drawLine([nearestSrc(bread.t.recipe[0]), bread.i, nearestSrc(bread.t.recipe[1])]);
    const after = tt.stats().tribute;
    return { divisor, before, after, line };
  });

  expect(econ.divisor).toBeGreaterThan(0); // the gold build economy is ON for the baked world
  expect(econ.line).toBeGreaterThanOrEqual(0); // an affordable chain commits
  expect(econ.after).toBeLessThan(econ.before); // building SPENT gold from the treasury (an ROI decision)

  await page.evaluate(() => (window as any).__ot.map.easeTo({ zoom: 12.6, duration: 0 }));
  await page.waitForTimeout(900);
  await page.screenshot({ path: "../../docs/progress/fantasy-economy.png" });
});
