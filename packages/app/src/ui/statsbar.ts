// Top-centre HUD: the headline Ridership counter + a 0–100 Coverage/Satisfaction gauge.
// Fed by the ~3 Hz stats throttle (never per frame). One number + one gauge, details on
// demand elsewhere (AGENTS IA).
import type { Stats } from "../types";

export interface StatsBar {
  update(s: Stats): void;
}

export function mountStatsBar(host: HTMLElement): StatsBar {
  const bar = document.createElement("div");
  bar.id = "stats-bar";
  bar.dataset.testid = "stats-bar";
  bar.style.cssText =
    "position:fixed;top:10px;left:50%;transform:translateX(-50%);display:flex;align-items:center;" +
    "gap:16px;padding:8px 14px;background:rgba(255,255,255,.95);border-radius:10px;" +
    "box-shadow:var(--ot-shadow);z-index:9;font:13px system-ui,sans-serif;color:#1c2024";

  bar.innerHTML =
    `<div><b data-testid="clock" style="font-variant-numeric:tabular-nums">06:00</b> ` +
    `<span data-testid="period" style="color:#7a818a">AM rush</span></div>` +
    `<div style="width:1px;align-self:stretch;background:#e2e5e9"></div>` +
    `<div>🚇 <b data-testid="ridership" style="font-size:16px">0</b> riders</div>` +
    `<div style="display:flex;align-items:center;gap:8px">Coverage` +
    `<div style="position:relative;width:90px;height:10px;background:#e7eaee;border-radius:6px;overflow:hidden">` +
    `<div data-testid="coverage-bar" style="position:absolute;inset:0 100% 0 0;background:var(--ot-gauge-good)"></div>` +
    `</div><b data-testid="coverage" style="width:26px;text-align:right">0</b></div>` +
    `<div style="color:#7a818a"><span data-testid="waiting">0</span> waiting</div>` +
    `<div>Build impact <b data-testid="build-impact" style="color:var(--ot-gauge-good)">0</b></div>`;

  host.appendChild(bar);

  const clock = bar.querySelector<HTMLElement>('[data-testid="clock"]')!;
  const period = bar.querySelector<HTMLElement>('[data-testid="period"]')!;
  const ridership = bar.querySelector<HTMLElement>('[data-testid="ridership"]')!;
  const coverage = bar.querySelector<HTMLElement>('[data-testid="coverage"]')!;
  const coverageBar = bar.querySelector<HTMLElement>('[data-testid="coverage-bar"]')!;
  const waiting = bar.querySelector<HTMLElement>('[data-testid="waiting"]')!;
  const impact = bar.querySelector<HTMLElement>('[data-testid="build-impact"]')!;

  let lastRidership = -1;
  let lastCoverage = -1;
  let lastWaiting = -1;
  let lastImpact = -1;
  let lastClock = "";

  return {
    update(s: Stats): void {
      const hh = Math.floor(s.simHour);
      const mm = Math.floor((s.simHour - hh) * 60);
      const t = `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
      if (t !== lastClock) {
        clock.textContent = t;
        period.textContent = s.period;
        lastClock = t;
      }

      const r = Math.round(s.ridershipTotal);
      if (r !== lastRidership) {
        ridership.textContent = String(r);
        lastRidership = r;
      }
      const c = Math.round(s.coverageScore);
      if (c !== lastCoverage) {
        coverage.textContent = String(c);
        // Bar fills left→right; hue shifts good→bad as coverage drops.
        coverageBar.style.right = `${100 - c}%`;
        coverageBar.style.background = c >= 60 ? "var(--ot-gauge-good)" : c >= 30 ? "#e69f00" : "var(--ot-gauge-bad)";
        lastCoverage = c;
      }
      const w = Math.round(s.waitingTotal);
      if (w !== lastWaiting) {
        waiting.textContent = String(w);
        lastWaiting = w;
      }
      const bi = Math.round(s.buildDifficulty);
      if (bi !== lastImpact) {
        impact.textContent = String(bi);
        impact.style.color = bi >= 50 ? "var(--ot-gauge-bad)" : bi >= 20 ? "#e69f00" : "var(--ot-gauge-good)";
        lastImpact = bi;
      }
    },
  };
}
