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

// A doubling gear ladder (1×→8×) for fine control, plus a max fast-forward. The sim is a GameLoop
// knob (loop.setSpeed), never a Command — speed never touches sim state.
const SPEEDS: [number, string][] = [
  [1, "1×"],
  [2, "2×"],
  [4, "4×"],
  [8, "8×"],
  [100, "max"],
];

// A cut seam between key groups on the console face (a dark groove with a faint top-light).
const SEP_STYLE: CSSProperties = {
  width: 2,
  alignSelf: "stretch",
  background: "linear-gradient(180deg, rgba(0,0,0,.45), rgba(255,255,255,.05))",
  borderRadius: 1,
  margin: "2px 5px",
};

// A console KEY (the bar's Run/Build, speeds, Demand, gear, and the popover tools) — a raised physical
// button on the operator's desk (#28 diegetic theme). `on` lights it; `tone` picks the glow (accent /
// good=Run / danger=Bulldoze). Layout (flex/padding/width) still comes via `style`; the look is `.ot-key`.
function Button({
  label,
  testid,
  onClick,
  style,
  title,
  disabled,
  on,
  tone = "accent",
}: {
  label: string;
  testid: string;
  onClick: () => void;
  style?: CSSProperties;
  title?: string;
  disabled?: boolean;
  on?: boolean;
  tone?: "accent" | "good" | "danger";
}) {
  const onClass = on ? (tone === "good" ? "on-good" : tone === "danger" ? "on-danger" : "on") : "";
  return (
    <button
      data-testid={testid}
      className={`ot-key ${onClass}`}
      onClick={disabled ? undefined : onClick}
      title={title}
      disabled={disabled}
      style={{
        padding: "6px 10px",
        cursor: disabled ? "default" : "pointer",
        ...(disabled ? { opacity: 0.45, filter: "saturate(0.4)" } : null),
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
  // A big mode KEY on the console: raised graphite, the mode's identity COLOUR as a lit edge + glow when
  // selected (diegetic — a backlit selector key), the icon/name etched in light. The kbd is its key-cap.
  return (
    <button
      data-testid={`mode-transport-${m.id}`}
      className="ot-key"
      disabled={!enabled}
      onClick={onClick}
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 3,
        minWidth: 64,
        padding: "8px 10px",
        borderRadius: 10,
        cursor: enabled ? "pointer" : "not-allowed",
        opacity: enabled ? 1 : 0.4,
        ...(on
          ? {
              color: "#fff",
              boxShadow: `var(--ot-well), 0 0 0 1.5px ${m.color}, 0 0 14px ${m.color}66`,
            }
          : null),
      }}
    >
      <span style={{ fontSize: 20, lineHeight: 1, filter: on ? "none" : "saturate(.85)" }}>{m.icon}</span>
      <span style={{ font: "600 13px system-ui,sans-serif", color: on ? m.color : "var(--ot-con-ink)" }}>{m.name}</span>
      <kbd
        style={{
          font: `600 10px ${"var(--ot-readout-font)"}`,
          borderRadius: 4,
          padding: "0 4px",
          background: "rgba(0,0,0,.35)",
          border: "1px solid rgba(0,0,0,.5)",
          color: on ? m.color : "var(--ot-con-ink-dim)",
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

  // Game keyboard: 1–5 chord transport modes; Space toggles Build↔Run; T/R/N/V/X (+ arcadia B/Y) arm the
  // build tools (Track/Service/Station/Select/Bulldoze); ',' / '.' step the speed ladder. (WASD/arrows/Q/E
  // camera nav live in App.tsx.) Ignored while typing in a field and for ctrl/meta/alt chords (Ctrl-Z etc.).
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

        <Button
          label={running ? "⏸ Build" : "▶ Run"}
          testid="mode-toggle"
          onClick={() => game.setMode(running ? "build" : "run")}
          on={running}
          tone="good"
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
              on={on}
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
          on={ui.showDemand}
        />

        <Button
          label="🕐 Reach"
          testid="layer-reach"
          disabled={ui.selectedStation === null}
          title={ui.selectedStation === null ? "Reach needs a pinned station — click one first, then shade every other by how fast transit reaches it." : "Reach: shade every station by how fast transit gets there from the pinned one. Faster reach pulls more demand — extend it to unlock trips."}
          onClick={() => game.setShowReach(!ui.showReach)}
          on={ui.showReach}
        />

        <Button
          label="🛣 Roads"
          testid="layer-roads"
          title="Road corridors where buses run cheap + fast. Turn on when planning a bus line — route along roads to cut cost and speed service."
          onClick={() => game.setShowRoads(!ui.showRoads)}
          on={ui.showRoads}
        />

        <Button
          label="🧍 Peeps"
          testid="layer-peeps"
          title="Show individual riders: walking to the platform, waiting, riding the train, and heading out at their stop. Purely visual — no effect on the sim."
          onClick={() => game.setShowPeeps(!ui.showPeeps)}
          on={ui.showPeeps}
        />

        {/* TTD signals (fantasy single-track): show each block's state so meets read at a glance. */}
        {ui.ruleset === "arcadia" && (
          <Button
            label="🚦 Signals"
            testid="layer-signals"
            title="Signal view: single-track block state — 🟢 clear · 🔴 occupied · 🟠 a cart held, waiting for the block ahead. Purely visual — shows WHY carts meet and wait on single track."
            onClick={() => game.setShowSignals(!ui.showSignals)}
            on={ui.showSignals}
          />
        )}

        {/* Map LENSES (#5): an EXCLUSIVE arcadia-only view-mode selector — pick ONE reading; the others dim
            (filtered in Game.composeAndSet). Styled as a SEGMENTED control (joined buttons, radio-like) so it
            reads as one-of-N — visually distinct from the additive layer-toggle pills above. */}
        {ui.ruleset === "arcadia" && (
          <>
            <span style={SEP_STYLE} />
            <span style={{ font: "700 10px var(--ot-readout-font)", letterSpacing: ".1em", color: "var(--ot-con-ink-dim)", alignSelf: "center" }}>LENS</span>
            <div data-testid="lens-bar" style={{ display: "flex", borderRadius: 7, overflow: "hidden", boxShadow: "var(--ot-well)" }}>
              {([
                ["realm", "◉", "All", "everything"],
                ["supply", "⛏", "Supply", "sources, towns, rivers — your economy"],
                ["military", "⚔", "War", "legions, raiders, conquest targets"],
                ["decadence", "☠", "Rot", "the creeping rot — the tide + its front"],
              ] as const).map(([id, icon, lbl, title]) => {
                const sel = ui.lens === id;
                return (
                  <button
                    key={id}
                    data-testid={`lens-${id}`}
                    title={`Lens: ${title}`}
                    onClick={() => game.setLens(id)}
                    style={{
                      border: "none",
                      borderRight: "1px solid rgba(0,0,0,.4)",
                      padding: "6px 9px",
                      cursor: "pointer",
                      font: "600 12px system-ui,sans-serif",
                      background: sel ? "linear-gradient(180deg,#2a3036,#20252b)" : "linear-gradient(180deg,#363c45,#2c323b)",
                      color: sel ? "var(--ot-con-accent)" : "var(--ot-con-ink-dim)",
                      textShadow: "0 1px 1px rgba(0,0,0,.5)",
                      boxShadow: sel ? "inset 0 2px 5px rgba(0,0,0,.55), 0 0 9px rgba(56,198,220,.3)" : "inset 0 1px 0 rgba(255,255,255,.08)",
                    }}
                  >
                    {icon} {lbl}
                  </button>
                );
              })}
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
