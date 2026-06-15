import { expect, test } from "@playwright/test";

// CONTROLS pass: game-feel keyboard. WASD/arrows pan the MapLibre camera (held-key rAF), Q/E zoom, Space
// pauses (Build<->Run). These drive the camera/loop imperatively — never the sim — so they're golden- and
// e2e-neutral; this spec proves they actually move the camera + toggle running.
test("keyboard controls: WASD pans, Q/E zooms, Space toggles run", async ({ page }) => {
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const center = () => page.evaluate(() => { const c = (window as any).__ot.map.getCenter(); return { lng: c.lng, lat: c.lat }; });
  const zoom = () => page.evaluate(() => (window as any).__ot.map.getZoom() as number);

  // Pan east (D): longitude increases.
  const c0 = await center();
  await page.keyboard.down("d");
  await page.waitForTimeout(300);
  await page.keyboard.up("d");
  await page.waitForTimeout(120);
  const c1 = await center();
  expect(c1.lng).toBeGreaterThan(c0.lng);

  // Pan north (W): latitude increases.
  await page.keyboard.down("w");
  await page.waitForTimeout(300);
  await page.keyboard.up("w");
  await page.waitForTimeout(120);
  const c2 = await center();
  expect(c2.lat).toBeGreaterThan(c1.lat);

  // Q zooms out.
  const z0 = await zoom();
  await page.keyboard.press("q");
  await page.waitForTimeout(250);
  expect(await zoom()).toBeLessThan(z0);

  // Space toggles Build<->Run.
  const running0 = await page.evaluate(() => (window as any).__ot.bridge.stats().running as boolean);
  await page.keyboard.press("Space");
  await page.waitForTimeout(80);
  const running1 = await page.evaluate(() => (window as any).__ot.bridge.stats().running as boolean);
  expect(running1).not.toBe(running0);
});
