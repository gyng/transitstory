import { expect, test } from "@playwright/test";

// S7e-2 — the 3-STAGE Forge-Line chain on the BAKED world. The baked continent now carries FORGE nodes
// (processors) and ARMS towns whose recipe is [INGOT=4, AETHER=2]. INGOT has NO source — it must be
// FORGED from ore at a forge node, then shipped onward. This proves, end-to-end on the real bundle +
// geometry, that the full chain closes: ore → forge → INGOT → arms-town, plus aether → arms-town, both
// consumed by Liebig → tribute. Tribute can ONLY appear if (a) the forge converted ore → ingot and (b)
// commodity-aware routing carried the ore to the forge (not past it to the town) — so a positive tribute
// is direct proof the processor + commodity routing work on the baked data. Deterministic via tickMs.
test("fantasy baked world: 3-stage Forge-Line (ore → forge → arms town) yields tribute", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const start = await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    // nearest RAW resource of a commodity to a node → station id (towns 0..nt-1, then resources at nt+ri)
    const nearestSrc = (node: any, comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => {
        if (KIND[r.kind] === comm) { const d = hex(node, r); if (d < bd) { bd = d; bi = ri; } }
      });
      return bi < 0 ? -1 : nt + bi;
    };
    // nearest FORGE (processor node) to a node → station id
    const nearestForge = (node: any) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => {
        if (r.kind === "forge") { const d = hex(node, r); if (d < bd) { bd = d; bi = ri; } }
      });
      return bi < 0 ? -1 : nt + bi;
    };
    // an ARMS town: recipe includes INGOT (4) — a 3-stage chain (INGOT is forged from ore, not mined).
    const armsTi = sg.towns.findIndex((t: any) => Array.isArray(t.recipe) && t.recipe.includes(4));
    const arms = sg.towns[armsTi];
    const forgeSid = nearestForge(arms);          // the forge nearest the arms town
    const forge = sg.resources[forgeSid - nt];
    const oreSid = nearestSrc(forge, 0);          // ore for the forge to convert → INGOT
    const aetherSid = nearestSrc(arms, 2);        // aether (the arms town's other Liebig input)
    const tt = (window as any).__ot_test;
    // The player wires the chain across three lines: ore→forge, forge→arms-town, aether→arms-town.
    tt.drawLine([oreSid, forgeSid]);   // stage 1: mine ore → forge (forge consumes ore, makes INGOT)
    tt.drawLine([forgeSid, armsTi]);   // stage 2: forge INGOT → arms town
    tt.drawLine([aetherSid, armsTi]);  // stage 3: aether → arms town (the second Liebig input)
    for (let l = 0; l < 3; l++) { tt.assignTrainset(l, 4); tt.setHeadwayMs(l, 120000); }
    return { ruleset: tt.stats().ruleset, armsRecipe: arms.recipe, forgeKind: forge.kind, oreSid, forgeSid, aetherSid };
  });
  expect(start.ruleset).toBe("arcadia");
  expect(start.armsRecipe).toContain(4);      // a genuine 3-stage (INGOT-consuming) town
  expect(start.forgeKind).toBe("forge");      // the middle node is a processor
  expect(start.oreSid).toBeGreaterThan(0);    // every leg resolved to a real station
  expect(start.aetherSid).toBeGreaterThan(0);

  // Run a long deterministic stretch (each tickMs step is instant — no rAF). The forge must first
  // accumulate ore, convert it to INGOT, and ship it on before the arms town can consume INGOT+AETHER —
  // a longer ramp than a 2-stage chain, so budget generously (still well inside the decadence runway).
  let tribute = 0;
  const traj: number[] = [];
  for (let i = 0; i < 30; i++) {
    tribute = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(150000); // 150 sim-sec per step
      return (window as any).__ot_test.stats().tribute;
    });
    traj.push(tribute);
    if (tribute > 0) break;
  }
  // eslint-disable-next-line no-console
  console.log("3-STAGE FORGE-LINE tribute trajectory:", JSON.stringify(traj));
  expect(tribute).toBeGreaterThan(0); // the full 3-stage chain closed on the real baked world

  // S11 ECONOMY SPLIT: the arms town consumes INGOT + AETHER → it mints MANPOWER (from ingot) and MANA
  // (from aether) ALONGSIDE gold. So an ARMS chain (unlike a BREAD chain, which is gold-only) feeds all
  // three channels — proven here on the real bundle.
  const econ = await page.evaluate(() => {
    const s = (window as any).__ot_test.stats();
    return { gold: s.tribute, mana: s.mana, manpower: s.manpower };
  });
  expect(econ.manpower).toBeGreaterThan(0); // INGOT delivered → manpower
  expect(econ.mana).toBeGreaterThan(0); // AETHER delivered → mana
  expect(econ.gold).toBeGreaterThan(0); // …and gold all the same

  await page.screenshot({ path: "../../docs/progress/fantasy-economy-split.png" });
});
