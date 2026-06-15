import { test, expect } from "@playwright/test";

// Depot rework — the REAL-CITY (transit) opt-in. With "Require depots" toggled on, a line runs trains only
// if one of its stops is a DEPOT (built + connected). Asserts the per-line gate on a transit city: a depot-
// less line is refused rolling stock; a line that stops at a depot runs. Core gate is unit-tested in
// crates/sim/tests/depot.rs; this proves the toggle + the gate reach the live bundle.
test("transit depot toggle: a line needs a connected depot to run trains", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });

  const r = await page.evaluate(() => {
    const t = window.__ot_test!;
    t.setRequireDepot(true); // the real-city opt-in
    // A line with NO depot — assigning trains must be refused.
    const a = t.placeStationLngLat(103.845, 1.29);
    const b = t.placeStationLngLat(103.86, 1.31);
    const noDepot = t.drawLine([a, b]);
    t.assignTrainset(noDepot, 3);
    const trainsNoDepot = window.__ot!.bridge.stats().perLine.find((l) => l.lineId === noDepot)?.trains ?? -1;
    // A line that STOPS at a depot — trains run.
    const c = t.placeStationLngLat(103.84, 1.335);
    const depot = t.placeDepotLngLat(103.85, 1.32);
    const withDepot = t.drawLine([a, depot, c]);
    t.assignTrainset(withDepot, 3);
    const trainsWithDepot = window.__ot!.bridge.stats().perLine.find((l) => l.lineId === withDepot)?.trains ?? -1;
    const requireDepot = window.__ot!.bridge.stats().requireDepot;
    return { trainsNoDepot, trainsWithDepot, requireDepot };
  });

  expect(r.requireDepot).toBe(true); // the toggle reached the sim
  expect(r.trainsNoDepot).toBe(0); // a depot-less line was refused its trainset
  expect(r.trainsWithDepot).toBe(3); // a depot-served line runs trains

  await page.screenshot({ path: "../../docs/progress/depot-toggle.png" });
});
