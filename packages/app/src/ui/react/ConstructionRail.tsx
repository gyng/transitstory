// LEFT-EDGE construction rail (OpenTTD/Transport-Tycoon sub-toolbar): a vertical column of category
// keys; the armed key opens a horizontal flyout of its tools. This is where the old bottom
// #transport-bar's modes + tools went — so the union of every build group never co-renders in one row
// again (the structural overflow fix is complete once #transport-bar is deleted).
//
//   L1 categories (one armed, radio): RAIL · MILITARY · BULLDOZE · TECH · BOUNTY. MILITARY/BOUNTY/TECH
//   are fantasy-only (greyed/hidden in transit).
//   L2 flyout (anchored to the armed key):
//     • RAIL → tool-select (head) · tool-line (Track) · tool-service · tool-station ‖ the transport-MODE
//       segment (mode-transport-*, gated by enabledModes; arcadia rail-gate shows rail + teched heavy) +
//       the per-tool hint. The flyout is OPEN by default so the mode buttons are visible without a click
//       (the modes/tech e2e assert mode-transport-0 visible on load).
//     • MILITARY → tool-barracks.  BULLDOZE → arms tool-bulldozer (danger, no submenu).
//     • TECH → the Forge of Ages launcher/panel (TechPanel, arcadia-only, self-positioned).
//     • BOUNTY → tool-bounty.
//
// The build keyboard (Space=Build/Run, 1-5=transport modes, T/R/N/V/X/B/Y=tools) lives here now;
// arming a tool also arms its category so the flyout follows. AGENTS: React owns DOM chrome only;
// writes via Game methods (game.setTool/setTransport/setMode); the live route HUD (BuildHud) floats.
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import type { Tool } from "../../game";
import { useGame, useGameUI, useStats } from "./GameContext";
import { MODES } from "./shared";
import { Button, BigModeButton } from "./keys";
import { techUnlocked } from "../../commands/codec";
import { TechPanel } from "./TechPanel";

// Per-tool hint shown in the flyout (the "how to cancel" affordance). TTD L6: TRACK lays bare grey rail
// (infrastructure); SERVICE draws the same way but lands a live coloured line (auto-stocked).
const TOOL_HINT: Record<Tool, string> = {
  line: "[T] Lay TRACK: click stations to chain bare rail (drops new ones as you go) · double-click to build · ⌫ undo · Esc to cancel. Stays grey until a service runs on it.",
  service: "[R] Run a SERVICE: click stations to route a coloured line (over track or fresh) · double-click to build · ⌫ undo · Esc to cancel. Lands with trains already running — tune them in the panel.",
  station: "[N] Click to drop a station — stays armed for the next · the Track/Service tools also drop them while chaining · Esc when done",
  select: "[V] Click — or right-click anything — to inspect it. Click bare grey track to assign it trains.",
  bulldozer: "[X] Click a station or line to demolish it · Esc to stop · right-click to inspect",
  barracks: "[B] Click to place a barracks — it fields legions once supplied · Esc when done",
  bounty: "[Y] Click a town to post a bounty — steers the NEXT legions fielded toward it (already-marching legions keep their target) · Esc when done",
};

// RAIL flyout tools: select at the head, then the build tools.
const RAIL_TOOLS: [Tool, string][] = [
  ["select", "▣ Select"],
  ["line", "╱ Track"],
  ["service", "🚆 Service"],
  ["station", "◉ Station"],
];

type Category = "rail" | "military" | "bulldoze" | "tech" | "bounty";

// Which tool a category arms when clicked (rail/tech open a flyout instead; see below).
const CATEGORY_TOOL: Partial<Record<Category, Tool>> = {
  military: "barracks",
  bulldoze: "bulldozer",
  bounty: "bounty",
};

// Map a tool back to its owning category, so arming a tool by hotkey arms the right category.
const TOOL_CATEGORY: Record<Tool, Category> = {
  select: "rail",
  line: "rail",
  service: "rail",
  station: "rail",
  bulldozer: "bulldoze",
  barracks: "military",
  bounty: "bounty",
};

const CATEGORIES: { id: Category; label: string; fantasyOnly: boolean; tone?: "danger" }[] = [
  { id: "rail", label: "🚆 Rail", fantasyOnly: false },
  { id: "military", label: "🏰 Military", fantasyOnly: true },
  { id: "bulldoze", label: "💥 Bulldoze", fantasyOnly: false, tone: "danger" },
  { id: "tech", label: "⚒ Tech", fantasyOnly: true },
  { id: "bounty", label: "⚑ Bounty", fantasyOnly: true },
];

const RAIL_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: 7,
  pointerEvents: "auto",
};
const KEY_STYLE: CSSProperties = { width: "100%", justifyContent: "flex-start", textAlign: "left" };

// A flyout panel anchored to the right of the armed category key.
const FLYOUT_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 8,
  padding: "12px 14px",
  width: "min(440px,70vw)",
  pointerEvents: "auto",
};

