import { expect, test } from "@playwright/test";

// Checkpoint corroboration (NOT a behavioural gate): once Arcane Awakening (tech 11) is unlocked, the
// SPELL BAR appears (bottom-right) — 3 auto-targeted, player-cast spells + an autocast checkbox. Builds the
// economy, teches the whole tree, then captures the spell bar + tech panel together. Deterministic via tickMs.
test("fantasy spell bar renders once Arcane Awakening is unlocked", async ({ page }) => {
  test.setTimeout(120_000); // the connected-rail arms haul ramps mana slower → a longer deterministic run
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  // Build the same representative economy the playtest uses (BREAD chains + arms chain) so mana flows.
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
    // #infrastructure connected-rail gate: every line roots at the capital network.
    const capitalIdx = sg.towns.findIndex((t: any) => t.kind === "capital");
    const sinks = sg.towns.map((t: any, i: number) => ({ t, i })).filter((x: any) => x.t.kind !== "capital" && x.t.recipe?.length === 2);
    const bread = sinks.filter((x: any) => x.t.recipe.every((c: number) => c < 4));
    let line = 0;
    for (const { t, i } of bread.slice(0, 2)) {
      tt.drawLine([capitalIdx, nearestRes(t, t.recipe[0]), i, nearestRes(t, t.recipe[1])]);
      tt.assignTrainset(line, 4); tt.setHeadwayMs(line, 120000); line++;
    }
    const armsTi = sg.towns.findIndex((t: any) => Array.isArray(t.recipe) && t.recipe.includes(4));
    if (armsTi >= 0) {
      const armsTown = sg.towns[armsTi];
      const forge = nearestForge(armsTown);
      const ore = nearestRes(sg.resources[forge - nt], 0);
      const aeth = nearestRes(armsTown, 2);
      // Capital-rooted ore→forge→arms (one line, affordable — the long capital→ore haul is the only one),
      // then the aether leg drawn FROM the on-network arms town (armsTi is reachable via the line above;
      // armsTi→aether is a short local span, so it stays inside the gold budget — a direct capital→aether
      // line would be cost-gated). Mirrors fantasy-multistage's proven shape.
      // 8 trains on the long capital-rooted ore→forge→arms leg (the connectivity gate routes the arms supply
      // the long way from the capital now, so pack the haul to keep mana ramping inside the run budget).
      tt.drawLine([capitalIdx, ore, forge, armsTi]); tt.assignTrainset(line, 8); tt.setHeadwayMs(line, 90000); line++;
      tt.drawLine([armsTi, aeth]); tt.assignTrainset(line, 6); tt.setHeadwayMs(line, 90000); line++;
    }
  });

  // Earn mana + buy the whole tree (Arcane Awakening last) over ~24 sim-min, then leave a mana surplus so
  // the spell buttons read as affordable in the shot.
  await page.evaluate(() => {
    const tt = (window as any).__ot_test;
    // Longer sim-time per step (150 sim-sec) so the long-haul arms chain accrues enough mana to climb the
    // whole tree to Arcane Awakening (the connected-rail route is slower than the old local arms lines).
    for (let i = 0; i < 30; i++) {
      tt.tickMs(150000);
      for (const id of [2, 0, 9, 1, 3, 10, 6, 8, 7, 4, 5, 11]) tt.unlockTech(id);
    }
  });

  // SETTLE: advance a few more sim-minutes in SEPARATE turns, pausing between so the ~3 Hz StatsRecorder
  // samples each — this populates the rolling history that drives the per-minute flow-rate pills + the
  // decadence ETA (the synchronous loop above never yields to the recorder). Mirrors real continuous play.
  for (let i = 0; i < 6; i++) {
    await page.evaluate(() => (window as any).__ot_test.tickMs(60000));
    await page.waitForTimeout(450);
  }

  // The spell bar is present (Arcane Awakening unlocked) with its three spells + the autocast toggle.
  await expect(page.getByTestId("spell-bar")).toBeVisible();
  await expect(page.getByTestId("spell-0")).toBeVisible(); // Purge
  await expect(page.getByTestId("spell-1")).toBeVisible(); // Smite
  await expect(page.getByTestId("spell-2")).toBeVisible(); // Warpath
  await expect(page.getByTestId("autocast-toggle")).toBeVisible();

  await page.waitForTimeout(800);
  await page.screenshot({ path: "../../docs/progress/fantasy-spellbar.png" });
});
