import { expect, test } from "@playwright/test";

// Winnability gate for the BAKED world (S7e disjoint chains + seeded decadence, baked-world balance pass):
// connect two towns' full chains so tribute flows, let the capital-barracks field legions, run a long
// deterministic stretch, and assert the full loop CLOSES — conquest completes AND the realm holds (the
// rot doesn't reach the capital). Deterministic via tickMs (no rAF). Logs the trajectory so balance stays
// observable. The native `tests/balance.rs::fantasy_baked_continent_is_winnable` is the authoritative
// pacing gate (synthetic, exhaustive); this corroborates it end-to-end on the real bundle + geometry.
test("fantasy baked world is winnable: supply → legions → conquest, realm holds", async ({ page }) => {
  test.setTimeout(60_000); // heavier probe (3 lines + tick-steps); generous under 15-worker parallel load
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const start = await page.evaluate(async () => {
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
    // Supply via the 2-stage BREAD towns (raw recipe, commodity < 4) — directly railable from raw sources.
    // ARMS towns are now 3-stage (need INGOT from a forge, commodity ≥ 4); their chain is exercised by
    // fantasy-multistage.spec.ts. Tribute from BREAD funds the legions all the same.
    const breadSinks = sinks.filter((x: any) => x.t.recipe.every((c: number) => c < 4));
    let line = 0;
    // Supply: connect two BREAD towns' full chains (src1→town→src2) so the two-chain Liebig tribute flows.
    for (const { t, i } of breadSinks.slice(0, 2)) {
      tt.drawLine([nearestSrc(t, t.recipe[0]), i, nearestSrc(t, t.recipe[1])]);
      tt.assignTrainset(line, 4);
      tt.setHeadwayMs(line, 120000);
      line++;
    }
    // Conquest: a line from the CAPITAL-barracks to the town NEAREST it (so a legion launches from the
    // barracks-on-a-line and marches to the bountied target), + a bounty to steer the legion there.
    const cap = sg.towns[capitalIdx];
    const target = sinks.slice().sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    tt.drawLine([capitalIdx, target.i]);
    tt.assignTrainset(line, 2);
    tt.setHeadwayMs(line, 120000);
    tt.postBounty(target.i, 3000);
    return (window as any).__ot_test.stats();
  });
  expect(start.ruleset).toBe("arcadia");

  // Run a long deterministic stretch (each tickMs step is instant — no rAF), sampling the trajectory,
  // until conquest lands or the realm falls. Generous step budget: at the baked army speed (200 m/s) the
  // 60 km nearest town is a ~5-min march, plus the supply ramp over the long real lines — comfortably
  // inside the ~40-min decadence runway, but the real geometry is slower than the synthetic harness.
  const traj: any[] = [];
  for (let i = 0; i < 24; i++) {
    const s = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(150000); // 150 sim-sec per step
      const st = (window as any).__ot_test.stats();
      return { tribute: st.tribute, armyCount: st.armyCount, townsCaptured: st.townsCaptured, decadencePct: Math.round(st.decadencePct), realmLost: st.realmLost };
    });
    traj.push(s);
    if (s.townsCaptured >= 1 || s.realmLost) break;
  }
  // eslint-disable-next-line no-console
  console.log("BAKED-WORLD BALANCE trajectory:", JSON.stringify(traj));
  const last = traj[traj.length - 1];

  // The full loop CLOSES on the real baked world (the balance pass): the two-chain Liebig supply flows,
  // tribute funds legions from the capital-barracks, a legion marches the continent, and conquest lands
  // BEFORE the rot overruns the realm. (Pre-pass, conquest never reached: the demo army speed needed
  // ~21 sim-min for the 60 km town — past this window — and the decadence integer-truncation froze the
  // lose meter so "holds" was vacuous. Now army speed + the decadence accumulator are baked-scaled.)
  const everArmy = traj.some((s) => s.armyCount > 0);
  expect(last.tribute).toBeGreaterThan(0); // the two-chain Liebig supply flows
  expect(everArmy).toBe(true); // tribute funded legions that launched from the barracks
  expect(last.townsCaptured).toBeGreaterThanOrEqual(1); // conquest COMPLETES — a town falls
  expect(last.realmLost).toBe(false); // and the realm HOLDS (conquest outpaced the corruption)
});
