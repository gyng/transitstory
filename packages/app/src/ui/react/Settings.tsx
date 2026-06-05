// Settings panel (⚙): toggle which transport modes are available, and switch the economy
// (capital + fares) on/off. Mode toggles are a frontend gate — they grey out the chorded
// bar's buttons; the economy toggle emits SetEconomy through Game. Opens as a small panel
// anchored above the bar; reads Game/Stats state and re-renders on its hook slices.
import { useState } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { MODES } from "./shared";

// A token-style switch: 38×22 track + sliding knob (mirrors the vanilla toggleRow look).
function Toggle({
  label,
  testid,
  on,
  onToggle,
}: {
  label: string;
  testid: string;
  on: boolean;
  onToggle: () => void;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 8,
        padding: "5px 0",
        cursor: "pointer",
      }}
    >
      <span>{label}</span>
      <button
        data-testid={testid}
        onClick={(e) => {
          e.preventDefault();
          onToggle();
        }}
        style={{
          width: 38,
          height: 22,
          borderRadius: 11,
          border: 0,
          cursor: "pointer",
          position: "relative",
          transition: "background .12s",
          background: on ? "#009e73" : "#c4cad0",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            width: 18,
            height: 18,
            borderRadius: "50%",
            background: "#fff",
            transition: "left .12s",
            boxShadow: "0 1px 2px rgba(0,0,0,.3)",
            left: on ? 18 : 2,
          }}
        />
      </button>
    </label>
  );
}

export function Settings({ open }: { open: boolean; onClose: () => void }) {
  const game = useGame();
  const ui = useGameUI();
  const stats = useStats();
  // Demand model is tracked on Game (no sim-stats field); the toggle is its only mutator.
  const [agentDemand, setAgentDemand] = useState(game.agentDemand);

  if (!open) return null;

  const enabled = new Set(ui.enabledModes);

  return (
    <div
      id="settings-panel"
      data-testid="settings-panel"
      style={{
        position: "fixed",
        bottom: 84,
        right: 14,
        width: 240,
        padding: 14,
        display: "block",
        background: "rgba(255,255,255,.98)",
        borderRadius: 12,
        boxShadow: "var(--ot-shadow)",
        zIndex: 11,
        font: "13px system-ui,sans-serif",
        color: "#1c2024",
      }}
    >
      <div style={{ fontWeight: 700, marginBottom: 6 }}>Settings</div>
      <div style={{ color: "#7a818a", fontSize: 11, marginBottom: 6 }}>Transport modes</div>

      {MODES.map((m) => (
        <Toggle
          key={m.id}
          label={`${m.icon}  ${m.name}`}
          testid={`setting-mode-${m.id}`}
          on={enabled.has(m.id)}
          onToggle={() => game.setModeEnabled(m.id, !enabled.has(m.id))}
        />
      ))}

      <div
        style={{
          color: "#7a818a",
          fontSize: 11,
          margin: "10px 0 4px",
          borderTop: "1px solid #eceef1",
          paddingTop: 10,
        }}
      >
        Economy
      </div>

      <Toggle
        label="💰  Capital & fares"
        testid="setting-economy"
        on={stats.economyEnabled}
        onToggle={() => game.setEconomy(!stats.economyEnabled)}
      />

      <div style={{ color: "#7a818a", fontSize: 11, margin: "10px 0 4px", borderTop: "1px solid #eceef1", paddingTop: 10 }}>
        Demand model
      </div>
      <Toggle
        label="🧍  Citizen agents"
        testid="setting-agents"
        on={agentDemand}
        onToggle={() => {
          const v = !agentDemand;
          setAgentDemand(v);
          game.setDemandMode(v);
        }}
      />
      <div style={{ color: "#9aa3ad", fontSize: 10, lineHeight: 1.3, marginTop: 2 }}>
        Trips come from a population with homes & jobs instead of gravity flow.
      </div>
    </div>
  );
}
