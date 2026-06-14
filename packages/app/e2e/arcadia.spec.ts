import { test, expect } from "@playwright/test";

// The fantasy fork (arcadia) end-to-end: load the baked Arcadia world + network, run it, and assert
// the WHOLE loop as concrete behavioural facts (never a load-only green) — the ruleset is arcadia,
// supply flows into TRIBUTE, a legion is fielded AND rendered (armyPositions), and a town is CONQUERED
// while the realm holds (decadence pushed back). This locks in the entire sim→wasm→frontend fantasy
// path (the HUD + army render + barracks bake) as a regression gate. Camera-independent; waits on
// window flags + the test hook, never sleeps.
type ArcStats = { ruleset: string; tribute: number; townsCaptured: number; armyCount: number; realmLost: boolean };

// Advance the sim SYNCHRONOUSLY in steps (tickMs — no rAF wall-clock, so it's deterministic + immune to
// the parallel-load rAF starvation that made the old setSpeed(100)+waitForFunction approach flaky). Steps
// so we can catch the TRANSIENT "a legion is afield + rendered" (armyPositions, the sim→wasm→frontend path)
// before it disbands. Stops as soon as a town falls. Returns the facts + whether a legion was ever rendered.
async function runToConquest(page: import("@playwright/test").Page) {
  let everRendered = false;
  let last = { tribute: 0, armyCount: 0, townsCaptured: 0, realmLost: false, rendered: 0 };
  for (let i = 0; i < 12; i++) {
    last = await page.evaluate(() => {
      window.__ot_test!.tickMs(50_000); // 50 sim-sec per step
      const s = window.__ot_test!.stats() as unknown as ArcStats;
      return { tribute: s.tribute, armyCount: s.armyCount, townsCaptured: s.townsCaptured, realmLost: s.realmLost, rendered: window.__ot!.bridge.armyPositions().length };
    });
    if (last.rendered > 0) everRendered = true;
    if (last.townsCaptured >= 1) break;
  }
  return { ...last, everRendered };
}

test("arcadia: supply → tribute → legions → a town falls, realm holds", async ({ page }) => {
  // The baked Arcadia world ships a barracks-anchored network, so it runs the full loop on load.
  await page.goto("/?city=arcadia&network=1");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  // It IS the fantasy mode (the ruleset crossed the boundary + selected ArcadiaRuleset).
  const ruleset = await page.evaluate(() => (window.__ot_test!.stats() as unknown as ArcStats).ruleset);
  expect(ruleset).toBe("arcadia");

  // The supply loop spawns commodities, towns consume them into tribute, tribute funds a legion from the
  // barracks, it marches + besieges + flips the target town. Driven by deterministic synchronous ticks.
  const r = await runToConquest(page);

  // Concrete facts: tribute earned, a legion fielded AND rendered (the sim→wasm→frontend render path),
  // a town taken, the realm not (yet) fallen.
  expect(r.tribute).toBeGreaterThan(0);
  expect(r.everRendered).toBe(true);
  expect(r.townsCaptured).toBeGreaterThanOrEqual(1);
  expect(r.realmLost).toBe(false);

  // The HUD reflects the fantasy readout (tribute box + towns-taken), not transit chrome.
  await expect(page.locator('[data-testid="tribute"]')).toBeVisible();
  await expect(page.locator('[data-testid="towns-captured"]')).toBeVisible();

  await page.screenshot({ path: "../../docs/progress/fantasy-arcadia-e2e.png" });
});

// The PLAYER-DRIVEN war loop: build a barracks (the new fantasy tool's path, Game.placeBarracks) + a
// route by hand, run, and a legion launches from the player's barracks and takes a town. Exercises the
// build-tool code path the UI button + pointer share (camera-independent via the geo.ts-routed hooks).
test("arcadia: a player-built barracks fields legions that conquer", async ({ page }) => {
  await page.goto("/?city=arcadia"); // no network — the player builds it
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  await page.evaluate(() => {
    const t = window.__ot_test!;
    const forge = t.placeBarracksLngLat(0.0, 0.0); // a barracks at the ore source
    const town = t.placeStationLngLat(0.02, 0.0); // the town to supply + take
    t.drawLine([forge, town]);
    t.assignTrainset(0, 3);
    t.postBounty(town, 2000); // the bounty tool's path: bait legions toward the town
  });

  // The player's barracks fields a legion (rendered) that conquers the town — deterministic synchronous run.
  const r = await runToConquest(page);
  expect(r.tribute).toBeGreaterThan(0);
  expect(r.everRendered).toBe(true); // a legion was fielded AND rendered from the player's barracks
  expect(r.townsCaptured).toBeGreaterThanOrEqual(1);
});
