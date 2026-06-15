import { expect, test } from "@playwright/test";

// DEPOT REWORK Stage 1 — the RAIL TRAIN-MODEL catalog. A line buys a model (Standard / Heavy / Express),
// a real capacity ⇄ speed ⇄ cost tradeoff. Default (spec 0) is byte-identical to the shipped metro (proven
// in crates/sim/tests/train_models.rs + the goldens); here we prove the picker works on the live bundle:
// switching to Heavy raises the line's gold build cost (more capacity, pricier stock). Screenshots the picker.
test("fantasy train models: picking Heavy raises the line's cost", async ({ page }) => {
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
    const g = (window as any).__ot.game;
    const line = tt.drawLine([nearestSrc(bread.t.recipe[0]), bread.i, nearestSrc(bread.t.recipe[1])]);
    tt.assignTrainset(line, 3); // 3 trains, default (Standard) model
    const before = tt.stats().perLine.find((l: any) => l.lineId === line);
    g.setAircraft(line, 1); // switch to Heavy (spec 1)
    const after = tt.stats().perLine.find((l: any) => l.lineId === line);
    return { line, specBefore: before.trainsetSpec, specAfter: after.trainsetSpec, costBefore: before.capitalCost, costAfter: after.capitalCost };
  });

  expect(econ.line).toBeGreaterThanOrEqual(0);
  expect(econ.specBefore).toBe(0); // started on the default model
  expect(econ.specAfter).toBe(1); // switched to Heavy
  expect(econ.costAfter).toBeGreaterThan(econ.costBefore); // Heavy stock costs more — a real tradeoff

  // The line is selected → the Editor shows the model picker. Zoom in for a legible shot.
  await page.evaluate(() => (window as any).__ot.map.easeTo({ zoom: 12.8, duration: 0 }));
  await page.waitForTimeout(700);
  await page.screenshot({ path: "../../docs/progress/fantasy-models.png" });
});
