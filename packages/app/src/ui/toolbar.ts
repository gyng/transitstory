// Bottom transport bar: tool selection (Select / Station / Line) + Build/Run toggle.
// Emits to Game (never mutates sim state). Mirrors Game state via the onChange hook.
import type { Game, Tool } from "../game";

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

export function mountToolbar(host: HTMLElement, game: Game): void {
  const bar = document.createElement("div");
  bar.id = "transport-bar";
  bar.style.cssText =
    "position:fixed;bottom:14px;left:50%;transform:translateX(-50%);display:flex;" +
    "align-items:center;gap:6px;padding:6px;background:rgba(255,255,255,.94);" +
    "border-radius:10px;box-shadow:var(--ot-shadow);z-index:10";

  const tools: [Tool, string][] = [
    ["select", "▣ Select"],
    ["station", "◉ Station"],
    ["line", "╱ Line"],
  ];
  const toolBtns = new Map<Tool, HTMLButtonElement>();
  for (const [t, label] of tools) {
    const b = button(label, `tool-${t}`, () => game.setTool(t));
    toolBtns.set(t, b);
    bar.appendChild(b);
  }

  const sep = document.createElement("span");
  sep.style.cssText = "width:1px;align-self:stretch;background:#e2e5e9;margin:0 4px";
  bar.appendChild(sep);

  const runBtn = button("▶ Run", "mode-toggle", () =>
    game.setMode(game.mode === "build" ? "run" : "build"),
  );
  bar.appendChild(runBtn);

  const sync = () => {
    for (const [t, b] of toolBtns) {
      const on = game.tool === t;
      b.style.background = on ? "#0072b2" : "#fff";
      b.style.color = on ? "#fff" : "#1c2024";
    }
    runBtn.textContent = game.mode === "run" ? "⏸ Build" : "▶ Run";
    runBtn.style.background = game.mode === "run" ? "#009e73" : "#fff";
    runBtn.style.color = game.mode === "run" ? "#fff" : "#1c2024";
  };
  game.onChange.push(sync);
  sync();

  host.appendChild(bar);
}
