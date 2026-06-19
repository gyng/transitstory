// Shared console-KEY primitives (the diegetic #28 control-desk look) used across the chrome's
// rails/clusters. Extracted from the old Toolbar so TimeCluster / LensRail / ConstructionRail share
// ONE button look (a raised physical key; `on` lights it, `tone` picks the glow). JSX, so it lives
// here rather than in the framework-free shared.ts. The look is owned by `.ot-key` in styles.css;
// layout (flex/padding/width) still comes via `style`.
import type { CSSProperties, ReactNode } from "react";
import type { ModeDef } from "./shared";
import type { Tool } from "../../game";
import { MODES } from "./shared";

// ─── KEYMAP: the single source of truth for every keyboard shortcut ──────────────────────────────
// The listeners (ConstructionRail = Space/tools/modes, TimeCluster = speed step, App = undo/redo +
// camera) reference THIS table for their key letters instead of hardcoding them, and the Settings
// "Keyboard controls" section renders it as a legend. One place to read, one place to change.
//
// `tool` ties a shortcut to a build tool so the tool BUTTONS can show their `<kbd>` hint from here
// (previously only the mode buttons did). `fantasyOnly` shortcuts arm fantasy tools (barracks/bounty).

/** A keyboard shortcut: the visible key cap(s), what it does, and (for build tools) the armed tool. */
export interface KeyBinding {
  /** The key cap(s) shown in `<kbd>` (e.g. "T", "Ctrl Z", "WASD"). */
  keys: string;
  /** Human-readable action label for the legend. */
  label: string;
  /** For a build-tool shortcut: the tool it arms (lets the tool button render its kbd hint). */
  tool?: Tool;
  /** Fantasy (arcadia) only — hidden in the legend + inert in the listener for transit. */
  fantasyOnly?: boolean;
}

/** Build-tool shortcuts (T/R/N/V/X + fantasy B/Y). The ConstructionRail listener builds its
 *  letter→tool map from this, and each tool button reads its `<kbd>` from `toolKey()`. */
export const TOOL_BINDINGS: KeyBinding[] = [
  { keys: "T", label: "Track tool", tool: "line" },
  { keys: "R", label: "Service tool", tool: "service" },
  { keys: "N", label: "Station tool", tool: "station" },
  { keys: "V", label: "Inspect tool", tool: "select" },
  { keys: "X", label: "Bulldoze tool", tool: "bulldozer" },
  { keys: "B", label: "Barracks tool", tool: "barracks", fantasyOnly: true },
  { keys: "Y", label: "Bounty tool", tool: "bounty", fantasyOnly: true },
];

/** Look up the single-letter hotkey for a tool (uppercase, for a `<kbd>` cap). */
export function toolKey(tool: Tool): string | undefined {
  return TOOL_BINDINGS.find((b) => b.tool === tool)?.keys;
}

/** letter (lowercase) → tool, split into the always-on transit tools and the fantasy-only ones, so
 *  the listener can include B/Y only in arcadia. */
export const TOOL_KEY_MAP: Record<string, Tool> = Object.fromEntries(
  TOOL_BINDINGS.filter((b) => !b.fantasyOnly && b.tool).map((b) => [b.keys.toLowerCase(), b.tool!]),
);
export const FANTASY_TOOL_KEY_MAP: Record<string, Tool> = Object.fromEntries(
  TOOL_BINDINGS.filter((b) => b.fantasyOnly && b.tool).map((b) => [b.keys.toLowerCase(), b.tool!]),
);

/** The legend, grouped into labelled sections for the Settings "Keyboard controls" panel. Mode
 *  shortcuts derive from MODES so they never drift from the actual chord keys. */
export interface KeyGroup {
  title: string;
  bindings: KeyBinding[];
}
export const KEYMAP: KeyGroup[] = [
  {
    title: "Build & run",
    bindings: [
      { keys: "Space", label: "Build ↔ Run (the hard wall)" },
      ...TOOL_BINDINGS,
    ],
  },
  {
    title: "Transport modes",
    bindings: MODES.map((m) => ({ keys: m.key, label: m.name })),
  },
  {
    title: "Speed & time",
    bindings: [
      { keys: ", .", label: "Slower / faster (speed ladder)" },
    ],
  },
  {
    title: "Edit",
    bindings: [
      { keys: "Ctrl Z", label: "Undo" },
      { keys: "Ctrl Y", label: "Redo (or Ctrl Shift Z)" },
    ],
  },
  {
    title: "Camera",
    bindings: [
      { keys: "WASD / ↑↓←→", label: "Pan the map" },
      { keys: "Q E", label: "Zoom out / in (also − / =)" },
    ],
  },
];

/** A key-cap `<kbd>` hint (the same etched-cap look the BigModeButton uses). Shared so the tool
 *  buttons + the Settings keyboard legend render shortcuts identically. */
