import { expect, test } from "@playwright/test";

// S11 — the TECH tree on the BAKED world. The realm spends TRIBUTE (the war-chest that funds legions) on
// permanent upgrades via Command::UnlockTech. This proves the full path on the real bundle: supply a town
// to earn tribute, then unlock FORGE MASTERY (tech 0, cost 24) and assert the gameplay facts — the tech's
// bit flips in the techUnlocked bitset AND exactly its cost is deducted from tribute (the core afford-gates
// + rejects a repeat, so the spend is exactly-once). Deterministic via tickMs (no rAF). The EFFECT (doubled
// production) is proven exhaustively by the native tests/tech.rs; this gate certifies the command + readout
// reach the wasm + HUD on the baked continent.
const FORGE_MASTERY = 0;
const FORGE_MASTERY_COST = 24; // mirrors crates/sim/tech.rs TECHS[0].cost / codec TECHS

test("fantasy baked world: unlock a tech — spends tribute, sets the bit", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  // Supply a 2-stage BREAD town (raw recipe, commodity < 4) so tribute flows — the same wiring the
  // playability spec uses. Both chain inputs (grain + fuel) routed to the town between them (Liebig).
  const ready = await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const KIND: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };
    const hex = (a: any, b: any) => (Math.abs(a.q - b.q) + Math.abs(a.q + a.r - b.q - b.r) + Math.abs(a.r - b.r)) / 2;
    const ti = sg.towns.findIndex((t: any) => t.kind !== "capital" && t.recipe?.length === 2 && t.recipe.every((c: number) => c < 4));
    const town = sg.towns[ti];
    const nearestSrc = (comm: number) => {
      let bi = -1, bd = 1e9;
      sg.resources.forEach((r: any, ri: number) => {
        if (KIND[r.kind] === comm) { const d = hex(town, r); if (d < bd) { bd = d; bi = ri; } }
      });
      return bi < 0 ? -1 : nt + bi;
    };
    const tt = (window as any).__ot_test;
    tt.drawLine([nearestSrc(town.recipe[0]), ti, nearestSrc(town.recipe[1])]);
    tt.assignTrainset(0, 4);
    tt.setHeadwayMs(0, 120000);
    return { ruleset: tt.stats().ruleset, techUnlocked: tt.stats().techUnlocked };
  });
  expect(ready.ruleset).toBe("arcadia");
  expect(ready.techUnlocked).toBe(0); // no tech owned at the start

  // Run deterministically until the realm can afford the tech (tribute ≥ its cost).
  let tribute = 0;
  for (let i = 0; i < 40; i++) {
    tribute = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(120000); // 120 sim-sec per step
      return (window as any).__ot_test.stats().tribute;
    });
    if (tribute >= 24) break;
  }
  expect(tribute).toBeGreaterThanOrEqual(FORGE_MASTERY_COST); // the supply loop funded the tech

  // Unlock FORGE MASTERY and assert the two gameplay facts: the bit flips + exactly its cost is spent.
  const after = await page.evaluate((tech) => {
    const tt = (window as any).__ot_test;
    const before = tt.stats().tribute;
    tt.unlockTech(tech);
    const s = tt.stats();
    return { before, tribute: s.tribute, techUnlocked: s.techUnlocked };
  }, FORGE_MASTERY);
  expect(after.techUnlocked & (1 << FORGE_MASTERY)).toBeGreaterThan(0); // the tech's bit is set
  expect(after.before - after.tribute).toBe(FORGE_MASTERY_COST); // exactly the cost was deducted

  // A REPEAT unlock is rejected by the core (no second spend) — the buy is exactly-once.
  const repeat = await page.evaluate((tech) => {
    const tt = (window as any).__ot_test;
    const before = tt.stats().tribute;
    tt.unlockTech(tech);
    return { before, after: tt.stats().tribute };
  }, FORGE_MASTERY);
  expect(repeat.after).toBe(repeat.before); // re-unlocking spends nothing

  // The HUD reflects ownership: the tech panel marks tech 0 owned (data-owned="1").
  await expect(page.getByTestId("tech-0")).toHaveAttribute("data-owned", "1");

  await page.screenshot({ path: "../../docs/progress/fantasy-tech.png" });
});
