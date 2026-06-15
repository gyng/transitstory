// Fantasy "confirm build": a small bar that appears when the Station tool has dropped a GHOST on a hex
// cell, asking the player to commit (✓ / Enter) or discard (✗ / Esc). Reads the `pendingStation` UI
// slice (set by Game.ghostStation); every action routes to an existing Game method (no new Command —
// the actual PlaceStation Command fires only on confirm). Build mode only; absent otherwise.
import type { CSSProperties } from "react";
import { useGame, useGameUI } from "./GameContext";

const BTN: CSSProperties = {
  pointerEvents: "auto",
  border: 0,
  borderRadius: 9,
  padding: "8px 14px",
  font: "700 13px system-ui",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: 6,
};
const KEY: CSSProperties = { font: "600 11px system-ui", opacity: 0.7 };

export function StationConfirmBar() {
  const game = useGame();
  const ui = useGameUI();
  if (!ui.pendingStation) return null;
  return (
    <div
      data-testid="station-confirm"
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
        background: "rgba(20,24,28,.97)",
        color: "#eef1f4",
        borderRadius: 12,
        boxShadow: "var(--ot-shadow,0 10px 30px rgba(0,0,0,.45))",
        border: "1px solid rgba(255,255,255,.08)",
        pointerEvents: "auto",
        font: "600 13px system-ui",
      }}
    >
      <span>◉ Build a station on this hex?</span>
      <button
        data-testid="station-confirm-build"
        onClick={() => game.confirmPendingStation()}
        style={{ ...BTN, background: "linear-gradient(180deg,#1ab6f0,#0a8fcc)", color: "#fff" }}
      >
        ✓ Build <span style={KEY}>Enter</span>
      </button>
      <button
        data-testid="station-confirm-cancel"
        onClick={() => game.cancelPendingStation()}
        style={{ ...BTN, background: "#2a323b", color: "#eef1f4" }}
      >
        ✗ Cancel <span style={KEY}>Esc</span>
      </button>
    </div>
  );
}
