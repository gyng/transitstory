import { expect, test } from "@playwright/test";

// S11 — the TECH tree + RAIL-GATE on the BAKED world. Arcadia builds RAIL only (bus/ferry/plane gated out;
// heavy rail is a tech unlock), and the Forge-of-Ages tech panel offers mana-bought upgrades. This proves
// the headline new behaviour on the real bundle: the toolbar shows rail (not bus/ferry), a rail line builds,
// the tech panel renders, and a tech is MANA-gated (with no mana it can't be bought). The full unlock→effect
// path is proven exhaustively in the native tests (tech.rs / tech_effects.rs / economy_split.rs).
test("fantasy baked world: rail-only gate + the tech panel", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto("/?city=fantasy");
  await page.waitForFunction(() => (window as any).__APP_READY && (window as any).__MAP_READY, undefined, { timeout: 30_000 });

  const ruleset = await page.evaluate(() => (window as any).__ot_test.stats().ruleset);
  expect(ruleset).toBe("arcadia");

  // RAIL-GATE: the toolbar offers RAIL (mode 0) but NOT bus/ferry/plane/heavy (1/2/3/4 — heavy needs the tech).
  await expect(page.getByTestId("mode-transport-0")).toBeVisible();
  for (const mode of [1, 2, 3, 4]) {
    await expect(page.getByTestId(`mode-transport-${mode}`)).toHaveCount(0);
  }

  // The realm still builds rail (a line through two baked stations) — the gate didn't break construction.
  const built = await page.evaluate(async () => {
    const m = await (await fetch("/data/fantasy_world.json")).json();
    const sg = m.supplyGraph;
    const nt = sg.towns.length;
    const tt = (window as any).__ot_test;
    tt.drawLine([nt + 0, nt + 1]); // a rail line between the first two resources
    return tt.stats().lineCount;
  });
  expect(built).toBeGreaterThan(0); // rail is buildable in the realm

  // The TECH tree lives behind a launcher (collapsed by default); open it. It offers the mana-bought
  // upgrades; Forge Mastery starts UNowned and (mana 0) unaffordable — tech is MANA-gated (aether is your science).
  await page.getByTestId("tech-launcher").click();
  await expect(page.getByTestId("tech-panel")).toContainText("Forge of Ages");
  await expect(page.getByTestId("tech-0")).toHaveAttribute("data-owned", "0");
  const techRejected = await page.evaluate(() => {
    const tt = (window as any).__ot_test;
    const before = tt.stats().techUnlocked;
    tt.unlockTech(0); // no mana yet → rejected, no bit set
    return { before, after: tt.stats().techUnlocked };
  });
  expect(techRejected.after).toBe(techRejected.before); // mana-gated: no mana, no tech

  // The SPELL BAR is gated behind Arcane Awakening (tech 11) — absent on a fresh realm (no SPELLCRAFT yet).
  await expect(page.getByTestId("spell-bar")).toHaveCount(0);

  await page.screenshot({ path: "../../docs/progress/fantasy-tech.png" });
});
