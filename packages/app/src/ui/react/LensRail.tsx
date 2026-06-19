// RIGHT-EDGE lens rail (Cities:Skylines / SimCity info-view lenses): a thin VERTICAL column of
// additive overlay toggles + (arcadia) the exclusive map-LENS selector. Lifted out of the old bottom
// #transport-bar. A vertical column is height-bounded and NEVER wraps (the structural overflow fix),
// so adding a lens can't reflow a row off-screen.
//
//   • Additive layer toggles (layer-demand/reach/roads/peeps/signals): flip overlay visibility only —
//     never new geometry. Bind ui.show* flags; emit via game.setShow*.
//   • Map LENS (arcadia only, the lens-bar): an EXCLUSIVE one-of-N view-mode selector (realm/supply/
//     military/decadence) — the others dim in Game.composeAndSet. Binds ui.lens; emits game.setLens.
//
// AGENTS: React owns DOM chrome only; toggles read/write through Game; DOM rides the ui/stats slices.
import type { CSSProperties } from "react";
import { useGame, useGameUI } from "./GameContext";
import { Button } from "./keys";

const RAIL_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
  padding: 7,
  pointerEvents: "auto",
};

// A horizontal cut seam between key groups on the console face (used between the additive toggles
// and the exclusive lens selector).
const SEP_STYLE: CSSProperties = {
  height: 2,
  alignSelf: "stretch",
  background: "linear-gradient(90deg, rgba(0,0,0,.45), rgba(255,255,255,.05))",
  borderRadius: 1,
  margin: "3px 2px",
};

// The additive overlay toggles — vertical, full-width keys (label etched, .ot-key glow when on).
const KEY_STYLE: CSSProperties = { width: "100%", justifyContent: "flex-start", textAlign: "left" };

export function LensRail() {
  const game = useGame();
  const ui = useGameUI();
  const arcadia = ui.ruleset === "arcadia";

  return (
    <div data-testid="lens-rail" className="ot-console" style={RAIL_STYLE}>
      <Button
        label={arcadia ? "🌡 Supply" : "🌡 Demand"}
        testid="layer-demand"
        disabled={game.lensHides("demand")}
        gated
        title={
          game.lensHides("demand")
            ? `Demand is hidden by the ${ui.lens} lens — switch to the All lens to show it.`
            : arcadia
              ? "Supply demand: which TOWNS still need cargo, and how much is UNMET. 🟪 cold-violet = an unmet need — run rail to it and the hex cools as your carts deliver. (Resources produce, towns consume; this is the supply-chain heat, not commuter trips.)"
              : "Travel-demand heat: 🟧 warm = unserved (build here) · 🟦 cool = covered. Homes start trips, jobs pull them. Pin a station to see where its riders go."
        }
        onClick={() => game.setShowDemand(!ui.showDemand)}
        on={ui.showDemand}
        compact
        style={KEY_STYLE}
      />
      <Button
        label="🕐 Reach"
        testid="layer-reach"
        disabled={ui.selectedStation === null}
        gated
        title={ui.selectedStation === null ? "Reach needs a pinned station — click one first, then shade every other by how fast transit reaches it." : "Reach: shade every station by how fast transit gets there from the pinned one. Faster reach pulls more demand — extend it to unlock trips."}
        onClick={() => game.setShowReach(!ui.showReach)}
        on={ui.showReach}
        compact
        style={KEY_STYLE}
      />
      <Button
        label="🛣 Roads"
        testid="layer-roads"
        title="Road corridors where buses run cheap + fast. Turn on when planning a bus line — route along roads to cut cost and speed service."
        onClick={() => game.setShowRoads(!ui.showRoads)}
        on={ui.showRoads}
        compact
        style={KEY_STYLE}
      />
      <Button
        label="🧍 Peeps"
        testid="layer-peeps"
        title="Show individual riders: walking to the platform, waiting, riding the train, and heading out at their stop. Purely visual — no effect on the sim."
        onClick={() => game.setShowPeeps(!ui.showPeeps)}
        on={ui.showPeeps}
        compact
        style={KEY_STYLE}
      />
      {/* TTD signals (fantasy single-track): show each block's state so meets read at a glance. */}
      {arcadia && (
        <Button
          label="🚦 Signals"
          testid="layer-signals"
          title="Signal view: single-track block state — 🟢 clear · 🔴 occupied · 🟠 a cart held, waiting for the block ahead. Purely visual — shows WHY carts meet and wait on single track."
          onClick={() => game.setShowSignals(!ui.showSignals)}
          on={ui.showSignals}
          compact
          style={KEY_STYLE}
        />
      )}

      {/* Map LENSES (#5): an EXCLUSIVE arcadia-only view-mode selector — pick ONE reading; the others
          dim (filtered in Game.composeAndSet). A vertical SEGMENTED control (joined buttons, radio-like)
          so it reads as one-of-N — visually distinct from the additive layer-toggle keys above. */}
      {arcadia && (
        <>
          <span style={SEP_STYLE} />
          <span style={{ font: "700 10px var(--ot-readout-font)", letterSpacing: ".1em", color: "var(--ot-con-ink-dim)", padding: "0 2px" }}>LENS</span>
          <div data-testid="lens-bar" style={{ display: "flex", flexDirection: "column", borderRadius: 7, overflow: "hidden", boxShadow: "var(--ot-well)" }}>
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
                    borderBottom: "1px solid rgba(0,0,0,.4)",
                    padding: "6px 9px",
                    cursor: "pointer",
                    textAlign: "left",
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
    </div>
  );
}