export function ConstructionRail() {
  const game = useGame();
  const ui = useGameUI();
  const stats = useStats();
  const arcadia = ui.ruleset === "arcadia";

  // Armed category (radio). RAIL by default so the mode segment is visible on load.
  const [armed, setArmed] = useState<Category>("rail");

  // Build keyboard: Space toggles Build↔Run; 1-5 chord transport modes; T/R/N/V/X (+ arcadia B/Y) arm the
  // build tools (and their owning category). (',' / '.' speed-step live in TimeCluster; WASD/Q/E camera in
  // App.tsx.) Ignored while typing in a field and for ctrl/meta/alt chords (Ctrl-Z etc.).
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
      if (m) { game.setTransport(m.id); setArmed("rail"); return; }
      // Build-tool hotkeys — disjoint from WASD/Q/E camera keys. T=Track, R=Service, N=statioN, V=select,
      // X=bulldoze; arcadia adds B=Barracks, Y=bountY. Arming a tool in Run flips to Build first.
      const lk = e.key.toLowerCase();
      const TOOL_KEYS: Record<string, Tool> = { t: "line", r: "service", n: "station", v: "select", x: "bulldozer" };
      if (arcadia) { TOOL_KEYS.b = "barracks"; TOOL_KEYS.y = "bounty"; }
      const tool = TOOL_KEYS[lk];
      if (tool) {
        if (game.mode === "run") game.setMode("build");
        game.setTool(tool);
        setArmed(TOOL_CATEGORY[tool]);
        return;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [game, arcadia]);

  // Visible transport modes: transit shows all; arcadia rail-gates to rail (+ heavy once teched).
  const HEAVY_RAIL_TECH = 5; // crates/sim/tech.rs HEAVY_RAIL id
  const enabled = new Set(ui.enabledModes);
  const visibleModes = arcadia
    ? MODES.filter((m) => m.id === 0 || (m.id === 4 && techUnlocked(stats.techUnlocked, HEAVY_RAIL_TECH)))
    : MODES;

  const categories = CATEGORIES.filter((c) => !c.fantasyOnly || arcadia);

  // Clicking a category: rail/tech open their flyout (no tool); the rest arm their tool (and flip to Build).
  const armCategory = (c: Category) => {
    setArmed(c);
    const t = CATEGORY_TOOL[c];
    if (t) {
      if (game.mode === "run") game.setMode("build");
      game.setTool(t);
    }
  };

  return (
    <div style={{ display: "flex", alignItems: "flex-start", gap: 8, pointerEvents: "none" }}>
      {/* L1 — the category keys */}
      <div data-testid="construction-rail" className="ot-console" style={RAIL_STYLE}>
        {categories.map((c) => (
          <Button
            key={c.id}
            label={c.label}
            testid={`category-${c.id}`}
            onClick={() => armCategory(c.id)}
            on={armed === c.id}
            tone={c.tone === "danger" ? "danger" : "accent"}
            style={KEY_STYLE}
          />
        ))}
      </div>

      {/* L2 — the armed category's flyout (anchored to the right of the rail) */}
      {armed === "rail" && (
        <div id="mode-flyout" className="ot-console" style={FLYOUT_STYLE}>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
            {RAIL_TOOLS.map(([t, label]) => (
              <Button
                key={t}
                label={label}
                testid={`tool-${t}`}
                onClick={() => game.setTool(t)}
                on={ui.tool === t}
                style={{ flex: "1 0 auto" }}
              />
            ))}
          </div>
          {/* the transport-MODE segment (gated by enabledModes; arcadia rail-gate) */}
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
            {visibleModes.map((m) => (
              <BigModeButton
                key={m.id}
                m={m}
                active={ui.transport === m.id}
                enabled={enabled.has(m.id)}
                onClick={() => game.setTransport(m.id)}
              />
            ))}
          </div>
          <div style={{ color: "#9aa3ad", font: "11px system-ui,sans-serif" }}>{TOOL_HINT[ui.tool]}</div>
        </div>
      )}

      {armed === "military" && (
        <div className="ot-console" style={FLYOUT_STYLE}>
          <Button label="🏰 Barracks" testid="tool-barracks" onClick={() => game.setTool("barracks")} on={ui.tool === "barracks"} style={KEY_STYLE} />
          <div style={{ color: "#9aa3ad", font: "11px system-ui,sans-serif" }}>{TOOL_HINT.barracks}</div>
        </div>
      )}

      {armed === "bounty" && (
        <div className="ot-console" style={FLYOUT_STYLE}>
          <Button label="⚑ Bounty" testid="tool-bounty" onClick={() => game.setTool("bounty")} on={ui.tool === "bounty"} style={KEY_STYLE} />
          <div style={{ color: "#9aa3ad", font: "11px system-ui,sans-serif" }}>{TOOL_HINT.bounty}</div>
        </div>
      )}

      {armed === "bulldoze" && (
        <div className="ot-console" style={FLYOUT_STYLE}>
          <Button label="💥 Bulldoze" testid="tool-bulldozer" onClick={() => game.setTool("bulldozer")} on={ui.tool === "bulldozer"} tone="danger" style={KEY_STYLE} />
          <div style={{ color: "#9aa3ad", font: "11px system-ui,sans-serif" }}>{TOOL_HINT.bulldozer}</div>
        </div>
      )}

      {/* TECH: the Forge launcher/panel renders itself (position:fixed, arcadia-only). Mounted here so it's
          conceptually part of construction; arming TECH is a no-op beyond highlighting the category key. */}
      {arcadia && <TechPanel />}
    </div>
  );
}
