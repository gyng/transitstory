import { expect, test } from "@playwright/test";

// LIVING WORLD (#living): the baked continent feels INHABITED — ambient ox-cart traders trundle the trade
// routes between the capital, towns, and resources (render-only, wall-clock animated, never sim state).
// This asserts the trade graph built + the carts actually move between two reads, then screenshots the
// living continent. Purely decorative (zero hashed state), so it's corroboration, not a determinism gate.
test("fantasy living world: ambient traders populate + move along the trade routes", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const probe = await page.evaluate(() => {
    const g = (window as any).__ot.game;
    const a = g.ambientTradersAt(1_000_000); // a sample of cart positions at t0
    const b = g.ambientTradersAt(1_030_000); // 30 s later — the ping-pong should have advanced them
    let moved = 0;
    for (let i = 0; i < a.length; i++) {
      if (Math.abs(a[i].lng - b[i].lng) > 1e-9 || Math.abs(a[i].lat - b[i].lat) > 1e-9) moved++;
    }
    return { count: a.length, moved };
  });

  expect(probe.count).toBeGreaterThan(30); // the trade graph populated carts across the continent
  expect(probe.moved).toBeGreaterThan(probe.count / 2); // most carts advanced — the world is animated, not frozen

  // Settle on the continent (mid zoom) so the trade routes between towns read in the shot, then let the
  // rAF loop animate the carts for a beat.
  await page.evaluate(() => {
    const cap = (window as any).__ot.game.towns.find((t: any) => t.kind === "capital");
    if (cap) (window as any).__ot.map.easeTo({ center: [cap.lng, cap.lat], zoom: 10.4, duration: 0 });
  });
  await page.waitForTimeout(1200);
  await page.screenshot({ path: "../../docs/progress/fantasy-living.png" });
});
