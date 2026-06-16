import { expect, test } from "@playwright/test";

// CONNECTED-RAIL GATE (#infrastructure): the realm's network must be ONE graph rooted at the capital —
// rail extends only from a station already wired to your seat (or a town conquest has flipped). A fresh
// line seeded at a NON-holding is rejected; a line anchored at the capital reaches ANY distance. This
// asserts those behavioural facts on the REAL baked world, then corroborates the gold rail-frontier
// node-halos with a screenshot. The core gate (`World::compute_rail_reachable`) is unit-tested in
// crates/sim/tests/influence.rs; this proves it reached the bundle + that the bake is winnable (the
// capital is always a valid seed to bootstrap from).
test("fantasy connected-rail gate: holdings build, isolated stops reject; the rail frontier renders", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const result = await page.evaluate(() => {
    const g = (window as any).__ot.game;
    const hops: number = g.influenceHops;
    const m = (window as any).__ot.city.raw;
    const dec = m.supplyGraph.decadenceSeed;
    const cap = { x: dec.capitalXMm, y: dec.capitalYMm };
    const towns: any[] = m.supplyGraph.towns;
    const d = (t: any) => Math.hypot(t.xMm - cap.x, t.yMm - cap.y);
    const capitalIdx = towns.findIndex((t) => t.kind === "capital");
    // Town station ids == their index in sg.towns (placed first, in order, by applyNetwork). Two
    // NON-holding towns: the nearest neutral and the farthest — both off-network at load.
    const nonCap = towns.map((t, i) => ({ i, t })).filter((x) => x.t.kind !== "capital").sort((a, b) => d(a.t) - d(b.t));
    const near = nonCap[0];
    const far = nonCap[nonCap.length - 1];
    const tt = (window as any).__ot_test;
    // The frontier overlay reads the ~3 Hz `lastStats` snapshot (the "two clocks" rule — buildView never
    // calls the core each frame). The test mutates synchronously, so push a fresh snapshot before each read
    // (in production the halo updates within ~333 ms of the build). `reachable` is the core's gate output.
    const frontierLen = () => { (g as any).setStats(tt.stats()); return (g as any).buildView().frontier.length; };

    // Before any rail, the frontier is EXACTLY the roots — on the fresh baked world (no conquest yet)
    // just the capital. No false halos around still-neutral towns.
    const frontier0 = frontierLen();

    // (1) A fresh line seeded at a NON-holding town is REJECTED — connectivity is required (the network
    // must root at the capital). Neither town is the capital or captured, so the first stop is off-network.
    const isolatedLine = tt.drawLine([near.i, far.i]);

    // (2) A line seeded at the CAPITAL (a root) commits and welds the town onto the network — distance is
    // no longer a gate (the OLD influence disc was a radius; connectivity is not). We extend to the NEAR
    // town so the build stays within the gold budget — the long-haul reach is proven distance-free in the
    // native unit test (`an_anchored_line_extends_to_any_distance`); here the cost gate is a separate lever.
    const capitalLine = tt.drawLine([capitalIdx, near.i]);

    // The frontier now spans the capital + the near town (the line welded it onto the network).
    const frontier1 = frontierLen();

    return {
      hops,
      nearKm: Math.round(d(near.t) / 1e6),
      farKm: Math.round(d(far.t) / 1e6),
      frontier0,
      isolatedLine,
      capitalLine,
      frontier1,
    };
  });

  // eslint-disable-next-line no-console
  console.log("CONNECTED-RAIL GATE:", JSON.stringify(result));
  expect(result.hops).toBeGreaterThan(0); // the gate is active on the baked world
  // Before conquest the frontier is EXACTLY the capital root — one gold halo, no false reach.
  expect(result.frontier0).toBe(1);
  // A line seeded at a non-holding station is rejected (rolled back ⇒ -1) — connectivity, not proximity.
  expect(result.isolatedLine).toBe(-1);
  // A capital-anchored line commits — the realm builds out from its seat (the gate honours the holding).
  expect(result.capitalLine).toBeGreaterThanOrEqual(0);
  expect(result.farKm).toBeGreaterThan(result.nearKm); // the far town exists (the disc would have gated it)
  // The committed line welded the near town onto the network → the frontier grew past the lone capital.
  expect(result.frontier1).toBeGreaterThan(result.frontier0);

  // Establish on the realm so the gold rail-frontier halos + the new line are legible in the shot.
  await page.evaluate(() => {
    const cap = (window as any).__ot.game.towns.find((t: any) => t.kind === "capital");
    if (cap) (window as any).__ot.map.easeTo({ center: [cap.lng, cap.lat], zoom: 9.6, duration: 0 });
  });
  await page.waitForTimeout(1500); // let deck redraw the frontier halos + the committed line
  await page.screenshot({ path: "../../docs/progress/fantasy-influence.png" });
});
