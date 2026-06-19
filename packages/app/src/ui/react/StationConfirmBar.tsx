// Fantasy "confirm build": a small bar that appears when the Station tool has dropped a GHOST on a hex
// cell, asking the player to commit (✓ / Enter) or discard (✗ / Esc). Reads the `pendingStation` UI
// slice (set by Game.ghostStation); every action routes to an existing Game method (no new Command —
// the actual PlaceStation Command fires only on confirm). Build mode only; absent otherwise.
import type { CSSProperties } from "react";
import { useGame, useGameUI } from "./GameContext";

// Layout-only for the .ot-key console buttons — the raised-key look (bg/border/shadow/colour) is
// owned by the .ot-key class (#28 diegetic console theme); we only set sizing + flex here.
const BTN: CSSProperties = {
  pointerEvents: "auto",
  padding: "8px 14px",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: 6,
};
const KEY: CSSProperties = { font: `600 11px var(--ot-readout-font)`, opacity: 0.75 };

export function StationConfirmBar() {
  const game = useGame();
  const ui = useGameUI();
  if (!ui.pendingStation) return null;
  return (
    <div
      data-testid="station-confirm"
      className="ot-console"
      style={{
        position: "fixed",
        left: "50%",
        bottom: 96,
        transform: "translateX(-50%)",
        zIndex: 40,
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 12px 10px 16px",
        pointerEvents: "auto",
        font: "600 13px system-ui",
      }}
    >
      <span style={{ color: "var(--ot-con-ink)" }}>◉ Build a station on this hex?</span>
      <button
        data-testid="station-confirm-build"
        className="ot-key on-good"
        title="Build a station on this hex (Enter)" // #25 match the DraftControls line-confirm buttons' titles
        onClick={() => game.confirmPendingStation()}
        style={BTN}
      >
        ✓ Build <span style={KEY}>Enter</span>
      </button>
      <button
        data-testid="station-confirm-cancel"
        className="ot-key"
        title="Discard this ghost (Esc)"
        onClick={() => game.cancelPendingStation()}
        style={BTN}
      >
        ✗ Cancel <span style={KEY}>Esc</span>
      </button>
    </div>
  );
}
