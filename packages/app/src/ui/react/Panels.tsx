// Left LineListPanel (network roster) + right contextual EditorPanel (selected line's
// trainset + headway + track mode). Reads sim snapshots via hooks, emits Commands via Game.
// Headway slider fires one SetHeadway Command on `change` (drag-end); `input` only updates the
// live preview label (local useState) — never an optimistic self-render of sim state.
import { useCallback, useState } from "react";
import type { CSSProperties } from "react";
import type { PerLine } from "../../types";
import { useGame, useGameUI, useStats } from "./GameContext";
import { PANEL_STYLE, hex, modeIcon } from "./shared";

// cssText token → React style object, so PANEL_STYLE stays the single source of truth (no
// duplicated literals) while remaining cssText-equivalent in JSX.
function cssToStyle(css: string): CSSProperties {
  const out: Record<string, string> = {};
  for (const decl of css.split(";")) {
    const i = decl.indexOf(":");
    if (i < 0) continue;
    const prop = decl.slice(0, i).trim();
    const val = decl.slice(i + 1).trim();
    if (!prop) continue;
    const camel = prop.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());
    out[camel] = val;
  }
  return out as CSSProperties;
}

const LIST_STYLE: CSSProperties = {
  ...cssToStyle(PANEL_STYLE),
  top: "56px",
  left: "14px",
  width: "200px",
  padding: "10px",
  maxHeight: "50vh",
  overflow: "auto",
};

const EDITOR_STYLE: CSSProperties = {
  ...cssToStyle(PANEL_STYLE),
  top: "56px",
  right: "14px",
  width: "230px",
  padding: "12px",
};

function LineRow({ l, selected }: { l: PerLine; selected: boolean }) {
  const game = useGame();
  return (
    <div
      data-testid={`line-row-${l.lineId}`}
      onClick={() => game.selectLine(l.lineId)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "8px",
        padding: "6px",
        borderRadius: "6px",
        cursor: "pointer",
        ...(selected ? { background: "#eef4fb" } : null),
      }}
    >
      <span
        style={{
          width: "14px",
          height: "14px",
          borderRadius: "50%",
          flex: "0 0 auto",
          background: hex(l.color),
          boxShadow: "0 0 0 2px #fff,0 0 0 3px #d7dade",
        }}
      ></span>
      <span style={{ flex: "0 0 auto" }} title="mode">
        {modeIcon(l.mode)}
      </span>
      <span
        style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        title={l.name}
      >
        {l.name || `Line ${l.lineId + 1}`}
      </span>
      <span data-testid={`line-ridership-${l.lineId}`} style={{ color: "#7a818a" }}>
        {Math.round(l.ridership)}
      </span>
    </div>
  );
}

function LineList() {
  const ui = useGameUI();
  const lines = useStats().perLine;
  return (
    <div id="line-list" data-testid="line-list" style={LIST_STYLE}>
      <div style={{ fontWeight: 700, marginBottom: "6px" }}>Lines</div>
      {lines.length === 0 ? (
        <div style={{ color: "#7a818a" }}>No lines yet — draw one with the ╱ Line tool.</div>
      ) : (
        lines.map((l) => <LineRow key={l.lineId} l={l} selected={ui.selectedLine === l.lineId} />)
      )}
    </div>
  );
}

