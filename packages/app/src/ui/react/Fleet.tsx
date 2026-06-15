// FLEET panel (#rolling-stock): view / build / edit trainsets DIRECTLY in one place + LIVE status in run
// mode. The line roster + Editor already cover per-line edits; this consolidates ALL rolling stock — every
// line's model, fleet size (inline ± edit), headway, and a live load bar — so you manage the whole fleet at
// a glance and watch it run. Pure React chrome: reads the ~3 Hz stats snapshot, writes via Game methods
// (AssignTrainset). No sim mutation here, no per-frame work — the load bar tracks the snapshot, not rAF.
import { useState } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { RAIL_ROSTER, hex, modeIcon } from "./shared";
import type { PerLine } from "../../types";

const PANEL: React.CSSProperties = {
  position: "fixed",
  top: 56,
  right: 252, // left of the Editor column
  width: 280,
  maxHeight: "70vh",
  overflowY: "auto",
  background: "rgba(255,255,255,.97)",
  borderRadius: 10,
  boxShadow: "var(--ot-shadow)",
  zIndex: 9,
  font: "13px system-ui,sans-serif",
  color: "#1c2024",
};

function loadColor(lf: number): string {
  return lf >= 0.92 ? "#d4604f" : lf >= 0.7 ? "#e6a23c" : "#5aa469";
}

