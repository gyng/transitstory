// Left LineListPanel (network roster) + right contextual EditorPanel (selected line's
// trainset + headway). Reads sim snapshots, emits Commands via Game. Headway slider fires
// one SetHeadway Command on `change` (drag-end); `input` only updates the live preview label.
import type { Game } from "../game";

const PANEL =
  "position:fixed;background:rgba(255,255,255,.96);border-radius:10px;" +
  "box-shadow:var(--ot-shadow);z-index:9;font:13px system-ui,sans-serif;color:#1c2024";

function hex(u: number): string {
  return "#" + (u & 0xffffff).toString(16).padStart(6, "0");
}

export function mountPanels(host: HTMLElement, game: Game): void {
  const list = document.createElement("div");
  list.id = "line-list";
  list.dataset.testid = "line-list";
  list.style.cssText = PANEL + ";top:56px;left:14px;width:200px;padding:10px;max-height:50vh;overflow:auto";

  const editor = document.createElement("div");
  editor.id = "editor-panel";
  editor.dataset.testid = "editor-panel";
  editor.style.cssText = PANEL + ";top:56px;right:14px;width:230px;padding:12px;display:none";

  function renderList(): void {
    const lines = game.bridge.stats().perLine;
    list.innerHTML = `<div style="font-weight:700;margin-bottom:6px">Lines</div>`;
    if (lines.length === 0) {
      list.innerHTML += `<div style="color:#7a818a">No lines yet — draw one with the ╱ Line tool.</div>`;
      return;
    }
    for (const l of lines) {
      const row = document.createElement("div");
      row.dataset.testid = `line-row-${l.lineId}`;
      const sel = game.selectedLine === l.lineId;
      row.style.cssText =
        "display:flex;align-items:center;gap:8px;padding:6px;border-radius:6px;cursor:pointer;" +
        (sel ? "background:#eef4fb" : "");
      row.innerHTML =
        `<span style="width:14px;height:14px;border-radius:50%;flex:0 0 auto;background:${hex(l.color)};` +
        `box-shadow:0 0 0 2px #fff,0 0 0 3px #d7dade"></span>` +
        `<span style="flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${l.name}">${l.name || `Line ${l.lineId + 1}`}</span>` +
        `<span data-testid="line-ridership-${l.lineId}" style="color:#7a818a">${Math.round(l.ridership)}</span>`;
      row.addEventListener("click", () => game.selectLine(l.lineId));
      list.appendChild(row);
    }
  }

  function renderEditor(): void {
    const id = game.selectedLine;
    if (id === null) {
      editor.style.display = "none";
      return;
    }
    const l = game.bridge.stats().perLine.find((x) => x.lineId === id);
    if (!l) {
      editor.style.display = "none";
      return;
    }
    editor.style.display = "block";
    const mins = Math.max(2, Math.min(20, Math.round(l.headwayMs / 60_000)));

    if (l.trains === 0) {
      editor.innerHTML =
        `<div style="font-weight:700;margin-bottom:8px">${l.name}</div>` +
        `<button data-testid="assign-trainset" style="width:100%;padding:8px;border:0;border-radius:7px;` +
        `background:#0072b2;color:#fff;font:600 13px system-ui;cursor:pointer">▶ Assign trainset</button>` +
        `<div style="color:#7a818a;margin-top:6px">Adds trains and auto-suggests a headway.</div>`;
      editor.querySelector<HTMLButtonElement>('[data-testid="assign-trainset"]')!
        .addEventListener("click", () => game.assignTrainset(id, 2));
      return;
    }

    editor.innerHTML =
      `<div style="font-weight:700;margin-bottom:8px">${l.name}</div>` +
      `<label style="display:flex;justify-content:space-between;align-items:center;margin:6px 0">` +
      `Trains <input data-testid="trains-input" type="number" min="1" max="8" value="${l.trains}" ` +
      `style="width:56px;padding:4px"></label>` +
      `<label style="display:block;margin-top:10px">Headway: ` +
      `<b data-testid="headway-label">${mins} min</b></label>` +
      `<input data-testid="headway-slider" type="range" min="2" max="20" step="1" value="${mins}" ` +
      `style="width:100%">` +
      `<div style="color:#7a818a;margin:4px 0 10px">Capacity × frequency are your two levers.</div>`;

    // Track mode (whole line): Surface / Elevated / Tunnel — the built-environment lever.
    const lv = game.bridge.linesView().find((x) => x.id === id);
    const modes = lv?.spanModes ?? [];
    const allMode = modes.length && modes.every((m) => m === modes[0]) ? modes[0] : -1;
    const tight = lv && lv.minRadiusMm < 100_000;
    const modeRow = document.createElement("div");
    modeRow.innerHTML = `<div style="font-weight:600;margin-bottom:4px">Track</div>`;
    const btns = document.createElement("div");
    btns.style.cssText = "display:flex;gap:4px";
    (["Surface", "Elevated", "Tunnel"] as const).forEach((label, m) => {
      const b = document.createElement("button");
      b.textContent = label;
      b.dataset.testid = `mode-${m}`;
      const on = allMode === m;
      b.style.cssText =
        "flex:1;padding:5px;border-radius:6px;border:1px solid #d7dade;cursor:pointer;font:600 12px system-ui;" +
        (on ? "background:#0072b2;color:#fff" : "background:#fff;color:#1c2024");
      b.addEventListener("click", () => game.setLineMode(id, m));
      btns.appendChild(b);
    });
    modeRow.appendChild(btns);

    const impact = document.createElement("div");
    impact.dataset.testid = "line-impact";
    impact.style.cssText = "margin-top:8px;font-size:12px";
    const water = l.crossesWater
      ? `<div data-testid="water-warning" style="color:#d62828;font-weight:600;margin-bottom:4px">⚠ Surface track crosses water — Elevate or Tunnel.</div>`
      : "";
    const curve = tight ? `<div style="color:#e69f00">⤳ Tight curves — trains slow here.</div>` : "";
    impact.innerHTML =
      water + curve +
      `<div style="color:#7a818a">Build impact: <b>${Math.round(l.disruption)}</b></div>`;

    editor.appendChild(modeRow);
    editor.appendChild(impact);

    const trains = editor.querySelector<HTMLInputElement>('[data-testid="trains-input"]')!;
    trains.addEventListener("change", () =>
      game.assignTrainset(id, Math.max(1, Math.min(8, Number(trains.value) | 0))),
    );

    const slider = editor.querySelector<HTMLInputElement>('[data-testid="headway-slider"]')!;
    const label = editor.querySelector<HTMLElement>('[data-testid="headway-label"]')!;
    slider.addEventListener("input", () => (label.textContent = `${slider.value} min`)); // preview only
    slider.addEventListener("change", () => game.setHeadwayMs(id, Number(slider.value) * 60_000)); // commit
  }

  const sync = () => {
    renderList();
    renderEditor();
  };
  game.onChange.push(sync);
  sync();

  host.appendChild(list);
  host.appendChild(editor);
}
