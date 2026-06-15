import { test, expect } from "@playwright/test";

// FLEET panel (#rolling-stock): view/build/edit trainsets directly + live status. Builds a line with trains,
// opens the Fleet panel, asserts the row + live load reflect it, and edits the fleet size inline. UI over the
// stats snapshot (no sim change), so this is the behavioural contract for the panel.
test("fleet panel: view + edit trainsets directly", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  // Build a bread line with trains so the fleet has rolling stock to show.
  const line = await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const cap = sg.towns[sg.towns.findIndex((t: any) => t.kind === "capital")];
    const bread = sg.towns.map((t: any, i: number) => ({ t, i })).filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2 && x.t.recipe.every((c: number) => c < 4)).sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    const nearestSrc = (comm: number) => { let bi = -1, bd = 1e9; sg.resources.forEach((r: any, ri: number) => { if (KIND[r.kind] === comm) { const d = hex(bread.t, r); if (d < bd) { bd = d; bi = ri; } } }); return bi < 0 ? -1 : nt + bi; };
    const tt = (window as any).__ot_test;
    const ln = tt.drawLine([nearestSrc(bread.t.recipe[0]), bread.i, nearestSrc(bread.t.recipe[1])]);
    tt.assignTrainset(ln, 3);
    return ln;
  });
  expect(line).toBeGreaterThanOrEqual(0);

  await page.getByTestId("fleet-toggle").click();
  await expect(page.getByTestId("fleet-panel")).toBeVisible();
  await expect(page.getByTestId(`fleet-row-${line}`)).toBeVisible();
  await expect(page.getByTestId(`fleet-count-${line}`)).toContainText("3");

  // Edit the fleet size directly from the panel (+1 → 4 trains).
  await page.getByTestId(`fleet-inc-${line}`).click();
  await expect(page.getByTestId(`fleet-count-${line}`)).toContainText("4");
  const trains = await page.evaluate((ln: number) => (window as any).__ot_test.stats().perLine.find((l: any) => l.lineId === ln)?.trains, line);
  expect(trains).toBe(4); // the inline edit committed via AssignTrainset

  await page.screenshot({ path: "../../docs/progress/fleet.png" });
});
