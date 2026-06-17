// Chorded bottom bar: big transport-mode buttons (1 Rail / 2 Bus / 3 Ferry / 4 Plane /
// 5 Heavy Rail) drive construction; selecting one opens its build controls in a popover ABOVE
// the bar. Right of the modes: Run/Build, speed, the Demand map-layer toggle, and Settings.
// Keyboard 1–5 chord the modes. Emits to Game / GameLoop only (never mutates sim state directly).
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import type { Tool } from "../../game";
import { useGame, useGameUI, useStats } from "./GameContext";
import { MODES } from "./shared";
import { Button, BigModeButton } from "./keys";
import { techUnlocked } from "../../commands/codec";
import { Settings } from "./Settings";
import { BuildHud } from "./BuildHud";

// Per-tool controls hint shown in the build popover (the #4 "how to cancel" tooltip).
// TTD L6 (track + services): TRACK lays bare grey rail (infrastructure, no trains); SERVICE draws the same
// way but lands a live coloured line (auto-stocked). Draw several services over the same stations to run
// many services on ONE shared corridor.
const TOOL_HINT: Record<Tool, string> = {
  line: "[T] Lay TRACK: click stations to chain bare rail (drops new ones as you go) · double-click to build · ⌫ undo · Esc to cancel. Stays grey until a service runs on it.",
  service: "[R] Run a SERVICE: click stations to route a coloured line (over track or fresh) · double-click to build · ⌫ undo · Esc to cancel. Lands with trains already running — tune them in the panel.",
  station: "[N] Click to drop a station — stays armed for the next · the Track/Service tools also drop them while chaining · Esc when done",
  select: "[V] Click — or right-click anything — to inspect it. Click bare grey track to assign it trains.",
  bulldozer: "[X] Click a station or line to demolish it · Esc to stop · right-click to inspect",
  barracks: "[B] Click to place a barracks — it fields legions once supplied · Esc when done",
  bounty: "[Y] Click a town to post a bounty — steers the NEXT legions fielded toward it (already-marching legions keep their target) · Esc when done",
};

const TOOLS: [Tool, string][] = [
  ["line", "╱ Track"],
  ["service", "🚆 Service"],
  ["station", "◉ Station"],
  ["select", "▣ Select"],
  ["bulldozer", "💥 Bulldoze"],
];

// Fantasy (arcadia) build tools: a barracks (fields legions) + bounties (steer them, Majesty-style).
const FANTASY_TOOLS: [Tool, string][] = [
  ["barracks", "🏰 Barracks"],
  ["bounty", "⚑ Bounty"],
];

// A cut seam between key groups on the console face (a dark groove with a faint top-light).
const SEP_STYLE: CSSProperties = {
  width: 2,
  alignSelf: "stretch",
  background: "linear-gradient(180deg, rgba(0,0,0,.45), rgba(255,255,255,.05))",
  borderRadius: 1,
  margin: "2px 5px",
};

export function Toolbar() {
  const game = useGame();
  const ui = useGameUI();
  const stats = useStats();

  const [settingsOpen, setSettingsOpen] = useState(false);

  // Game keyboard: 1–5 chord transport modes; Space toggles Build↔Run; T/R/N/V/X (+ arcadia B/Y) arm the
  // build tools (Track/Service/Station/Select/Bulldoze). The speed ladder's ',' / '.' keys moved with the
  // speed buttons to TimeCluster. (WASD/arrows/Q/E camera nav live in App.tsx.) Ignored while typing in a
  // field and for ctrl/meta/alt chords (Ctrl-Z etc.).
  useEffect(() => {
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
      const m = MODES.find((x) => x.key === e.key);
      if (m) { game.setTransport(m.id); return; }
      // Build-tool hotkeys — letters disjoint from the WASD/Q/E camera keys. TTD L6: T=Track, R=Service
      // (Route), N=statioN, V=select, X=bulldoze; arcadia adds B=Barracks, Y=bountY. (Run/Build is Space —
      // R no longer aliases it, so it can route a service.) Arming a tool in Run flips to Build first so it
      // isn't silently inert behind the build-gated pointer/popover.
      const lk = e.key.toLowerCase();
      const TOOL_KEYS: Record<string, Tool> = { t: "line", r: "service", n: "station", v: "select", x: "bulldozer" };
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
  }, [game, ui.ruleset]);

  const enabled = new Set(ui.enabledModes);
  // S11 RAIL-GATE: arcadia builds RAIL only; Heavy Rail (mode 4) appears once its tech is unlocked.
  // Transit shows every mode. (The sim enforces this regardless; this is the matching chrome.)
  const HEAVY_RAIL_TECH = 5; // crates/sim/tech.rs HEAVY_RAIL id
  const visibleModes =
    ui.ruleset === "arcadia"
      ? MODES.filter((m) => m.id === 0 || (m.id === 4 && techUnlocked(stats.techUnlocked, HEAVY_RAIL_TECH)))
      : MODES;
  const activeMode = MODES.find((x) => x.id === ui.transport) ?? MODES[0];

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
        {/* build-controls popover (opens above the bar for the active mode). The `mode-controls`
            testid moved to TimeCluster (stage 2); this popover keeps its own id. */}
        <div
          id="build-controls"
          className="ot-console"
          style={{
            display: ui.mode === "build" ? "flex" : "none",
            flexDirection: "column",
            gap: 8,
            padding: "12px 14px",
            width: "min(440px,92vw)",
            pointerEvents: "auto",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ fontSize: 18 }}>{activeMode.icon}</span>
            <b style={{ font: "700 14px system-ui", color: activeMode.color, textShadow: "0 1px 2px rgba(0,0,0,.5)", letterSpacing: ".01em" }}>{activeMode.name}</b>
            <span style={{ color: "var(--ot-con-ink-dim)", font: "11px var(--ot-readout-font)", letterSpacing: ".06em", textTransform: "uppercase" }}>construction</span>
          </div>
          <div style={{ color: "#aab3bf", font: "12px system-ui,sans-serif", lineHeight: 1.35 }}>
            {activeMode.hint}
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            {[...TOOLS, ...(ui.ruleset === "arcadia" ? FANTASY_TOOLS : [])].map(([t, label]) => {
              const on = ui.tool === t;
              return (
                <Button
                  key={t}
                  label={label}
                  testid={`tool-${t}`}
                  onClick={() => game.setTool(t)}
                  on={on}
                  tone={t === "bulldozer" ? "danger" : "accent"}
                  style={{ flex: 1 }}
                />
              );
            })}
          </div>
          <div style={{ color: "#9aa3ad", font: "11px system-ui,sans-serif" }}>{TOOL_HINT[ui.tool]}</div>
        </div>

        {/* the chord bar (wraps on narrow viewports rather than overflowing) */}
        <div
          id="transport-bar"
          className="ot-console"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexWrap: "wrap",
            gap: 6,
            padding: 7,
            maxWidth: "96vw",
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

        {/* layer toggles + the arcadia lens-bar moved to the right-edge LensRail (stage 3). */}

        <Button label="⚙" testid="open-settings" onClick={() => setSettingsOpen((o) => !o)} />
        </div>
      </div>

      <Settings open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </>
  );
}
