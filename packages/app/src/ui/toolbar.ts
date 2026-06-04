// Chorded bottom bar: four big transport-mode buttons (1 Rail / 2 Bus / 3 Ferry / 4 Plane)
// drive construction; selecting one opens its build controls in a popover ABOVE the bar.
// Right of the modes: Run/Build, speed, the Demand map-layer toggle, and Settings. Keyboard
// 1–4 chord the modes. Emits to Game / GameLoop only (never mutates sim state directly).
import type { Game, Tool } from "../game";
import type { GameLoop } from "../sim/GameLoop";
import { mountSettings } from "./settings";

export interface ModeDef {
  id: number;
  key: string;
  icon: string;
  name: string;
  hint: string;
  color: string;
}

// Mode ids match crates/sim trainset::tmode (0 rail,1 bus,2 ferry,3 air).
export const MODES: ModeDef[] = [
  { id: 0, key: "1", icon: "🚇", name: "Rail", color: "#0072b2",
    hint: "Place stations, then draw track. Surface routes avoid buildings — elevate or tunnel to cross built-up land and water." },
  { id: 1, key: "2", icon: "🚌", name: "Bus", color: "#d55e00",
    hint: "Runs on existing roads — cheap and quick to build, but lower capacity." },
  { id: 2, key: "3", icon: "⛴", name: "Ferry", color: "#009e73",
    hint: "Terminals on the waterfront — routes cross open water with no track to build." },
  { id: 3, key: "4", icon: "✈", name: "Plane", color: "#cc79a7",
    hint: "Airports for long hops — flies over anything, at any distance." },
];

function bigModeButton(m: ModeDef, onClick: () => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.dataset.testid = `mode-transport-${m.id}`;
  b.innerHTML =
    `<span style="font-size:20px;line-height:1">${m.icon}</span>` +
    `<span style="font:600 13px system-ui,sans-serif">${m.name}</span>` +
    `<kbd style="font:600 10px system-ui;color:#9aa3ad;border:1px solid #d7dade;border-radius:4px;padding:0 4px">${m.key}</kbd>`;
  b.style.cssText =
    "display:flex;flex-direction:column;align-items:center;gap:3px;min-width:64px;" +
    "padding:8px 10px;border:2px solid #d7dade;background:#fff;color:#1c2024;" +
    "border-radius:10px;cursor:pointer";
  b.addEventListener("click", onClick);
  return b;
}

function button(label: string, testid: string, onClick: () => void): HTMLButtonElement {
  const b = document.createElement("button");
  b.textContent = label;
  b.dataset.testid = testid;
  b.style.cssText =
    "border:1px solid #d7dade;background:#fff;color:#1c2024;border-radius:7px;" +
    "padding:6px 10px;font:600 13px system-ui,sans-serif;cursor:pointer";
  b.addEventListener("click", onClick);
  return b;
}

