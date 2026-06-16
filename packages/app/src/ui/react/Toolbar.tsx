// Chorded bottom bar: big transport-mode buttons (1 Rail / 2 Bus / 3 Ferry / 4 Plane /
// 5 Heavy Rail) drive construction; selecting one opens its build controls in a popover ABOVE
// the bar. Right of the modes: Run/Build, speed, the Demand map-layer toggle, and Settings.
// Keyboard 1–5 chord the modes. Emits to Game / GameLoop only (never mutates sim state directly).
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import type { Tool } from "../../game";
import { useGame, useGameUI, useLoop, useStats } from "./GameContext";
import { MODES, type ModeDef } from "./shared";
import { techUnlocked } from "../../commands/codec";
import { Settings } from "./Settings";
import { BuildHud } from "./BuildHud";

// Per-tool controls hint shown in the build popover (the #4 "how to cancel" tooltip).
const TOOL_HINT: Record<Tool, string> = {
  station: "[T] Click to place — stays armed for the next one · Esc when done · right-click to inspect",
  line: "Click stations to chain · double-click to build · ⌫ undo · Esc / right-click to cancel the draft",
  select: "[V] Click — or right-click anything — to inspect it",
  bulldozer: "[X] Click a station or line to demolish it · Esc to stop · right-click to inspect",
  barracks: "[B] Click to place a barracks — it fields legions once supplied · Esc when done",
  bounty: "[Y] Click a town to post a bounty — baits AI legions to attack it · Esc when done",
};

const TOOLS: [Tool, string][] = [
  ["station", "◉ Stations"],
  ["line", "╱ Draw line"],
  ["select", "▣ Select"],
  ["bulldozer", "💥 Bulldoze"],
];

// Fantasy (arcadia) build tools: a barracks (fields legions) + bounties (steer them, Majesty-style).
const FANTASY_TOOLS: [Tool, string][] = [
  ["barracks", "🏰 Barracks"],
  ["bounty", "⚑ Bounty"],
];