export function Kbd({ children, lit }: { children: ReactNode; lit?: boolean }) {
  return (
    <kbd
      style={{
        font: `600 10px var(--ot-readout-font)`,
        borderRadius: 4,
        padding: "0 4px",
        background: "rgba(0,0,0,.35)",
        border: "1px solid rgba(0,0,0,.5)",
        color: lit ? "var(--ot-con-accent)" : "var(--ot-con-ink-dim)",
        whiteSpace: "nowrap",
      }}
    >
      {children}
    </kbd>
  );
}

/** A console KEY — a raised physical button on the operator's desk. `on` lights it; `tone` picks
 *  the glow (accent / good=Run / danger=Bulldoze). */
export function Button({
  label,
  testid,
  onClick,
  style,
  title,
  disabled,
  gated,
  on,
  tone = "accent",
  compact = false,
  kbd,
}: {
  label: string;
  testid: string;
  onClick: () => void;
  style?: CSSProperties;
  title?: string;
  disabled?: boolean;
  /** A GATED control — off because a precondition isn't met (e.g. Reach needs a pinned station). When also
   *  `disabled`, it stays FOCUSABLE via aria-disabled so a keyboard user can reach it and read the title's gate
   *  reason; native `disabled` would drop BOTH the tooltip and the tab stop — hiding the "why is this off?" hint
   *  that matters most. The onClick is still suppressed while disabled, so it's inert either way. */
  gated?: boolean;
  on?: boolean;
  tone?: "accent" | "good" | "danger";
  /** When set, the text after the leading glyph is wrapped in `.ot-con-compact-label` so the responsive
   *  (<1024px) rules can hide it, leaving an icon-only key (left/lens rails). */
  compact?: boolean;
  /** Optional trailing key-cap hint (e.g. a tool hotkey from the KEYMAP). Hidden when compact collapses
   *  to icon-only (wrapped in `.ot-con-compact-label`), so a narrow icon key stays clean. */
  kbd?: string;
}) {
  const onClass = on ? (tone === "good" ? "on-good" : tone === "danger" ? "on-danger" : "on") : "";
  // Split a "🚆 Rail"-style label into its leading glyph + the rest, so the rest can collapse at narrow
  // widths. Falls back to the raw label when there's no space (already icon-only).
  let content: ReactNode = label;
  if (compact) {
    const sp = label.indexOf(" ");
    if (sp > 0) {
      content = (
        <>
          {label.slice(0, sp)}
          <span className="ot-con-compact-label">{label.slice(sp)}</span>
        </>
      );
    }
  }
  const kbdNode = kbd ? (
    <span className={compact ? "ot-con-compact-label" : undefined} style={{ marginLeft: 6 }}>
      <Kbd lit={on}>{kbd}</Kbd>
    </span>
  ) : null;
  return (
    <button
      data-testid={testid}
      className={`ot-key ${onClass}`}
      onClick={disabled ? undefined : onClick}
      title={title}
      disabled={disabled && !gated}
      aria-disabled={disabled || undefined}
      style={{
        display: "inline-flex",
        alignItems: "center",
        padding: "6px 10px",
        cursor: disabled ? "default" : "pointer",
        ...(disabled ? { opacity: 0.45, filter: "saturate(0.4)" } : null),
        ...style,
      }}
    >
      {content}
      {kbdNode}
    </button>
  );
}

/** A transport-MODE selector key: raised graphite, the mode's identity COLOUR as a lit edge + glow
 *  when selected (diegetic backlit selector key), icon/name etched in light + a key-cap kbd.
 *  `compact` packs it into one short ROW (icon · name · kbd) so the RAIL flyout stays ~100px — the
 *  name collapses to icon-only below 1024px (`.ot-con-compact-label`); the default is the taller
 *  column used elsewhere. */
export function BigModeButton({
  m,
  active,
  enabled,
  onClick,
  compact = false,
}: {
  m: ModeDef;
  active: boolean;
  enabled: boolean;
  onClick: () => void;
  compact?: boolean;
}) {
  const on = active && enabled;
  return (
    <button
      data-testid={`mode-transport-${m.id}`}
      className="ot-key"
      disabled={!enabled}
      onClick={onClick}
      style={{
        display: "flex",
        flexDirection: compact ? "row" : "column",
        alignItems: "center",
        gap: compact ? 5 : 3,
        minWidth: compact ? 0 : 64,
        padding: compact ? "5px 8px" : "8px 10px",
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
      <span style={{ fontSize: compact ? 16 : 20, lineHeight: 1, filter: on ? "none" : "saturate(.85)" }}>{m.icon}</span>
      <span
        className={compact ? "ot-con-compact-label" : undefined}
        style={{ font: "600 13px system-ui,sans-serif", color: on ? m.color : "var(--ot-con-ink)" }}
      >
        {m.name}
      </span>
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
