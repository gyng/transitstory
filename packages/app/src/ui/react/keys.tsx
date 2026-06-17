// Shared console-KEY primitives (the diegetic #28 control-desk look) used across the chrome's
// rails/clusters. Extracted from the old Toolbar so TimeCluster / LensRail / ConstructionRail share
// ONE button look (a raised physical key; `on` lights it, `tone` picks the glow). JSX, so it lives
// here rather than in the framework-free shared.ts. The look is owned by `.ot-key` in styles.css;
// layout (flex/padding/width) still comes via `style`.
import type { CSSProperties } from "react";
import type { ModeDef } from "./shared";

/** A console KEY — a raised physical button on the operator's desk. `on` lights it; `tone` picks
 *  the glow (accent / good=Run / danger=Bulldoze). */
export function Button({
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

/** A big transport-MODE selector key: raised graphite, the mode's identity COLOUR as a lit edge +
 *  glow when selected (diegetic backlit selector key), icon/name etched in light + a key-cap kbd. */
export function BigModeButton({
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