export function mountToolbar(host: HTMLElement, game: Game, loop: GameLoop): void {
  // --- build-controls popover (opens above the bar for the active mode) ---
  const popover = document.createElement("div");
  popover.id = "mode-controls";
  popover.dataset.testid = "mode-controls";
  popover.style.cssText =
    "position:fixed;bottom:84px;left:50%;transform:translateX(-50%);display:flex;" +
    "flex-direction:column;gap:8px;padding:12px 14px;width:min(440px,92vw);" +
    "background:rgba(255,255,255,.97);border-radius:12px;box-shadow:var(--ot-shadow);z-index:10";

  const popHead = document.createElement("div");
  popHead.style.cssText = "display:flex;align-items:center;gap:8px";
  popover.appendChild(popHead);

  const popHint = document.createElement("div");
  popHint.style.cssText = "color:#7a818a;font:12px system-ui,sans-serif;line-height:1.35";
  popover.appendChild(popHint);

  const tools: [Tool, string][] = [
    ["station", "◉ Stations"],
    ["line", "╱ Draw line"],
    ["select", "▣ Select"],
  ];
  const toolRow = document.createElement("div");
  toolRow.style.cssText = "display:flex;gap:6px";
  const toolBtns = new Map<Tool, HTMLButtonElement>();
  for (const [t, label] of tools) {
    const b = button(label, `tool-${t}`, () => game.setTool(t));
    b.style.flex = "1";
    toolBtns.set(t, b);
    toolRow.appendChild(b);
  }
  popover.appendChild(toolRow);

  // --- the chord bar ---
  const bar = document.createElement("div");
  bar.id = "transport-bar";
  bar.style.cssText =
    "position:fixed;bottom:14px;left:50%;transform:translateX(-50%);display:flex;" +
    "align-items:center;gap:6px;padding:6px;background:rgba(255,255,255,.94);" +
    "border-radius:12px;box-shadow:var(--ot-shadow);z-index:10";

  const modeBtns = new Map<number, HTMLButtonElement>();
  for (const m of MODES) {
    const b = bigModeButton(m, () => game.setTransport(m.id));
    modeBtns.set(m.id, b);
    bar.appendChild(b);
  }

  const sep = () => {
    const s = document.createElement("span");
    s.style.cssText = "width:1px;align-self:stretch;background:#e2e5e9;margin:0 4px";
    return s;
  };
  bar.appendChild(sep());

  const runBtn = button("▶ Run", "mode-toggle", () =>
    game.setMode(game.mode === "build" ? "run" : "build"),
  );
  bar.appendChild(runBtn);
  bar.appendChild(sep());

  let speed = 1;
  const speeds: [number, string][] = [
    [1, "1×"],
    [10, "10×"],
    [100, "max"],
  ];
  const speedBtns = new Map<number, HTMLButtonElement>();
  for (const [mult, label] of speeds) {
    const b = button(label, `speed-${mult}`, () => {
      speed = mult;
      loop.setSpeed(mult);
      syncSpeed();
    });
    speedBtns.set(mult, b);
    bar.appendChild(b);
  }
  const syncSpeed = () => {
    for (const [mult, b] of speedBtns) {
      const on = speed === mult;
      b.style.background = on ? "#1c2024" : "#fff";
      b.style.color = on ? "#fff" : "#1c2024";
    }
  };
  syncSpeed();

  bar.appendChild(sep());

  const demandBtn = button("🌡 Demand", "layer-demand", () => game.setShowDemand(!game.showDemand));
  bar.appendChild(demandBtn);

  const settings = mountSettings(host, game);
  const gearBtn = button("⚙", "open-settings", () => settings.toggle());
  bar.appendChild(gearBtn);

  // --- sync ---
  const sync = () => {
    for (const m of MODES) {
      const b = modeBtns.get(m.id)!;
      const enabled = game.enabledModes.has(m.id);
      const active = game.transport === m.id;
      b.disabled = !enabled;
      b.style.opacity = enabled ? "1" : "0.35";
      b.style.cursor = enabled ? "pointer" : "not-allowed";
      b.style.borderColor = active && enabled ? m.color : "#d7dade";
      b.style.background = active && enabled ? m.color : "#fff";
      b.style.color = active && enabled ? "#fff" : "#1c2024";
      const kbd = b.querySelector("kbd");
      if (kbd) kbd.setAttribute("style", `font:600 10px system-ui;border-radius:4px;padding:0 4px;border:1px solid ${active && enabled ? "rgba(255,255,255,.5)" : "#d7dade"};color:${active && enabled ? "#fff" : "#9aa3ad"}`);
    }

    for (const [t, b] of toolBtns) {
      const on = game.tool === t;
      b.style.background = on ? "#1c2024" : "#fff";
      b.style.color = on ? "#fff" : "#1c2024";
      b.style.borderColor = on ? "#1c2024" : "#d7dade";
    }

    // The build-controls popover is shown only while building (run mode hides it).
    const m = MODES.find((x) => x.id === game.transport) ?? MODES[0];
    popover.style.display = game.mode === "build" ? "flex" : "none";
    popHead.innerHTML =
      `<span style="font-size:18px">${m.icon}</span>` +
      `<b style="font:600 14px system-ui;color:${m.color}">${m.name}</b>` +
      `<span style="color:#9aa3ad;font:12px system-ui">construction</span>`;
    popHint.textContent = m.hint;

    runBtn.textContent = game.mode === "run" ? "⏸ Build" : "▶ Run";
    runBtn.style.background = game.mode === "run" ? "#009e73" : "#fff";
    runBtn.style.color = game.mode === "run" ? "#fff" : "#1c2024";

    demandBtn.style.background = game.showDemand ? "#0072b2" : "#fff";
    demandBtn.style.color = game.showDemand ? "#fff" : "#1c2024";
  };
  game.onChange.push(sync);
  sync();

  // Keyboard chords: 1–4 select modes; B/R toggles build↔run (ignored while typing).
  window.addEventListener("keydown", (e) => {
    const tag = (e.target as HTMLElement | null)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
    const m = MODES.find((x) => x.key === e.key);
    if (m) {
      game.setTransport(m.id);
    } else if (e.key === "r" || e.key === "R") {
      game.setMode(game.mode === "build" ? "run" : "build");
    }
  });

  host.appendChild(popover);
  host.appendChild(bar);
}
