import { expect, test } from "@playwright/test";

// The baked procedural continent is PLAYABLE — not just rendered. The supply graph placed source + sink
// stations (roles from the baked demand grid); drawing a line between a resource (source) and a town
// (sink), assigning carts, and running delivers supply → TRIBUTE. Asserts gameplay facts (never load-only),
// camera-independent (waits on flags + a tribute>0 condition, no sleeps as the gate).
test("fantasy baked world is playable: supply flows to tribute", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY);

  // the baked supply graph placed nodes with source/sink roles assigned from the demand grid
  const roles = await page.evaluate(() => {
    const s = (window as any).__ot_test.stats();
    const per = s.perStation || [];
    return {
      ruleset: s.ruleset,
      stationCount: s.stationCount,
      sources: per.filter((p: any) => p.demandOrigin > p.demandDest).length,
      sinks: per.filter((p: any) => p.demandDest > p.demandOrigin).length,
      decadence: s.decadence,
    };
  });
  expect(roles.ruleset).toBe("arcadia");
  expect(roles.stationCount).toBeGreaterThan(20); // resources + towns placed
  expect(roles.sources).toBeGreaterThan(0); // resources read as sources
  expect(roles.sinks).toBeGreaterThan(0); // towns read as sinks
  expect(roles.decadence).toBeGreaterThan(0); // S4 baked starting corruption seeded world.decadence at load

  // connect the nearest (non-capital town ↔ resource) pair, assign carts, run
  await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    // S7e-2: a baked town demands BOTH inputs of its chain — Liebig. Pick a 2-stage BREAD town (recipe =
    // grain+fuel, both commodities < 4 → railable directly from raw sources), find the nearest source of
    // EACH required commodity, and run a line src1→town→src2 so both chains reach it (one input alone yields
    // no tribute). The 3-stage ARMS towns (recipe includes INGOT=4, which has no source — it's forged) are
    // proven separately by fantasy-multistage.spec.ts. Station ids: towns 0..nt-1, then resources.
    const capitalIdx = sg.towns.findIndex((t: any) => t.kind === "capital");
    const ti = sg.towns.findIndex((t: any) => t.kind !== "capital" && t.recipe?.length === 2 && t.recipe.every((c: number) => c < 4));
    const town = sg.towns[ti];
    const nearestSrc = (comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => {
        if (KIND[r.kind] === comm) { const d = hex(town, r); if (d < bd) { bd = d; bi = ri; } }
      });
      return bi < 0 ? -1 : nt + bi;
    };
    const s1 = nearestSrc(town.recipe[0]);
    const s2 = nearestSrc(town.recipe[1]);
    const tt = (window as any).__ot_test;
    // CONNECTED-RAIL gate (#infrastructure): root the chain at the capital (the starter bread town + its
    // grain/fuel sources sit in the capital's carved bootstrap valley). capital → src1 → town → src2: both
    // chain inputs route to the town (the sink between them) → Liebig, and the whole line ties to the seat.
    tt.drawLine([capitalIdx, s1, ti, s2]);
    tt.assignTrainset(0, 4);
    tt.setHeadwayMs(0, 120000);
    // advance the sim SYNCHRONOUSLY (deterministic, no rAF) so the result is load-independent.
    tt.tickMs(300000);
  });

  // gameplay facts on the BAKED procedural map: carts run AND the town's BOTH chain inputs are delivered
  // → consumed by Liebig → tribute. (Feeding only one input would yield 0 — the disjoint-chain pressure.)
  const s = await page.evaluate(() => (window as any).__ot_test.stats());
  expect(s.vehicleCount).toBeGreaterThan(0); // playable: carts run on the baked continent
  expect(s.tribute).toBeGreaterThan(0); // both chain inputs delivered → Liebig bread/arms → tribute

  await page.screenshot({ path: "../../docs/progress/fantasy-baked-playable.png" });
});