// A doubling gear ladder (1×→8×) for fine control, plus a max fast-forward. The sim is a GameLoop
// knob (loop.setSpeed), never a Command — speed never touches sim state.
const SPEEDS: [number, string][] = [
  [1, "1×"],
  [2, "2×"],
  [4, "4×"],
  [8, "8×"],
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
  title,
  disabled,
}: {
  label: string;
  testid: string;
  onClick: () => void;
  style?: CSSProperties;
  title?: string;
  disabled?: boolean;
}) {
  return (
    <button
      data-testid={testid}
      onClick={disabled ? undefined : onClick}
      title={title}
      disabled={disabled}
      style={{
        border: "1px solid #d7dade",
        background: "#fff",
        color: "#1c2024",
        borderRadius: 7,
        padding: "6px 10px",
        font: "600 13px system-ui,sans-serif",
        cursor: disabled ? "default" : "pointer",
        ...style,
        ...(disabled ? { opacity: 0.4, background: "#eef0f2", color: "#9aa3ad" } : null),
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
  const stats = useStats();

  const [speed, setSpeed] = useState(1);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // The keydown handler reads the live speed via a ref so the single window listener stays stable across
  // speed changes (no add/remove churn that would also re-bind the co-located tool/Space handlers).
  const speedRef = useRef(speed);
  speedRef.current = speed;

  // Game keyboard: 1–5 chord transport modes; R / Space toggle Build↔Run; T/V/X (+ arcadia B/Y) arm the
  // build tools; ',' / '.' step the speed ladder. (WASD/arrows/Q/E camera nav live in App.tsx.) Ignored
  // while typing in a field and for ctrl/meta/alt chords (so Ctrl-Z etc. pass through).
  useEffect(() => {
    const setSpd = (mult: number) => {
      setSpeed(mult);
      loop.setSpeed(mult);
    };
    const stepSpeed = (dir: number) => {
      const i = SPEEDS.findIndex(([v]) => v === speedRef.current);
      const ni = Math.max(0, Math.min(SPEEDS.length - 1, (i < 0 ? 0 : i) + dir));
      setSpd(SPEEDS[ni][0]);
    };
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      // Space = pause/resume. Blur the focused element first so a focused Run/Build pill can't ALSO fire on
      // the keyup-as-click (single toggle by construction, not by listener ordering).
      if (e.code === "Space") {
        e.preventDefault();
        (document.activeElement as HTMLElement | null)?.blur?.();
        game.setMode(game.mode === "build" ? "run" : "build");
        return;
      }
      if (e.key === ",") { stepSpeed(-1); return; }
      if (e.key === ".") { stepSpeed(1); return; }
      const m = MODES.find((x) => x.key === e.key);
      if (m) { game.setTransport(m.id); return; }
      if (e.key === "r" || e.key === "R") {
        game.setMode(game.mode === "build" ? "run" : "build");
        return;
      }
      // Build-tool hotkeys — letters chosen disjoint from the WASD/Q/E camera keys (T=sTation, V=select,
      // X=bulldoze; arcadia adds B=Barracks, Y=bountY). Arming a tool in Run flips to Build first so it
      // isn't silently inert behind the build-gated pointer/popover.
      const lk = e.key.toLowerCase();
      const TOOL_KEYS: Record<string, Tool> = { t: "station", v: "select", x: "bulldozer" };
      if (ui.ruleset === "arcadia") { TOOL_KEYS.b = "barracks"; TOOL_KEYS.y = "bounty"; }
      const tool = TOOL_KEYS[lk];
      if (tool) {
        if (game.mode === "run") game.setMode("build");
        game.setTool(tool);
        return;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [game, loop, ui.ruleset]);

  const enabled = new Set(ui.enabledModes);
  // S11 RAIL-GATE: arcadia builds RAIL only; Heavy Rail (mode 4) appears once its tech is unlocked.
  // Transit shows every mode. (The sim enforces this regardless; this is the matching chrome.)
  const HEAVY_RAIL_TECH = 5; // crates/sim/tech.rs HEAVY_RAIL id
  const visibleModes =
    ui.ruleset === "arcadia"
      ? MODES.filter((m) => m.id === 0 || (m.id === 4 && techUnlocked(stats.techUnlocked, HEAVY_RAIL_TECH)))
      : MODES;
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
        {/* live route readout (stops · length · validity) while drawing */}
        <BuildHud />
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
            {[...TOOLS, ...(ui.ruleset === "arcadia" ? FANTASY_TOOLS : [])].map(([t, label]) => {
              const on = ui.tool === t;
              const activeBg = t === "bulldozer" ? "#d62828" : "#1c2024"; // bulldozer reads destructive
              return (
                <Button
                  key={t}
                  label={label}
                  testid={`tool-${t}`}
                  onClick={() => game.setTool(t)}
                  style={{
                    flex: 1,
                    background: on ? activeBg : "#fff",
                    color: on ? "#fff" : "#1c2024",
                    borderColor: on ? activeBg : "#d7dade",
                  }}
                />
              );
            })}
          </div>
          <div style={{ color: "#9aa3ad", font: "11px system-ui,sans-serif" }}>{TOOL_HINT[ui.tool]}</div>
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
          {visibleModes.map((m) => (
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
          disabled={game.lensHides("demand")}
          title={game.lensHides("demand") ? `Demand is hidden by the ${ui.lens} lens — switch to the All lens to show it.` : "Travel-demand heat: 🟧 warm = unserved (build here) · 🟦 cool = covered. Homes start trips, jobs pull them. Pin a station to see where its riders go."}
          onClick={() => game.setShowDemand(!ui.showDemand)}
          style={{
            background: ui.showDemand ? "#0072b2" : "#fff",
            color: ui.showDemand ? "#fff" : "#1c2024",
          }}
        />

        <Button
          label="🕐 Reach"
          testid="layer-reach"
          title="Reach: pin a station, then shade every other by how fast transit gets there. Faster reach pulls more demand — extend it to unlock trips."
          onClick={() => game.setShowReach(!ui.showReach)}
          style={{
            background: ui.showReach ? "#0072b2" : "#fff",
            color: ui.showReach ? "#fff" : "#1c2024",
          }}
        />

        <Button
          label="🛣 Roads"
          testid="layer-roads"
          title="Road corridors where buses run cheap + fast. Turn on when planning a bus line — route along roads to cut cost and speed service."
          onClick={() => game.setShowRoads(!ui.showRoads)}
          style={{
            background: ui.showRoads ? "#0072b2" : "#fff",
            color: ui.showRoads ? "#fff" : "#1c2024",
          }}
        />

        <Button
          label="🧍 Peeps"
          testid="layer-peeps"
          title="Show individual riders: walking to the platform, waiting, riding the train, and heading out at their stop. Purely visual — no effect on the sim."
          onClick={() => game.setShowPeeps(!ui.showPeeps)}
          style={{
            background: ui.showPeeps ? "#0072b2" : "#fff",
            color: ui.showPeeps ? "#fff" : "#1c2024",
          }}
        />

        {/* TTD signals (fantasy single-track): show each block's state so meets read at a glance. */}
        {ui.ruleset === "arcadia" && (
          <Button
            label="🚦 Signals"
            testid="layer-signals"
            title="Signal view: single-track block state — 🟢 clear · 🔴 occupied · 🟠 a cart held, waiting for the block ahead. Purely visual — shows WHY carts meet and wait on single track."
            onClick={() => game.setShowSignals(!ui.showSignals)}
            style={{
              background: ui.showSignals ? "#0072b2" : "#fff",
              color: ui.showSignals ? "#fff" : "#1c2024",
            }}
          />
        )}

        {/* Map LENSES (#5): an EXCLUSIVE arcadia-only view-mode selector — pick ONE reading; the others dim
            (filtered in Game.composeAndSet). Styled as a SEGMENTED control (joined buttons, radio-like) so it
            reads as one-of-N — visually distinct from the additive layer-toggle pills above. */}
        {ui.ruleset === "arcadia" && (
          <>
            <span style={SEP_STYLE} />
            <span style={{ font: "700 10px system-ui", letterSpacing: ".06em", color: "#8a93a3", alignSelf: "center" }}>LENS</span>
            <div data-testid="lens-bar" style={{ display: "flex" }}>
              {([
                ["realm", "◉", "All", "everything"],
                ["supply", "⛏", "Supply", "sources, towns, rivers — your economy"],
                ["military", "⚔", "War", "legions, raiders, conquest targets"],
                ["decadence", "☠", "Rot", "the creeping rot — the tide + its front"],
              ] as const).map(([id, icon, lbl, title], i, arr) => (
                <button
                  key={id}
                  data-testid={`lens-${id}`}
                  title={`Lens: ${title}`}
                  onClick={() => game.setLens(id)}
                  style={{
                    border: "1px solid #d7dade",
                    borderLeft: i === 0 ? "1px solid #d7dade" : "none",
                    borderTopLeftRadius: i === 0 ? 6 : 0,
                    borderBottomLeftRadius: i === 0 ? 6 : 0,
                    borderTopRightRadius: i === arr.length - 1 ? 6 : 0,
                    borderBottomRightRadius: i === arr.length - 1 ? 6 : 0,
                    padding: "6px 8px",
                    cursor: "pointer",
                    font: "600 12px system-ui,sans-serif",
                    background: ui.lens === id ? "#1c2024" : "#fff",
                    color: ui.lens === id ? "#fff" : "#1c2024",
                  }}
                >
                  {icon} {lbl}
                </button>
              ))}
            </div>
          </>
        )}

        <Button label="⚙" testid="open-settings" onClick={() => setSettingsOpen((o) => !o)} />
        </div>
      </div>

      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </>
  );
}
