import { expect, test } from "@playwright/test";

// S11 — the arcadia campaign's scored VICTORY ("Against the Dark"). The fork had a lose state (the realm
// falls) but an open-ended win; the objectives layer now hosts a real victory: conquer towns + hold
// standing before the decadence overruns the capital. This proves, on the real bundle, that the arcadia
// goal kinds (towns / standing) TRACK the live sim and the panel renders — the objective is wired, not
// dead chrome. (A full 3-town win is a long campaign; this asserts the loop CLOSES one notch — a town
// falls and the objective reflects it — which is the load-bearing wiring.) Deterministic via tickMs.
test("fantasy campaign: the victory objective tracks conquest", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy&scenario=arcadia-conquest");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  // The scored scenario is live: its panel renders with the arcadia goals (towns + standing).
  await expect(page.getByTestId("objectives")).toContainText("Against the Dark");
  await expect(page.getByTestId("objective-goal-towns")).toContainText("/3");
  await expect(page.getByTestId("objective-goal-standing")).toContainText("/20");
  // It starts ACTIVE (not pre-won, not failed).
  await expect(page.getByTestId("objective-status")).toContainText("in progress");

  // Build the canonical conquest loop on the baked world (mirrors fantasy-conquest): supply two BREAD
  // towns for tribute, then a capital-barracks line to the nearest town + a bounty to steer the legion.
  await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const nearestSrc = (t: any, comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => {
        if (KIND[r.kind] === comm) { const d = hex(t, r); if (d < bd) { bd = d; bi = ri; } }
      });
      return bi < 0 ? -1 : nt + bi;
    };
    const tt = (window as any).__ot_test;
    const capitalIdx = sg.towns.findIndex((t: any) => t.kind === "capital");
    const sinks = sg.towns.map((t: any, i: number) => ({ t, i })).filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2);
    const breadSinks = sinks.filter((x: any) => x.t.recipe.every((c: number) => c < 4));
    let line = 0;
    for (const { t, i } of breadSinks.slice(0, 2)) {
      tt.drawLine([nearestSrc(t, t.recipe[0]), i, nearestSrc(t, t.recipe[1])]);
      tt.assignTrainset(line, 4);
      tt.setHeadwayMs(line, 120000);
      line++;
    }
    const cap = sg.towns[capitalIdx];
    const target = sinks.slice().sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    tt.drawLine([capitalIdx, target.i]);
    tt.assignTrainset(line, 2);
    tt.setHeadwayMs(line, 120000);
    tt.postBounty(target.i, 3000);
  });

  // Run deterministically until a town falls (or the realm does).
  let st: any = null;
  for (let i = 0; i < 24; i++) {
    st = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(150000);
      const s = (window as any).__ot_test.stats();
      return { townsCaptured: s.townsCaptured, realmLost: s.realmLost };
    });
    if (st.townsCaptured >= 1 || st.realmLost) break;
  }
  expect(st.realmLost).toBe(false);
  expect(st.townsCaptured).toBeGreaterThanOrEqual(1); // the conquest loop closed one notch

  // The objective panel TRACKS it: the towns goal now reads "1/3" (the live sim drives the scored goal).
  await expect(page.getByTestId("objective-goal-towns-current")).toContainText(`${st.townsCaptured}/3`);

  await page.screenshot({ path: "../../docs/progress/fantasy-victory.png" });
});