function FleetRow({ l, running }: { l: PerLine; running: boolean }) {
  const game = useGame();
  const hasTrains = l.trains > 0;
  // RAIL (mode 0) exposes the model catalog; other modes have a single preset (show its mode glyph).
  const isRail = l.mode === 0;
  const spec = l.trainsetSpec ?? 0;
  const modelName = isRail ? (RAIL_ROSTER[spec]?.name ?? "Standard") : `${modeIcon(l.mode)} default`;
  const lf = l.loadFactor ?? 0;
  const setCount = (n: number) => game.assignTrainset(l.lineId, Math.max(1, Math.min(24, n)));
  return (
    <div data-testid={`fleet-row-${l.lineId}`} style={{ padding: "8px 10px", borderTop: "1px solid #eef0f2" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
        <span style={{ width: 22, height: 16, borderRadius: 4, background: hex(l.color), flex: "0 0 auto" }} />
        <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontWeight: 600 }} title={l.name}>
          {l.name || `Line ${l.lineId + 1}`}
        </span>
        {hasTrains ? (
          // Inline fleet-size editor — build/edit trainsets directly without opening the line.
          <span style={{ display: "flex", alignItems: "center", gap: 3, flex: "0 0 auto" }}>
            <button data-testid={`fleet-dec-${l.lineId}`} onClick={() => setCount(l.trains - 1)} style={stepBtn}>−</button>
            <span data-testid={`fleet-count-${l.lineId}`} style={{ minWidth: 28, textAlign: "center", fontVariantNumeric: "tabular-nums", fontWeight: 700 }}>{l.trains}🚆</span>
            <button data-testid={`fleet-inc-${l.lineId}`} onClick={() => setCount(l.trains + 1)} style={stepBtn}>+</button>
          </span>
        ) : (
          <button data-testid={`fleet-assign-${l.lineId}`} onClick={() => setCount(2)} style={{ ...stepBtn, width: "auto", padding: "2px 8px" }}>+ trains</button>
        )}
      </div>
      <div style={{ marginLeft: 29, marginTop: 4, display: "flex", alignItems: "center", gap: 8, color: "#7a818a", fontSize: 11 }}>
        <span>{modelName}</span>
        <span style={{ color: "#cfd4da" }}>·</span>
        <span>{Math.max(1, Math.round(l.headwayMs / 60000))} min</span>
        {hasTrains && (
          <>
            <span style={{ color: "#cfd4da" }}>·</span>
            {/* Live load bar — fills + colours with the fleet's mean load while running (view-mode status). */}
            <span style={{ flex: 1, height: 6, background: "#eceef1", borderRadius: 3, overflow: "hidden" }} title={`Load ${Math.round(lf * 100)}%`}>
              <span data-testid={`fleet-load-${l.lineId}`} style={{ display: "block", width: `${Math.min(100, Math.round(lf * 100))}%`, height: "100%", background: loadColor(lf), opacity: running ? 1 : 0.45 }} />
            </span>
          </>
        )}
      </div>
      {/* RAIL model quick-pick (build/edit the model directly) — only when trains run + the catalog applies. */}
      {isRail && hasTrains && (
        <div style={{ marginLeft: 29, marginTop: 5, display: "flex", gap: 4 }}>
          {RAIL_ROSTER.map((m, i) => (
            <button
              key={m.name}
              data-testid={`fleet-model-${l.lineId}-${i}`}
              title={`${m.name} — ${m.capacity} cap · ${m.kmh} km/h · $${m.costM}M`}
              onClick={() => game.setAircraft(l.lineId, i)}
              style={{
                flex: 1,
                padding: "3px 0",
                borderRadius: 6,
                border: spec === i ? "1px solid #0072b2" : "1px solid #d7dade",
                background: spec === i ? "#eef4fb" : "#fff",
                font: "600 11px system-ui",
                color: "#1c2024",
                cursor: "pointer",
              }}
            >
              {m.name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

const stepBtn: React.CSSProperties = {
  width: 22,
  height: 22,
  borderRadius: 6,
  border: "1px solid #d7dade",
  background: "#fff",
  font: "700 14px system-ui",
  color: "#1c2024",
  cursor: "pointer",
  lineHeight: 1,
};

export function Fleet() {
  const stats = useStats();
  const ui = useGameUI();
  const [open, setOpen] = useState(false);
  const lines = stats.perLine.filter((l) => l.stops >= 2);
  const running = stats.running;
  const totalTrains = lines.reduce((a, l) => a + l.trains, 0);

  return (
    <>
      <button
        data-testid="fleet-toggle"
        onClick={() => setOpen((o) => !o)}
        title="Fleet — view, build + edit every line's trainsets, with live status"
        style={{
          position: "fixed",
          top: 16,
          right: 14,
          zIndex: 10,
          padding: "5px 11px",
          borderRadius: 8,
          border: "0",
          background: open ? "#0072b2" : "#1c2024",
          color: "#fff",
          font: "700 13px system-ui,sans-serif",
          cursor: "pointer",
          boxShadow: "var(--ot-shadow)",
        }}
      >
        🚆 Fleet{totalTrains > 0 ? ` · ${totalTrains}` : ""}
      </button>
      {open && (
        <div data-testid="fleet-panel" style={PANEL}>
          <div style={{ padding: "10px 12px 6px", fontWeight: 700, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span>🚆 Fleet {running ? "· live" : ""}</span>
            <button data-testid="fleet-close" onClick={() => setOpen(false)} style={{ border: 0, background: "transparent", cursor: "pointer", fontSize: 16, color: "#9aa1a9" }}>×</button>
          </div>
          {lines.length === 0 ? (
            <div style={{ padding: "4px 12px 14px", color: "#9aa1a9", fontSize: 12 }}>Draw a line (≥ 2 stops), then assign trains here.</div>
          ) : (
            <div style={{ paddingBottom: 8 }}>
              {lines.map((l) => (
                <FleetRow key={l.lineId} l={l} running={running} />
              ))}
            </div>
          )}
          <div style={{ padding: "6px 12px 12px", color: "#9aa3ad", fontSize: 11, lineHeight: 1.35, borderTop: "1px solid #eceef1" }}>
            {ui.ruleset === "arcadia" ? "Heavier carts haul more; express carts run faster." : "± sets fleet size · pick a model · the bar shows live load."}
          </div>
        </div>
      )}
    </>
  );
}
