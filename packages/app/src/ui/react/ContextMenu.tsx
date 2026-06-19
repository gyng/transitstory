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
  const color = disabled ? "var(--ot-con-ink-dim)" : danger ? "var(--ot-con-red)" : "var(--ot-con-ink)";
  return (
    <div
      data-testid={testid}
      role="menuitem" // #16 a menuitem inside the role="menu" container (was role="button")
      // #25 keyboard-operable: the rows announce as buttons but were mouse-only (no tab stop, no key handler).
      // Tab reaches each, Enter/Space activates, and focus reuses the hover highlight so the target is visible.
      tabIndex={disabled ? -1 : 0}
      aria-disabled={disabled || undefined}
      onClick={disabled ? undefined : onClick}
      onKeyDown={disabled ? undefined : (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onClick(); } }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onFocus={() => setHover(true)}
      onBlur={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "6px 12px",
        cursor: disabled ? "default" : "pointer",
        color,
        background: hover && !disabled ? "rgba(56,198,220,.14)" : "transparent",
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ width: 16, textAlign: "center" }}>{icon}</span>
      <span>{label}</span>
    </div>
  );
}

const HEADER: CSSProperties = { padding: "7px 12px 5px", font: "600 12px system-ui", color: "var(--ot-con-ink-dim)", display: "flex", alignItems: "center", gap: 6, borderBottom: "1px solid rgba(255,255,255,.08)", maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis" };
const SEP: CSSProperties = { height: 1, background: "rgba(255,255,255,.08)", margin: "3px 0" };
const INFO: CSSProperties = { padding: "5px 12px", color: "var(--ot-con-ink-dim)", fontSize: 12, lineHeight: 1.45 };

// Inspect labels for the baked fantasy POIs — mirror the on-map node glyphs (render.ts) so the menu
// names match what you clicked.
function townInfo(kind: string): { glyph: string; label: string } {
  if (kind === "capital") return { glyph: "★", label: "Capital seat" };
  if (kind === "starter") return { glyph: "✪", label: "Your hold" };
  return { glyph: "⌂", label: "Town" };
}
function resourceInfo(kind: string): { glyph: string; label: string } {
  switch (kind) {
    case "ore": return { glyph: "⛏", label: "Ore vein" };
    case "grain": return { glyph: "✿", label: "Grainfield" };
    case "fuel": return { glyph: "♣", label: "Fuel grove" };
    case "aether": return { glyph: "✦", label: "Aether well" };
    case "forge": return { glyph: "⚒", label: "Forge" };
    default: return { glyph: "◆", label: kind || "Source" };
  }
}
function chainNeeds(chain: string): string {
  if (chain === "bread") return "grain + fuel";
  if (chain === "arms") return "ore + aether";
  return chain;
}

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
  const estH = cm.kind === "empty" ? 168 : cm.kind === "town" ? 150 : cm.kind === "resource" || cm.kind === "vehicle" ? 118 : 96;
  const x = Math.min(cm.x, window.innerWidth - W - 8);
  const y = Math.min(cm.y, window.innerHeight - estH - 8);

  const station = cm.kind === "station" ? game.bridge.stationsView()[cm.id] : undefined;
  const poi = cm.kind === "station" ? game.stationPoi(cm.id) : undefined; // the town/resource this station sits on
  const line = cm.kind === "line" ? game.perLineById.get(cm.id) : undefined;
  const veh = cm.kind === "vehicle" ? game.vehicleInspect(cm.id) : undefined;
  const town = cm.kind === "town" ? game.towns[cm.id] : undefined;
  const res = cm.kind === "resource" ? game.resources[cm.id] : undefined;

  return (
    <div
      data-testid="context-menu"
      className="ot-console"
      role="menu" // #16 it announces as a menu of menuitems now, not a bare div of buttons
      aria-label={`Actions for this ${cm.kind}`}
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left: x,
        top: y,
        width: W,
        zIndex: 30,
        padding: "4px 0",
        font: "13px system-ui,sans-serif",
        pointerEvents: "auto",
        userSelect: "none",
      }}
    >
      {cm.kind === "station" && (
        <>
          <div style={HEADER}>◉ {station?.name || `Station ${cm.id + 1}`}</div>
          {/* The supply-chain role of the town/resource this station sits on (every fantasy station does). */}
          {poi?.town && (
            <div style={INFO}>
              {townInfo(poi.town.kind).glyph} {townInfo(poi.town.kind).label} · tribute {poi.town.value.toLocaleString()}
              {poi.town.chain ? <><br />Needs: {chainNeeds(poi.town.chain)}</> : null}
              {poi.town.decadence > 0 ? <><br />Decadence floor: {Math.round(poi.town.decadence)}%</> : null}
            </div>
          )}
          {poi?.resource && !poi.town && (
            <div style={INFO}>
              {resourceInfo(poi.resource.kind).glyph} {resourceInfo(poi.resource.kind).label} · yield {poi.resource.yield}
              <br />Feeds: {poi.resource.kind === "grain" || poi.resource.kind === "fuel" ? "bread chain" : poi.resource.kind === "ore" || poi.resource.kind === "aether" ? "arms chain" : "supply"}
            </div>
          )}
          <MenuItem icon="🔍" label="Inspect" testid="ctx-inspect" onClick={() => { game.selectStation(cm.id); close(); }} />
          {/* Mid-line insertion: with a line selected, an off-line station can join it at the
              span it sits closest to (one AddStop — one undo step). */}
          {(() => {
            if (ui.selectedLine === null) return null;
            const lv = game.bridge.linesView()[ui.selectedLine];
            if (!lv || lv.removed || lv.stops.length < 2 || lv.stops.includes(cm.id)) return null;
            return (
              <MenuItem
                icon="➕"
                label={`Add to ${lv.name || `Line ${lv.id + 1}`}`}
                testid="ctx-add-to-line"
                onClick={() => { game.insertStopOnLine(lv.id, cm.id); close(); }}
              />
            );
          })()}
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

      {cm.kind === "vehicle" && veh && (
        <>
          <div style={HEADER}>
            <span style={{ width: 11, height: 11, borderRadius: 3, flex: "0 0 auto", background: hex(veh.color) }} />
            <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>🚆 {veh.name}</span>
          </div>
          <div style={INFO}>
            Hauling {veh.onboard} / {veh.capacity}
            {veh.capacity > 0 ? ` (${Math.round((veh.onboard / veh.capacity) * 100)}%)` : ""}
          </div>
          <MenuItem icon="🔍" label="Inspect line" testid="ctx-inspect" onClick={() => { game.selectLine(veh.lineId); close(); }} />
        </>
      )}

      {cm.kind === "peep" && (
        <>
          <div style={HEADER}>🧍 Rider</div>
          <MenuItem icon="👁" label="Follow this rider" testid="ctx-follow" onClick={() => { game.setFollowed(cm.id); close(); }} />
          <MenuItem icon="📍" label="Center here" testid="ctx-center" onClick={() => { game.map.easeTo({ center: [cm.lngLat.lng, cm.lngLat.lat] }); close(); }} />
        </>
      )}

      {cm.kind === "town" && town && (
        <>
          <div style={HEADER}>{townInfo(town.kind).glyph} {townInfo(town.kind).label}</div>
          <div style={INFO}>
            Tribute reward: {town.value.toLocaleString()}
            {town.chain ? <><br />Needs: {chainNeeds(town.chain)}</> : null}
            <br />Decadence: {Math.round(town.decadence)}%
          </div>
          <div style={SEP} />
          <MenuItem icon="📍" label="Center here" testid="ctx-center" onClick={() => { game.map.easeTo({ center: [cm.lngLat.lng, cm.lngLat.lat] }); close(); }} />
        </>
      )}

      {cm.kind === "resource" && res && (
        <>
          <div style={HEADER}>{resourceInfo(res.kind).glyph} {resourceInfo(res.kind).label}</div>
          <div style={INFO}>
            Yield: {res.yield}/cycle
            <br />Feeds: {res.kind === "grain" || res.kind === "fuel" ? "bread chain" : res.kind === "ore" || res.kind === "aether" ? "arms chain" : "supply"}
          </div>
          <div style={SEP} />
          <MenuItem icon="📍" label="Center here" testid="ctx-center" onClick={() => { game.map.easeTo({ center: [cm.lngLat.lng, cm.lngLat.lat] }); close(); }} />
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