function Editor({ l }: { l: PerLine }) {
  const game = useGame();
  const id = l.lineId;
  const mins = Math.max(2, Math.min(20, Math.round(l.headwayMs / 60_000)));
  // Local preview of the headway slider — `input` updates only this label; `change` commits.
  const [previewMins, setPreviewMins] = useState<number | null>(null);

  // React's onChange maps to the DOM `input` event, which would commit a Command on every drag
  // tick. To preserve the vanilla `input`(preview) / `change`(drag-end commit) split exactly, we
  // attach real native listeners via callback refs (mirrors panels.ts addEventListener 1:1).
  const sliderRef = useCallback(
    (el: HTMLInputElement | null) => {
      if (!el) return;
      el.oninput = () => setPreviewMins(Number(el.value)); // preview only
      el.onchange = () => game.setHeadwayMs(id, Number(el.value) * 60_000); // commit
    },
    [game, id],
  );
  const trainsRef = useCallback(
    (el: HTMLInputElement | null) => {
      if (!el) return;
      el.onchange = () => game.assignTrainset(id, Math.max(1, Math.min(8, Number(el.value) | 0)));
    },
    [game, id],
  );

  // Track mode (whole line): Surface / Elevated / Tunnel — the built-environment lever.
  // Only rail builds track; bus rides roads, ferry/air have no grade to choose.
  const lv = game.bridge.linesView().find((x) => x.id === id);
  const modes = lv?.spanModes ?? [];
  const allMode = modes.length && modes.every((m) => m === modes[0]) ? modes[0] : -1;
  const tight = lv ? lv.minRadiusMm < 100_000 : false;
  const isRail = l.mode === 0;

  const header = (
    <div style={{ fontWeight: 700, marginBottom: "8px" }}>
      {modeIcon(l.mode)} {l.name}
    </div>
  );

  if (l.trains === 0) {
    return (
      <div id="editor-panel" data-testid="editor-panel" style={EDITOR_STYLE}>
        {header}
        <button
          data-testid="assign-trainset"
          onClick={() => game.assignTrainset(id, 2)}
          style={{
            width: "100%",
            padding: "8px",
            border: 0,
            borderRadius: "7px",
            background: "#0072b2",
            color: "#fff",
            font: "600 13px system-ui",
            cursor: "pointer",
          }}
        >
          ▶ Assign trainset
        </button>
        <div style={{ color: "#7a818a", marginTop: "6px" }}>Adds trains and auto-suggests a headway.</div>
      </div>
    );
  }

  const shownMins = previewMins ?? mins;

  return (
    <div id="editor-panel" data-testid="editor-panel" style={EDITOR_STYLE}>
      {header}
      <label style={{ display: "flex", justifyContent: "space-between", alignItems: "center", margin: "6px 0" }}>
        Trains{" "}
        <input
          key={l.trains}
          ref={trainsRef}
          data-testid="trains-input"
          type="number"
          min="1"
          max="8"
          defaultValue={l.trains}
          style={{ width: "56px", padding: "4px" }}
        />
      </label>
      <label style={{ display: "block", marginTop: "10px" }}>
        Headway: <b data-testid="headway-label">{shownMins} min</b>
      </label>
      <input
        key={mins}
        ref={sliderRef}
        data-testid="headway-slider"
        type="range"
        min="2"
        max="20"
        step="1"
        defaultValue={mins}
        style={{ width: "100%" }}
      />
      <div style={{ color: "#7a818a", margin: "4px 0 10px" }}>Capacity × frequency are your two levers.</div>

      {isRail && (
        <div>
          <div style={{ fontWeight: 600, marginBottom: "4px" }}>Track</div>
          <div style={{ display: "flex", gap: "4px" }}>
            {(["Surface", "Elevated", "Tunnel"] as const).map((label, m) => {
              const on = allMode === m;
              return (
                <button
                  key={m}
                  data-testid={`mode-${m}`}
                  onClick={() => game.setLineMode(id, m)}
                  style={{
                    flex: 1,
                    padding: "5px",
                    borderRadius: "6px",
                    border: "1px solid #d7dade",
                    cursor: "pointer",
                    font: "600 12px system-ui",
                    ...(on ? { background: "#0072b2", color: "#fff" } : { background: "#fff", color: "#1c2024" }),
                  }}
                >
                  {label}
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div data-testid="line-impact" style={{ marginTop: "8px", fontSize: "12px" }}>
        {l.crossesWater && (
          <div data-testid="water-warning" style={{ color: "#d62828", fontWeight: 600, marginBottom: "4px" }}>
            ⚠ Surface track crosses water — Elevate or Tunnel.
          </div>
        )}
        {tight && <div style={{ color: "#e69f00" }}>⤳ Tight curves — trains slow here.</div>}
        <div style={{ color: "#7a818a" }}>
          Build impact: <b>{Math.round(l.disruption)}</b> · Cost: <b>${Math.round(l.capitalCost / 1e6)}M</b>
        </div>
      </div>
    </div>
  );
}

export function Panels() {
  const ui = useGameUI();
  const perLine = useStats().perLine;
  const id = ui.selectedLine;
  const l = id === null ? undefined : perLine.find((x) => x.lineId === id);
  return (
    <>
      <LineList />
      {l ? <Editor key={l.lineId} l={l} /> : null}
    </>
  );
}
