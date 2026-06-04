// Settings panel (⚙): toggle which transport modes are available, and switch the economy
// (capital + fares) on/off. Mode toggles are a frontend gate — they grey out the chorded
// bar's buttons; the economy toggle emits SetEconomy through Game. Opens as a small panel
// anchored above the bar; mirrors Game state and re-syncs on every refresh.
import type { Game } from "../game";
import { MODES } from "./toolbar";

export interface SettingsPanel {
  toggle(): void;
}

export function mountSettings(host: HTMLElement, game: Game): SettingsPanel {
  const panel = document.createElement("div");
  panel.id = "settings-panel";
  panel.dataset.testid = "settings-panel";
  panel.style.cssText =
    "position:fixed;bottom:84px;right:14px;width:240px;padding:14px;display:none;" +
    "background:rgba(255,255,255,.98);border-radius:12px;box-shadow:var(--ot-shadow);" +
    "z-index:11;font:13px system-ui,sans-serif;color:#1c2024";

  const toggleRow = (label: string, testid: string, get: () => boolean, set: (on: boolean) => void): HTMLElement => {
    const row = document.createElement("label");
    row.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;padding:5px 0;cursor:pointer";
    const span = document.createElement("span");
    span.textContent = label;
    const sw = document.createElement("button");
    sw.dataset.testid = testid;
    sw.style.cssText =
      "width:38px;height:22px;border-radius:11px;border:0;cursor:pointer;position:relative;transition:background .12s";
    const knob = document.createElement("span");
    knob.style.cssText =
      "position:absolute;top:2px;width:18px;height:18px;border-radius:50%;background:#fff;transition:left .12s;box-shadow:0 1px 2px rgba(0,0,0,.3)";
    sw.appendChild(knob);
    const paint = () => {
      const on = get();
      sw.style.background = on ? "#009e73" : "#c4cad0";
      knob.style.left = on ? "18px" : "2px";
    };
    sw.addEventListener("click", (e) => {
      e.preventDefault();
      set(!get());
      paint();
    });
    row.append(span, sw);
    (row as unknown as { _paint: () => void })._paint = paint;
    return row;
  };

  panel.innerHTML = `<div style="font-weight:700;margin-bottom:6px">Settings</div>` +
    `<div style="color:#7a818a;font-size:11px;margin-bottom:6px">Transport modes</div>`;

  const rows: HTMLElement[] = [];
  for (const m of MODES) {
    const r = toggleRow(
      `${m.icon}  ${m.name}`,
      `setting-mode-${m.id}`,
      () => game.enabledModes.has(m.id),
      (on) => game.setModeEnabled(m.id, on),
    );
    rows.push(r);
    panel.appendChild(r);
  }

  const econHdr = document.createElement("div");
  econHdr.style.cssText = "color:#7a818a;font-size:11px;margin:10px 0 4px;border-top:1px solid #eceef1;padding-top:10px";
  econHdr.textContent = "Economy";
  panel.appendChild(econHdr);

  const econRow = toggleRow(
    "💰  Capital & fares",
    "setting-economy",
    () => game.bridge.stats().economyEnabled,
    (on) => game.setEconomy(on),
  );
  rows.push(econRow);
  panel.appendChild(econRow);

  const repaint = () => {
    if (panel.style.display === "none") return;
    for (const r of rows) (r as HTMLElement & { _paint?: () => void })._paint?.();
  };
  game.onChange.push(repaint);

  host.appendChild(panel);

  return {
    toggle(): void {
      panel.style.display = panel.style.display === "none" ? "block" : "none";
      repaint();
    },
  };
}
