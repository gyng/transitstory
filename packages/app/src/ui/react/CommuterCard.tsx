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
      style={{
        position: "fixed",
        right: 14,
        bottom: 14,
        zIndex: 11,
        width: 250,
        padding: "10px 12px",
        borderRadius: 10,
        background: "rgba(255,255,255,.96)",
        boxShadow: "var(--ot-shadow, 0 2px 10px rgba(0,0,0,.12))",
        font: "12px system-ui,sans-serif",
        color: "#1c2024",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
        <b data-testid="commuter-name">🧍 {who}</b>
        {j.queueLen > 1 && (
          <button
            data-testid="commuter-another"
            onClick={() => setNth((n) => n + 1)}
            title="Show another waiting rider"
            style={{ border: 0, background: "none", color: "#0a8fcc", cursor: "pointer", font: "11px system-ui" }}
          >
            {(nth % j.queueLen) + 1}/{j.queueLen} 🎲
          </button>
        )}
      </div>

      {!j.anonymous && (j.home || j.work) && (
        <div data-testid="commuter-homework" style={{ color: "#5a626b", margin: "3px 0 5px" }}>
          🏠 {j.home || "—"} <span style={{ color: "#9aa3ad" }}>→</span> 🏢 {j.work || "—"}
        </div>
      )}

      {/* The trip: where they are now → where they're going, with each leg's line. */}
      <div style={{ color: "#7a818a", marginBottom: 4 }}>
        at <b style={{ color: "#1c2024" }}>{j.here}</b> · waited {wait}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
        {j.legs.map((leg, i) => (
          <div key={i} style={{ display: "flex", alignItems: "center", gap: 5, opacity: i < j.leg ? 0.45 : 1 }}>
            <span style={{ width: 11, height: 11, borderRadius: 3, flex: "none", background: hex(leg.lineColor) }} />
            <span style={{ flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
              {leg.lineName}
            </span>
            <span style={{ color: "#9aa3ad", fontSize: 11, whiteSpace: "nowrap" }}>→ {leg.alight}</span>
          </div>
        ))}
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 6 }}>
        <span style={{ color: "#9aa3ad", fontSize: 11 }}>
          → <b style={{ color: "#5a626b" }}>{j.dest}</b>
        </span>
        {!j.anonymous && j.citizenId !== 0xffffffff && (
          <button
            data-testid="commuter-follow"
            onClick={() => game.setFollowed(j.citizenId)}
            title="Follow this commuter's whole journey"
            style={{
              border: 0,
              borderRadius: 7,
              padding: "4px 10px",
              font: "700 11px system-ui",
              cursor: "pointer",
              background: "linear-gradient(180deg,#1ab6f0,#0a8fcc)",
              color: "#fff",
            }}
          >
            👁 Follow
          </button>
        )}
      </div>
    </div>
  );
}
