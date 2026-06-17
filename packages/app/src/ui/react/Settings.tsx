// Settings panel (⚙): toggle which transport modes are available, and switch the economy
// (capital + fares) on/off. Mode toggles are a frontend gate — they grey out the chorded
// bar's buttons; the economy toggle emits SetEconomy through Game. Opens as a small panel
// anchored above the bar; reads Game/Stats state and re-renders on its hook slices.
import { useState } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { MODES } from "./shared";
import { audio } from "../../fx/audio";

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
          // ON = lit go-green (semantic); OFF = a recessed dark well in the console face.
          background: on ? "var(--ot-con-green)" : "#14171c",
          boxShadow: on ? "0 0 8px var(--ot-con-green)" : "var(--ot-well)",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            width: 18,
            height: 18,
            borderRadius: "50%",
            background: "#e6ebf2",
            transition: "left .12s",
            boxShadow: "0 1px 2px rgba(0,0,0,.5)",
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
  // Sound is owned by the audio kit (persisted in localStorage); mirror it in local state.
  const [soundOn, setSoundOn] = useState(!audio.muted);
  // Day/night map tint (default on); the sky module owns the actual divs.
  const [dayNight, setDayNight] = useState(true);

  if (!open) return null;

  const enabled = new Set(ui.enabledModes);

  return (
    <div
      id="settings-panel"
      data-testid="settings-panel"
      className="ot-console"
      style={{
        // Anchored near the bottom-LEFT ⚙ trigger in the CornerCluster (stage 8 — it was disconnected
        // bottom-right). Opens upward from just above the corner cluster so the ⚙ and its panel read as
        // one control. Capped height with scroll so it never runs off the top on a short viewport.
        position: "fixed",
        bottom: 60,
        left: 14,
        width: 240,
        maxHeight: "calc(100vh - 80px)",
        overflowY: "auto",
        padding: 14,
        display: "block",
        zIndex: 11,
        font: "13px system-ui,sans-serif",
      }}
    >
      <div style={{ fontWeight: 700, marginBottom: 6, color: "var(--ot-con-ink)" }}>Settings</div>
      <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, marginBottom: 6 }}>Transport modes</div>

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
          color: "var(--ot-con-ink-dim)",
          fontSize: 11,
          margin: "10px 0 4px",
          borderTop: "1px solid rgba(255,255,255,.08)",
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

      <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, margin: "10px 0 4px", borderTop: "1px solid rgba(255,255,255,.08)", paddingTop: 10 }}>
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
      <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 10, lineHeight: 1.3, marginTop: 2 }}>
        Trips come from a population with homes & jobs instead of gravity flow.
      </div>

      <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, margin: "10px 0 4px", borderTop: "1px solid rgba(255,255,255,.08)", paddingTop: 10 }}>
        Display
      </div>
      <Toggle
        label="🌗  Day / night tint"
        testid="setting-daynight"
        on={dayNight}
        onToggle={() => {
          const on = !dayNight;
          setDayNight(on);
          game.sky.setEnabled(on);
          if (on) game.sky.set(stats.simHour); // re-apply now, don't wait for the next 3 Hz tick
        }}
      />

      <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, margin: "10px 0 4px", borderTop: "1px solid rgba(255,255,255,.08)", paddingTop: 10 }}>
        Sound
      </div>
      <Toggle
        label="🔊  Sound effects"
        testid="setting-sound"
        on={soundOn}
        onToggle={() => {
          const on = !soundOn;
          setSoundOn(on);
          audio.unlock(); // this click is a user gesture — start/resume the context
          audio.setMuted(!on);
          if (on) audio.tick(); // immediate confirmation that sound is back
        }}
      />
    </div>
  );
}
