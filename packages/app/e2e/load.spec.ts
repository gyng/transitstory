import { test, expect } from "@playwright/test";
import { mkdirSync } from "node:fs";

// Skeleton smoke (the START of testing, not a feature gate): the app mounts and signals
// readiness. Feature specs (vehicle moved, ridership>0) assert behavioural facts.
test("app shell loads and signals ready", async ({ page }) => {
  await page.goto("/?city=singapore");
  await page.waitForFunction(() => window.__APP_READY === true, undefined, {
    timeout: 30_000,
  });
  await expect(page.locator("#app-title")).toContainText("onlytransits");

  mkdirSync("../../docs/progress", { recursive: true });
  await page.screenshot({ path: "../../docs/progress/cp0-app-shell.png" });
});
