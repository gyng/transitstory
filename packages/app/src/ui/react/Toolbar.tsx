// Chorded bottom bar: four big transport-mode buttons (1 Rail / 2 Bus / 3 Ferry / 4 Plane)
// drive construction; selecting one opens its build controls in a popover ABOVE the bar.
// Right of the modes: Run/Build, speed, the Demand map-layer toggle, and Settings. Keyboard
// 1–4 chord the modes. Emits to Game / GameLoop only (never mutates sim state directly).
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import type { Tool } from "../../game";
import { useGame, useGameUI, useLoop } from "./GameContext";
import { MODES, type ModeDef } from "./shared";
import { Settings } from "./Settings";

const TOOLS: [Tool, string][] = [
  ["station", "◉ Stations"],
  ["line", "╱ Draw line"],
  ["select", "▣ Select"],
];

const SPEEDS: [number, string][] = [
  [1, "1×"],
  [10, "10×"],
  [100, "max"],
];

const SEP_STYLE: CSSProperties = {
  width: 1,
  alignSelf: "stretch",
  background: "#e2e5e9",
  margin: "0 4px",
};

// Plain pill button (the bar's Run/Build, speeds, Demand, gear, and the popover tools).
function Button({
  label,
  testid,
  onClick,
  style,
}: {
  label: string;
  testid: string;
  onClick: () => void;
  style?: CSSProperties;
}) {
  return (
    <button
      data-testid={testid}
      onClick={onClick}
      style={{
        border: "1px solid #d7dade",
        background: "#fff",
        color: "#1c2024",
        borderRadius: 7,
        padding: "6px 10px",
        font: "600 13px system-ui,sans-serif",
        cursor: "pointer",
        ...style,
      }}
    >
      {label}
    </button>
  );
}

function BigModeButton({
  m,
  active,
  enabled,
  onClick,
}: {
  m: ModeDef;
  active: boolean;
  enabled: boolean;
  onClick: () => void;
}) {
  const on = active && enabled;
  return (
    <button
      data-testid={`mode-transport-${m.id}`}
      disabled={!enabled}
      onClick={onClick}
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 3,
        minWidth: 64,
        padding: "8px 10px",
        border: `2px solid ${on ? m.color : "#d7dade"}`,
        background: on ? m.color : "#fff",
        color: on ? "#fff" : "#1c2024",
        borderRadius: 10,
        cursor: enabled ? "pointer" : "not-allowed",
        opacity: enabled ? 1 : 0.35,
      }}
    >
      <span style={{ fontSize: 20, lineHeight: 1 }}>{m.icon}</span>
      <span style={{ font: "600 13px system-ui,sans-serif" }}>{m.name}</span>
      <kbd
        style={{
          font: "600 10px system-ui",
          borderRadius: 4,
          padding: "0 4px",
          border: `1px solid ${on ? "rgba(255,255,255,.5)" : "#d7dade"}`,
          color: on ? "#fff" : "#9aa3ad",
        }}
      >
        {m.key}
      </kbd>
    </button>
  );
}

export function Toolbar() {
  const game = useGame();
  const loop = useLoop();
  const ui = useGameUI();

  const [speed, setSpeed] = useState(1);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Keyboard chords: 1–4 select modes; R toggles build↔run (ignored while typing).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      const m = MODES.find((x) => x.key === e.key);
      if (m) {
        game.setTransport(m.id);
      } else if (e.key === "r" || e.key === "R") {
        game.setMode(game.mode === "build" ? "run" : "build");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [game]);

  const enabled = new Set(ui.enabledModes);
  const activeMode = MODES.find((x) => x.id === ui.transport) ?? MODES[0];
  const running = ui.mode === "run";

  return (
    <>
      {/* Bottom-centred stack: the build-controls popover sits ABOVE the chord bar with a real
          gap, so they can never overlap regardless of bar height — the bar stays pinned at the
          bottom (this column is bottom-anchored) and the popover grows upward off it. The wrapper
          is pointer-transparent so map drags pass through the gaps; children re-enable pointers. */}
      <div
        style={{
          position: "fixed",
          bottom: 14,
          left: "50%",
          transform: "translateX(-50%)",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 10,
          maxWidth: "96vw",
          zIndex: 10,
          pointerEvents: "none",
        }}
      >
        {/* build-controls popover (opens above the bar for the active mode) */}
        <div
          id="mode-controls"
          data-testid="mode-controls"
          style={{
            display: ui.mode === "build" ? "flex" : "none",
            flexDirection: "column",
            gap: 8,
            padding: "12px 14px",
            width: "min(440px,92vw)",
            background: "rgba(255,255,255,.97)",
            borderRadius: 12,
            boxShadow: "var(--ot-shadow)",
            pointerEvents: "auto",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontSize: 18 }}>{activeMode.icon}</span>
            <b style={{ font: "600 14px system-ui", color: activeMode.color }}>{activeMode.name}</b>
            <span style={{ color: "#9aa3ad", font: "12px system-ui" }}>construction</span>
          </div>
          <div style={{ color: "#7a818a", font: "12px system-ui,sans-serif", lineHeight: 1.35 }}>
            {activeMode.hint}
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            {TOOLS.map(([t, label]) => {
              const on = ui.tool === t;
              return (
                <Button
                  key={t}
                  label={label}
                  testid={`tool-${t}`}
                  onClick={() => game.setTool(t)}
                  style={{
                    flex: 1,
                    background: on ? "#1c2024" : "#fff",
                    color: on ? "#fff" : "#1c2024",
                    borderColor: on ? "#1c2024" : "#d7dade",
                  }}
                />
              );
            })}
          </div>
        </div>

        {/* the chord bar (wraps on narrow viewports rather than overflowing) */}
        <div
          id="transport-bar"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexWrap: "wrap",
            gap: 6,
            padding: 6,
            maxWidth: "96vw",
            background: "rgba(255,255,255,.94)",
            borderRadius: 12,
            boxShadow: "var(--ot-shadow)",
            pointerEvents: "auto",
          }}
        >
          {MODES.map((m) => (
          <BigModeButton
            key={m.id}
            m={m}
            active={ui.transport === m.id}
            enabled={enabled.has(m.id)}
            onClick={() => game.setTransport(m.id)}
          />
        ))}

        <span style={SEP_STYLE} />

        <Button
          label={running ? "⏸ Build" : "▶ Run"}
          testid="mode-toggle"
          onClick={() => game.setMode(running ? "build" : "run")}
          style={{
            background: running ? "#009e73" : "#fff",
            color: running ? "#fff" : "#1c2024",
          }}
        />

        <span style={SEP_STYLE} />

        {SPEEDS.map(([mult, label]) => {
          const on = speed === mult;
          return (
            <Button
              key={mult}
              label={label}
              testid={`speed-${mult}`}
              onClick={() => {
                setSpeed(mult);
                loop.setSpeed(mult);
              }}
              style={{
                background: on ? "#1c2024" : "#fff",
                color: on ? "#fff" : "#1c2024",
              }}
            />
          );
        })}

        <span style={SEP_STYLE} />

        <Button
          label="🌡 Demand"
          testid="layer-demand"
          onClick={() => game.setShowDemand(!ui.showDemand)}
          style={{
            background: ui.showDemand ? "#0072b2" : "#fff",
            color: ui.showDemand ? "#fff" : "#1c2024",
          }}
        />

        <Button label="⚙" testid="open-settings" onClick={() => setSettingsOpen((o) => !o)} />
        </div>
      </div>

      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </>
  );
}
