// Right-click context menu (run/select mode). A small token-styled card at the cursor offering
// Inspect / Bulldoze on the resolved target (station, line) or a few view/time power-tools on empty
// map. Reads the `contextMenu` UI slice (set by Game.openContextMenu from pointer.ts) and routes
// every item to an EXISTING Game method (no new Command surface). Build mode never opens this — the
// pointer keeps its two-stage "stop building" there. Dismiss: pick an item, Esc, or click away.
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { useGame, useGameUI } from "./GameContext";
import { hex } from "./shared";

function MenuItem({
  icon,
  label,
  testid,
  onClick,
  danger,
  disabled,
}: {
  icon: string;
  label: string;
  testid: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}) {
  const [hover, setHover] = useState(false);
  const color = disabled ? "#b3b9c0" : danger ? "var(--ot-gauge-bad,#d62828)" : "#1c2024";
  return (
    <div
      data-testid={testid}
      role="button"
      onClick={disabled ? undefined : onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "6px 12px",
        cursor: disabled ? "default" : "pointer",
        color,
        background: hover && !disabled ? "#f1f3f5" : "transparent",
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ width: 16, textAlign: "center" }}>{icon}</span>
      <span>{label}</span>
    </div>
  );
}

const HEADER: CSSProperties = { padding: "7px 12px 5px", font: "600 12px system-ui", color: "#5a626b", display: "flex", alignItems: "center", gap: 6, borderBottom: "1px solid #eceef1", maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis" };
const SEP: CSSProperties = { height: 1, background: "#eceef1", margin: "3px 0" };

export function ContextMenu() {
  const game = useGame();
  const ui = useGameUI();
  const cm = ui.contextMenu;
  const [armed, setArmed] = useState(false); // second-click confirm for the destructive item

  // Esc closes the menu (in run/select); reset the bulldoze arm whenever the target changes.
  useEffect(() => {
    setArmed(false);
    if (!cm) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") game.closeContextMenu();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [cm, game]);

  if (!cm) return null;
  const close = () => game.closeContextMenu();
  // Clamp to the viewport (flip near the right / bottom edge).
  const W = 210;
  const estH = cm.kind === "empty" ? 168 : 96;
  const x = Math.min(cm.x, window.innerWidth - W - 8);
  const y = Math.min(cm.y, window.innerHeight - estH - 8);

  const station = cm.kind === "station" ? game.bridge.stationsView()[cm.id] : undefined;
  const line = cm.kind === "line" ? game.perLineById.get(cm.id) : undefined;

  return (
    <div
      data-testid="context-menu"
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left: x,
        top: y,
        width: W,
        zIndex: 30,
        background: "rgba(255,255,255,.98)",
        borderRadius: 8,
        boxShadow: "var(--ot-shadow)",
        padding: "4px 0",
        font: "13px system-ui,sans-serif",
        color: "#1c2024",
        pointerEvents: "auto",
        userSelect: "none",
      }}
    >
      {cm.kind === "station" && (
        <>
          <div style={HEADER}>◉ {station?.name || `Station ${cm.id + 1}`}</div>
          <MenuItem icon="🔍" label="Inspect" testid="ctx-inspect" onClick={() => { game.selectStation(cm.id); close(); }} />
          <div style={SEP} />
          <MenuItem
            icon="💥"
            label={armed ? "Bulldoze — confirm?" : "Bulldoze"}
            testid="ctx-bulldoze"
            danger
            onClick={() => (armed ? (game.removeStationById(cm.id), close()) : setArmed(true))}
          />
        </>
      )}

      {cm.kind === "line" && (
        <>
          <div style={HEADER}>
            <span style={{ width: 11, height: 11, borderRadius: 3, flex: "0 0 auto", background: hex(line?.color ?? 0x888888) }} />
            <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{line?.name || `Line ${cm.id + 1}`}</span>
          </div>
          <MenuItem icon="🔍" label="Inspect / Edit" testid="ctx-inspect" onClick={() => { game.selectLine(cm.id); close(); }} />
          <div style={SEP} />
          <MenuItem
            icon="💥"
            label={armed ? "Bulldoze line — confirm?" : "Bulldoze line"}
            testid="ctx-bulldoze"
            danger
            onClick={() => (armed ? (game.removeLineById(cm.id), close()) : setArmed(true))}
          />
        </>
      )}

      {cm.kind === "empty" && (
        <>
          <MenuItem icon="🧍" label={`${ui.showPeeps ? "Hide" : "Show"} riders (peeps)`} testid="ctx-peeps" onClick={() => { game.setShowPeeps(!ui.showPeeps); close(); }} />
          <MenuItem icon="🌡" label={`${ui.showDemand ? "Hide" : "Show"} demand heat`} testid="ctx-demand" onClick={() => { game.setShowDemand(!ui.showDemand); close(); }} />
          <MenuItem icon="👁" label="Follow a random rider" testid="ctx-follow" onClick={() => { game.followRandomPeep(); close(); }} />
          <div style={SEP} />
          <MenuItem icon="📍" label="Center here" testid="ctx-center" onClick={() => { game.map.easeTo({ center: [cm.lngLat.lng, cm.lngLat.lat] }); close(); }} />
        </>
      )}
    </div>
  );
}
