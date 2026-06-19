import { expect, test } from "@playwright/test";

// S11 RIVAL — decadence raiders on the BAKED world. The decadence isn't just a passive tide: it FIELDS
// marauders from the far-edge reservoir that march the capital and deepen the rot if they get through; the
// player's rail network cuts them down. This proves, on the real bundle, that the rival is LIVE — raiders
// actually field on the baked continent (raidersFielded > 0, the cumulative spawn count) — and that they
// don't break winnability (the realm still holds). NOTE: a well-covered network cuts raiders the INSTANT
// they spawn, so the LIVE count (raiderCount) can read 0 throughout even under steady assault — which is why
// this asserts the cumulative raidersFielded, not an instantaneous catch. The native tests/raider.rs gate
// the structural invariants (bounded, monotone, etc.). Deterministic via tickMs.
test("fantasy baked world: the rival fields raiders, the realm still holds", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  // Build the canonical conquest loop (mirrors fantasy-conquest): supply BREAD towns, a capital-barracks
  // line to the nearest town + a bounty — partial network coverage, so some raiders are cut down and some
  // get through (the realm must still hold).
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
    // #infrastructure connected-rail gate: root each bread chain at the capital network (capital → src → town → src).
    for (const { t, i } of breadSinks.slice(0, 2)) {
      tt.drawLine([capitalIdx, nearestSrc(t, t.recipe[0]), i, nearestSrc(t, t.recipe[1])]);
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

  // Run deterministically, sampling the rival + the realm. The decadence-fed cadence fields raiders as the rot
  // deepens; the covering network cuts them down on contact (so raiderCount, the LIVE count, can stay 0), but
  // raidersFielded — the cumulative spawn count — climbs, proving the rival genuinely musters marauders.
  let last: any = null;
  for (let i = 0; i < 24; i++) {
    last = await page.evaluate(() => {
      (window as any).__ot_test.tickMs(150000);
      const s = (window as any).__ot_test.stats();
      return { raidersFielded: s.raidersFielded, townsCaptured: s.townsCaptured, realmLost: s.realmLost, decadencePct: Math.round(s.decadencePct) };
    });
    if (last.townsCaptured >= 1 && last.raidersFielded > 0) break; // both facts observed → done
  }

  expect(last.raidersFielded).toBeGreaterThan(0); // the rival is LIVE — marauders field from the reservoir (the rail then cuts them down)
  expect(last.townsCaptured).toBeGreaterThanOrEqual(1); // conquest still lands…
  expect(last.realmLost).toBe(false); // …and the realm holds despite the raids (the network defends)

  await page.screenshot({ path: "../../docs/progress/fantasy-rival.png" });
});
