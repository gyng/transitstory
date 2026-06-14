import { expect, test } from "@playwright/test";

// BALANCE PLAYTEST (not a hard gate — a telemetry probe): build a representative arcadia economy on the
// BAKED world (BREAD chains → gold+manpower, the 3-stage arms chain → mana+manpower+gold, a capital-
// barracks conquest line), run a long deterministic stretch, and LOG the channel + threat trajectory so
// the tech/spell/economy costs can be tuned against real pacing. Deterministic via tickMs.
test("fantasy baked-world balance playtest (telemetry)", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const nearestRes = (node: any, comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => { if (KIND[r.kind] === comm) { const d = hex(node, r); if (d < bd) { bd = d; bi = ri; } } });
      return bi < 0 ? -1 : nt + bi;
    };
    const nearestForge = (node: any) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => { if (r.kind === "forge") { const d = hex(node, r); if (d < bd) { bd = d; bi = ri; } } });
      return bi < 0 ? -1 : nt + bi;
    };
    const tt = (window as any).__ot_test;
    const capitalIdx = sg.towns.findIndex((t: any) => t.kind === "capital");
    const sinks = sg.towns.map((t: any, i: number) => ({ t, i })).filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2);
    const bread = sinks.filter((x: any) => x.t.recipe.every((c: number) => c < 4));
    let line = 0;
    // Two BREAD chains → gold + manpower.
    for (const { t, i } of bread.slice(0, 2)) {
      tt.drawLine([nearestRes(t, t.recipe[0]), i, nearestRes(t, t.recipe[1])]);
      tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
    }
    // The 3-stage arms chain → mana (+ manpower) — ore→forge, forge→arms-town, aether→arms-town.
    const armsTi = sg.towns.findIndex((t: any) => Array.isArray(t.recipe) && t.recipe.includes(4));
    if (armsTi >= 0) {
      const armsTown = sg.towns[armsTi];
      const forge = nearestForge(armsTown);
      const ore = nearestRes(sg.resources[forge - nt], 0);
      const aeth = nearestRes(armsTown, 2);
      tt.drawLine([ore, forge]); tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
      tt.drawLine([forge, armsTi]); tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
      tt.drawLine([aeth, armsTi]); tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
    }
    // Conquest line: capital-barracks → nearest town.
    const cap = sg.towns[capitalIdx];
    const target = sinks.slice().sort((a: any, b: any) => hex(cap, a.t) - hex(cap, b.t))[0];
    tt.drawLine([capitalIdx, target.i]); tt.assignTrainset(line, 2); tt.setHeadwayMs(line, 120000);
  });

  const traj: any[] = [];
  for (let i = 0; i < 30; i++) {
    const s = await page.evaluate(() => {
      const tt = (window as any).__ot_test;
      tt.tickMs(60000); // 60 sim-sec per step → 1 min granularity
      // Logistician-bot: buy affordable techs in a priority order. Survival + economy FIRST (Sappers →
      // Forge → Ley Tap → spines/branches), and the spell arm (11) LAST. Spells DON'T auto-drain mana now
      // (manual cast is the default), so the whole tree is reachable; the order is just sensible tech-first.
      // unlockTech is idempotent (rejects owned/unaffordable/prereq-missing).
      for (const id of [2, 0, 9, 1, 3, 10, 6, 8, 7, 4, 5, 11]) tt.unlockTech(id);
      let st = tt.stats();
      let techs = 0; for (let b = 0; b < 12; b++) if (st.techUnlocked & (1 << b)) techs++;
      // Once the tree is complete, DEFEND with MANUAL casts (the new default — no autocast): Purge the tide,
      // Smite a breacher. Exercises the CastSpell command end-to-end through the real wasm; idempotent-safe
      // (the core no-ops a cast with no mana / no target). A real player casts when threatened; the bot
      // casts whenever it can once teched.
      if (techs === 12) { tt.castSpell(0); tt.castSpell(1); st = tt.stats(); }
      return { min: 0, gold: Math.round(st.tribute), mana: Math.round(st.mana), manpower: Math.round(st.manpower), dec: Math.round(st.decadencePct), towns: st.townsCaptured, raiders: st.raiderCount, spells: st.spellsCast, techs, lost: st.realmLost };
    });
    s.min = i + 1;
    traj.push(s);
    if (s.lost) break;
  }
  // eslint-disable-next-line no-console
  console.log("PLAYTEST TRAJECTORY (per sim-minute):\n" + traj.map((s) => `  m${s.min}: gold=${s.gold} mana=${s.mana} manpower=${s.manpower} dec=${s.dec}% towns=${s.towns} raiders=${s.raiders} techs=${s.techs} spells=${s.spells}${s.lost ? " LOST" : ""}`).join("\n"));
  await page.screenshot({ path: "../../docs/progress/fantasy-playtest.png" });
  expect(traj.length).toBeGreaterThan(0);
});
