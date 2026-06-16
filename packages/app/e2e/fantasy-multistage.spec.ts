import { expect, test } from "@playwright/test";

// S7e-2 — the 3-STAGE Forge-Line chain on the BAKED world, NOW behind the #infrastructure connected-rail
// gate. The baked continent carries FORGE nodes (processors) and ARMS towns whose recipe is [INGOT=4,
// AETHER=2]. INGOT has NO source — it must be FORGED from ore at a forge node. The ore, its forge, and the
// arms town can be wired into ONE capital-rooted network, but AETHER is baked far + scarce (the arcane
// resource) — an ISLAND off that network until conquest plants a root nearer it. So the full chain can
// ONLY close AFTER conquest captures the arms town (minting a new rail root there): bootstrap a
// capital-rooted BREAD chain → fund a legion → capture the arms town → extend rail FROM the captured root
// to the aether → ore→forge→INGOT + aether → Liebig → tribute. This proves, end-to-end on the real bundle,
// BOTH the forge/commodity-routing mechanic AND that the conquest progression (conquer to unlock the deeper
// chain) works under the connected-rail rule. Deterministic via tickMs (no rAF).
test("fantasy baked world: conquest unlocks the 3-stage Forge-Line (ore → forge → arms town) for tribute", async ({ page }) => {
  test.setTimeout(90_000); // conquest ramp + the 3-stage forge ramp, both deterministic tick-steps
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const start = await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const nearestSrc = (node: any, comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => { if (KIND[r.kind] === comm) { const d = hex(node, r); if (d < bd) { bd = d; bi = ri; } } });
      return bi < 0 ? -1 : nt + bi;
    };
    const nearestForge = (node: any) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => { if (r.kind === "forge") { const d = hex(node, r); if (d < bd) { bd = d; bi = ri; } } });
      return bi < 0 ? -1 : nt + bi;
    };
    const capitalIdx = sg.towns.findIndex((t: any) => t.kind === "capital");
    const cap = sg.towns[capitalIdx];
    // The ARMS town NEAREST the capital (so it's within starting reach to lay the conquest line + siege it).
    const armsTi = sg.towns.map((t: any, i: number) => ({ t, i }))
      .filter((x: any) => Array.isArray(x.t.recipe) && x.t.recipe.includes(4))
      .sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0].i;
    const arms = sg.towns[armsTi];
    const forgeSid = nearestForge(arms);
    const forge = sg.resources[forgeSid - nt];
    const oreSid = nearestSrc(forge, 0);
    const aetherSid = nearestSrc(arms, 2);
    // A reachable BREAD town (raw recipe, all commodities < 4) to bootstrap tribute → fund the legion.
    const bread = sg.towns.map((t: any, i: number) => ({ t, i }))
      .filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2 && x.t.recipe.every((c: number) => c < 4))
      .sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    const tt = (window as any).__ot_test;
    let line = 0;
    // CONNECTED-RAIL gate (#infrastructure): every line must root at the capital network. Bootstrap a
    // BREAD chain ANCHORED at the capital (capital → src → town → src) — a two-input Liebig tribute that
    // funds the war (this is the intended capital→grain→starter bootstrap).
    tt.drawLine([capitalIdx, nearestSrc(bread.t, bread.t.recipe[0]), bread.i, nearestSrc(bread.t, bread.t.recipe[1])]);
    tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
    // The reachable two legs of the ARMS chain, ALSO rooted at the capital (capital → ore → forge → arms
    // town); the aether leg waits on conquest (the aether sits off the connected network until then).
    const oreForgeArms = tt.drawLine([capitalIdx, oreSid, forgeSid, armsTi]);
    tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
    // Conquest: capital-barracks → the arms town, with a bounty to steer the legion onto it. Capturing the
    // arms town flips it to a HOLDING — a new rail ROOT you may extend the (otherwise unreachable) aether from.
    tt.drawLine([capitalIdx, armsTi]);
    tt.assignTrainset(line, 2); tt.setHeadwayMs(line, 120000);
    tt.postBounty(armsTi, 3000);
    line++;
    return { ruleset: tt.stats().ruleset, armsRecipe: arms.recipe, forgeKind: forge.kind, armsTi, aetherSid, oreForgeArms, line };
  });
  expect(start.ruleset).toBe("arcadia");
  expect(start.armsRecipe).toContain(4); // a genuine 3-stage (INGOT-consuming) town
  expect(start.forgeKind).toBe("forge"); // the middle node is a processor
  expect(start.oreForgeArms).toBeGreaterThanOrEqual(0); // the reachable two legs committed

  // Phase 1 — run until conquest CAPTURES the arms town (extending the realm to its aether).
  let captured = 0;
  for (let i = 0; i < 30 && captured < 1; i++) {
    captured = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(150000);
      return (window as any).__ot_test.stats().townsCaptured;
    });
  }
  expect(captured).toBeGreaterThanOrEqual(1); // the legion conquered the arms town — the frontier moved

  // Phase 2 — the captured arms town is now a rail ROOT: extend rail FROM it to the aether (draw outward
  // from the holding — the first stop must be on-network, so the captured town leads, the aether follows).
  const aetherLeg = await page.evaluate((aetherSid: number) => {
    const tt = (window as any).__ot_test;
    const armsTi = (tt.stats().perStation.find((p: any) => p.captured) || {}).stationId;
    const ln = tt.drawLine([armsTi ?? 0, aetherSid]); // arms town (captured root) → aether (the 2nd Liebig input)
    if (ln >= 0) { tt.assignTrainset(ln, 4); tt.setHeadwayMs(ln, 120000); }
    return ln;
  }, start.aetherSid);
  expect(aetherLeg).toBeGreaterThanOrEqual(0); // conquest UNLOCKED the formerly-gated aether leg

  // Phase 3 — with all three legs flowing, the 3-stage chain closes: ore→forge→INGOT + aether → Liebig.
  let econ = { gold: 0, mana: 0, manpower: 0 };
  const traj: any[] = [];
  for (let i = 0; i < 30; i++) {
    econ = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(150000);
      const s = (window as any).__ot_test.stats();
      return { gold: s.tribute, mana: s.mana, manpower: s.manpower };
    });
    traj.push(econ);
    if (econ.manpower > 0 && econ.mana > 0) break;
  }
  // eslint-disable-next-line no-console
  console.log("3-STAGE FORGE-LINE (post-conquest) econ trajectory:", JSON.stringify(traj.slice(-3)));
  // The arms town consumes INGOT (forged from ore) + AETHER → it mints MANPOWER (ingot) and MANA (aether)
  // alongside GOLD. A positive manpower is direct proof the forge converted ore→ingot AND commodity routing
  // carried the ore to the forge (not past it) — the 3-stage mechanic, closed only after conquest reached aether.
  expect(econ.manpower).toBeGreaterThan(0); // INGOT delivered → manpower (the forge fired)
  expect(econ.mana).toBeGreaterThan(0); // AETHER delivered → mana (the unlocked leg flowed)
  expect(econ.gold).toBeGreaterThan(0); // …and gold all the same

  await page.screenshot({ path: "../../docs/progress/fantasy-economy-split.png" });
});
