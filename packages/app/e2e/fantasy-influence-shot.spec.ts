import { expect, test } from "@playwright/test";

// AREA OF INFLUENCE (#9): you may only lay rail within `influence_hops` grid-hexes of a HOLDING (the
// capital + any captured town); conquest expands the buildable frontier. This asserts the gate's two
// behavioural facts on the REAL baked world — a near town is buildable, a far town is REJECTED (the line
// rolls back, gaining no stops) — then corroborates the realm-border overlay with a screenshot. The core
// gate (`World::buildable_at`) is unit-tested in crates/sim/tests/influence.rs; this proves it reached the
// bundle + that the bake is winnable-by-construction (a near town always exists to bootstrap from).
test("fantasy area-of-influence gates the far frontier; the realm border renders", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const result = await page.evaluate(() => {
    const g = (window as any).__ot.game;
    const hops: number = g.influenceHops;
    const m = (window as any).__ot.city.raw;
    const dec = m.supplyGraph.decadenceSeed;
    const cap = { x: dec.capitalXMm, y: dec.capitalYMm };
    const R = hops * m.gridCellMm * 1.7320508; // the euclidean reach (mm) the core uses (√3·hops·cell)
    const towns: any[] = m.supplyGraph.towns;
    const d = (t: any) => Math.hypot(t.xMm - cap.x, t.yMm - cap.y);
    const capitalIdx = towns.findIndex((t) => t.kind === "capital");
    // Town station ids == their index in sg.towns (placed first, in order, by applyNetwork).
    const near = towns.map((t, i) => ({ i, t })).filter((x) => x.t.kind !== "capital" && d(x.t) < R).sort((a, b) => d(a.t) - d(b.t))[0];
    const far = towns.map((t, i) => ({ i, t })).filter((x) => d(x.t) > R).sort((a, b) => d(a.t) - d(b.t))[0];
    const tt = (window as any).__ot_test;
    // A line to a NEAR town (within the capital's reach) commits; a line to a FAR town is gated (rolls back).
    const nearLine = tt.drawLine([capitalIdx, near.i]);
    const farLine = tt.drawLine([capitalIdx, far.i]);
    const influence = (g as any).buildView().influence as { radiusM: number }[];
    return {
      hops,
      nearKm: Math.round(d(near.t) / 1e6),
      farKm: Math.round(d(far.t) / 1e6),
      reachKm: Math.round(R / 1e6),
      nearLine,
      farLine,
      influenceDiscs: influence.length,
      influenceRadiusKm: influence.length ? Math.round(influence[0].radiusM / 1000) : 0,
    };
  });

  // eslint-disable-next-line no-console
  console.log("INFLUENCE GATE:", JSON.stringify(result));
  expect(result.hops).toBeGreaterThan(0); // the gate is active on the baked world
  expect(result.nearLine).toBeGreaterThanOrEqual(0); // a town within reach is buildable
  expect(result.farLine).toBe(-1); // a town beyond reach is REJECTED (the line rolled back)
  // Before any conquest the realm border is EXACTLY the capital disc — no false discs around still-neutral
  // towns (the overlay reads the core's `captured` flag, not the pre-siege-ambiguous resistance default).
  expect(result.influenceDiscs).toBe(1);
  expect(result.influenceRadiusKm).toBe(result.reachKm); // the overlay radius mirrors the core's reach

  // Establish on the realm so the gold reach-border + the new in-reach line are legible in the shot.
  await page.evaluate(() => {
    const cap = (window as any).__ot.game.towns.find((t: any) => t.kind === "capital");
    if (cap) (window as any).__ot.map.easeTo({ center: [cap.lng, cap.lat], zoom: 9.6, duration: 0 });
  });
  await page.waitForTimeout(1500); // let deck redraw the influence discs + the committed line
  await page.screenshot({ path: "../../docs/progress/fantasy-influence.png" });
});
