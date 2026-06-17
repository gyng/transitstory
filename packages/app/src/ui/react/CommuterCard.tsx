// "Commuter" card: click a station and meet a real rider waiting there. Under agent demand they're
// a NAMED citizen with a home and a workplace; under gravity flow they're an anonymous trip (just
// the route). Makes the abstract demand tangible — "Mei Tan, home near Tampines, works near Raffles
// Place, waited 4 min." Pure chrome: reads the selected station + samples one waiting rider via the
// read-only sim query, re-rendering on the ~3 Hz stats tick. Bottom-right (a free corner).
import { useEffect, useState } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { hex } from "./shared";

export function CommuterCard() {
  const game = useGame();
  const ui = useGameUI();
  useStats(); // re-render on the 3 Hz snapshot so wait time + the sampled rider stay live
  const [nth, setNth] = useState(0);

  // Reset the sample index whenever the selected station changes.
  useEffect(() => setNth(0), [ui.selectedStation]);

  if (ui.selectedStation === null) return null;
  const j = game.bridge.sampleJourney(ui.selectedStation, nth);
  if (!j) return null; // no one waiting here right now

  const who = j.anonymous ? "A commuter" : j.name;
  const wait = j.waitMin >= 1 ? `${Math.round(j.waitMin)} min` : "<1 min";

  return (
    <div
      data-testid="commuter-card"
      className="ot-console"
      style={{
        position: "fixed",
        right: 14,
        bottom: 14,
        zIndex: 11,
        width: 250,
        padding: "10px 12px",
        font: "12px system-ui,sans-serif",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
        <b data-testid="commuter-name" style={{ color: "var(--ot-con-ink)" }}>🧍 {who}</b>
        {j.queueLen > 1 && (
          <button
            data-testid="commuter-another"
            onClick={() => setNth((n) => n + 1)}
            title="Show another waiting rider"
            style={{ border: 0, background: "none", color: "var(--ot-con-accent)", cursor: "pointer", font: "11px system-ui" }}
          >
            {(nth % j.queueLen) + 1}/{j.queueLen} 🎲
          </button>
        )}
      </div>

      {!j.anonymous && (j.home || j.work) && (
        <div data-testid="commuter-homework" style={{ color: "var(--ot-con-ink-dim)", margin: "3px 0 5px" }}>
          🏠 {j.home || "—"} <span style={{ color: "var(--ot-con-ink-dim)" }}>→</span> 🏢 {j.work || "—"}
        </div>
      )}

      {/* The trip: where they are now → where they're going, with each leg's line. */}
      <div style={{ color: "var(--ot-con-ink-dim)", marginBottom: 4 }}>
        at <b style={{ color: "var(--ot-con-ink)" }}>{j.here}</b> · waited {wait}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
        {j.legs.map((leg, i) => (
          <div key={i} style={{ display: "flex", alignItems: "center", gap: 5, opacity: i < j.leg ? 0.45 : 1 }}>
            <span style={{ width: 11, height: 11, borderRadius: 3, flex: "none", background: hex(leg.lineColor) }} />
            <span style={{ flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
              {leg.lineName}
            </span>
            <span style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, whiteSpace: "nowrap" }}>→ {leg.alight}</span>
          </div>
        ))}
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 6 }}>
        <span style={{ color: "var(--ot-con-ink-dim)", fontSize: 11 }}>
          → <b style={{ color: "var(--ot-con-ink)" }}>{j.dest}</b>
        </span>
        {!j.anonymous && j.citizenId !== 0xffffffff && (
          <button
            data-testid="commuter-follow"
            className="ot-key on"
            onClick={() => game.setFollowed(j.citizenId)}
            title="Follow this commuter's whole journey"
            style={{
              padding: "4px 10px",
              font: "700 11px system-ui",
              cursor: "pointer",
            }}
          >
            👁 Follow
          </button>
        )}
      </div>
    </div>
  );
}
