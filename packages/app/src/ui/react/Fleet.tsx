// FLEET panel (#rolling-stock): view / build / edit trainsets DIRECTLY in one place + LIVE status in run
// mode. The line roster + Editor already cover per-line edits; this consolidates ALL rolling stock — every
// line's model, fleet size (inline ± edit), headway, and a live load bar — so you manage the whole fleet at
// a glance and watch it run. Pure React chrome: reads the ~3 Hz stats snapshot, writes via Game methods
// (AssignTrainset). No sim mutation here, no per-frame work — the load bar tracks the snapshot, not rAF.
import { useState } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { RAIL_ROSTER, SIM_MS_PER_CLOCK_MIN, fmtMoney, hex, modeIcon } from "./shared";
import type { PerLine } from "../../types";

// The FLEET panel face — a brushed-graphite console (.ot-console owns bg/border/shadow/radius/text).
const PANEL: React.CSSProperties = {
  position: "fixed",
  top: 56,
  right: 252, // left of the Editor column
  width: 280,
  maxHeight: "70vh",
  overflowY: "auto",
  zIndex: 9,
  font: "13px system-ui,sans-serif",
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
  // #25 clamp only the FLOOR — AssignTrainset clamps the STORED count to the sim's fixed MAX_TRAINS_PER_LINE
  // (=24, world.rs) on apply, and the snapshot reads that stored count back, so the Fleet stepper and the Editor
  // stay in sync (was a 24-vs-8 magic-max divergence that read as a bug). Pressing + past 24 is a no-op. NOTE:
  // on a shared single-track block the runtime DISPATCH cap (dispatch.rs cross_cap) may run fewer than shown —
  // that cap bounds vehicles dispatched, not the stored trainset count, so the displayed number can exceed it.
  const setCount = (n: number) => game.assignTrainset(l.lineId, Math.max(1, n));
  return (
    <div data-testid={`fleet-row-${l.lineId}`} style={{ padding: "8px 10px", borderTop: "1px solid rgba(255,255,255,.08)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
        <span style={{ width: 22, height: 16, borderRadius: 4, background: hex(l.color), flex: "0 0 auto" }} />
        <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontWeight: 600 }} title={l.name}>
          {l.name || `Line ${l.lineId + 1}`}
        </span>
        {hasTrains ? (
          // Inline fleet-size editor — build/edit trainsets directly without opening the line.
          <span style={{ display: "flex", alignItems: "center", gap: 3, flex: "0 0 auto" }}>
            <button data-testid={`fleet-dec-${l.lineId}`} className="ot-key" onClick={() => setCount(l.trains - 1)} style={stepBtn}>−</button>
            <span data-testid={`fleet-count-${l.lineId}`} style={{ minWidth: 28, textAlign: "center", fontVariantNumeric: "tabular-nums", fontWeight: 700 }}>{l.trains}🚆</span>
            <button data-testid={`fleet-inc-${l.lineId}`} className="ot-key" onClick={() => setCount(l.trains + 1)} style={stepBtn}>+</button>
          </span>
        ) : (
          <button data-testid={`fleet-assign-${l.lineId}`} className="ot-key" onClick={() => setCount(2)} style={{ ...stepBtn, width: "auto", padding: "2px 8px" }}>+ trains</button>
        )}
      </div>
      <div style={{ marginLeft: 29, marginTop: 4, display: "flex", alignItems: "center", gap: 8, color: "var(--ot-con-ink-dim)", fontSize: 11 }}>
        <span>{modelName}</span>
        <span style={{ color: "rgba(255,255,255,.18)" }}>·</span>
        <span>{Math.max(1, Math.round(l.headwayMs / SIM_MS_PER_CLOCK_MIN))} min</span>
        {hasTrains && (
          <>
            <span style={{ color: "rgba(255,255,255,.18)" }}>·</span>
            {/* Live load bar — fills + colours with the fleet's mean load while running (view-mode status).
                Track is a recessed well; the fill keeps its semantic load colour. */}
            <span style={{ flex: 1, height: 6, background: "var(--ot-well-bg)", boxShadow: "var(--ot-well)", borderRadius: 3, overflow: "hidden" }} title={`Load ${Math.round(lf * 100)}%`}>
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
              className={`ot-key ${spec === i ? "on" : ""}`}
              title={`${m.name} — ${m.capacity} cap · ${m.kmh} km/h · ${fmtMoney(m.cost)}`}
              onClick={() => game.setAircraft(l.lineId, i)}
              style={{
                flex: 1,
                padding: "3px 0",
                font: "600 11px system-ui",
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

// Layout-only for the ± / "+ trains" KEYS; the raised-button look comes from .ot-key (size/font stay here).
const stepBtn: React.CSSProperties = {
  width: 22,
  height: 22,
  font: "700 14px system-ui",
  cursor: "pointer",
  lineHeight: 1,
};

/** The Fleet rows + footer hint, with no panel chrome — embedded into the bottom Outliner's Fleet
 *  tab (stage 6). The legacy floating `Fleet` toggle/panel is retired; the dock tab IS the fleet view. */
export function FleetBody() {
  const stats = useStats();
  const ui = useGameUI();
  const lines = stats.perLine.filter((l) => l.stops >= 2);
  const running = stats.running;
  return (
    <div data-testid="fleet-panel" style={{ height: "100%", overflowY: "auto", font: "13px system-ui,sans-serif", color: "var(--ot-con-ink)" }}>
      {lines.length === 0 ? (
        <div style={{ padding: "8px 12px", color: "var(--ot-con-ink-dim)", fontSize: 12 }}>Draw a line (≥ 2 stops), then assign trains here.</div>
      ) : (
        <div style={{ paddingBottom: 6 }}>
          {lines.map((l) => (
            <FleetRow key={l.lineId} l={l} running={running} />
          ))}
        </div>
      )}
      <div style={{ padding: "6px 12px 8px", color: "var(--ot-con-ink-dim)", fontSize: 11, lineHeight: 1.35, borderTop: "1px solid rgba(255,255,255,.08)" }}>
        {ui.ruleset === "arcadia" ? "Heavier carts haul more; express carts run faster." : "± sets fleet size · pick a model · the bar shows live load."}
      </div>
    </div>
  );
}

// Legacy floating Fleet panel — retired in stage 6 (migrated into the Outliner's Fleet tab). Kept
// only for reference of the toggle/total chrome; no longer mounted by App.
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
        className={`ot-key ${open ? "on" : ""}`}
        onClick={() => setOpen((o) => !o)}
        title="Fleet — view, build + edit every line's trainsets, with live status"
        style={{
          position: "fixed",
          top: 16,
          right: 14,
          zIndex: 10,
          padding: "5px 11px",
          font: "700 13px system-ui,sans-serif",
          cursor: "pointer",
        }}
      >
        🚆 Fleet{totalTrains > 0 ? ` · ${totalTrains}` : ""}
      </button>
      {open && (
        <div data-testid="fleet-panel" className="ot-console" style={PANEL}>
          <div style={{ padding: "10px 12px 6px", fontWeight: 700, display: "flex", justifyContent: "space-between", alignItems: "center", color: "var(--ot-con-ink)" }}>
            <span>🚆 Fleet {running ? "· live" : ""}</span>
            <button data-testid="fleet-close" onClick={() => setOpen(false)} style={{ border: 0, background: "transparent", cursor: "pointer", fontSize: 16, color: "var(--ot-con-ink-dim)" }}>×</button>
          </div>
          {lines.length === 0 ? (
            <div style={{ padding: "4px 12px 14px", color: "var(--ot-con-ink-dim)", fontSize: 12 }}>Draw a line (≥ 2 stops), then assign trains here.</div>
          ) : (
            <div style={{ paddingBottom: 8 }}>
              {lines.map((l) => (
                <FleetRow key={l.lineId} l={l} running={running} />
              ))}
            </div>
          )}
          <div style={{ padding: "6px 12px 12px", color: "var(--ot-con-ink-dim)", fontSize: 11, lineHeight: 1.35, borderTop: "1px solid rgba(255,255,255,.08)" }}>
            {ui.ruleset === "arcadia" ? "Heavier carts haul more; express carts run faster." : "± sets fleet size · pick a model · the bar shows live load."}
          </div>
        </div>
      )}
    </>
  );
}
