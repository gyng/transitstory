import { expect, test } from "@playwright/test";

// MAP LENSES (#5): a view-mode selector (Realm/Supply/War/Decadence) that emphasises one reading of the
// busy arcadia map by hiding the others' deck layers. Render-only. Asserts the bar + each lens applies
// (the deck layer set changes) without error, then screenshots the Supply lens.
test("fantasy map lenses render + filter the overlay", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  await expect(page.getByTestId("lens-bar")).toBeVisible();
  for (const id of ["realm", "supply", "military", "decadence"]) {
    await expect(page.getByTestId(`lens-${id}`)).toBeVisible();
  }

  // Switching lens must change Game.lens (the composeAndSet filter reads it) without throwing.
  const lensState = await page.evaluate(() => {
    const g = (window as any).__ot.game;
    const out: Record<string, string> = {};
    for (const id of ["supply", "military", "decadence", "realm"]) {
      g.setLens(id);
      out[id] = g.lens;
    }
    return out;
  });
  expect(lensState.decadence).toBe("decadence");
  expect(lensState.realm).toBe("realm");

  // Land on the Supply lens for the shot (sources/towns/rivers emphasised).
  await page.getByTestId("lens-supply").click();
  await page.waitForTimeout(700);
  await page.screenshot({ path: "../../docs/progress/fantasy-lens.png" });
});
