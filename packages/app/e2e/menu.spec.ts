import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

// F5 — the start menu: pick a city + mode, then boot it.
test("start menu boots the chosen city", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator('[data-testid="menu"]')).toBeVisible();

  // Real cities collapse under a submenu (the fantasy campaign is the headline) — expand it first.
  await page.locator('[data-testid="real-cities-toggle"]').click();
  // Choose Tokyo, empty sandbox, start.
  await page.locator('[data-testid="city-tokyo"]').click();
  await page.locator('[data-testid="mode-sandbox"]').click();
  await page.screenshot({ path: "../../docs/progress/f5-menu.png" });
  await page.locator('[data-testid="start"]').click();

  await page.waitForFunction(() => window.__MAP_READY === true, undefined, { timeout: 30_000 });
  await expect(page.locator("#app-title")).toContainText("Tokyo");
  // Empty sandbox => no pre-seeded lines.
  expect(await page.evaluate(() => window.__ot!.bridge.linesView().length)).toBe(0);
  mkdirSync("../../docs/progress", { recursive: true });
});
