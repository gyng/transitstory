// Live build readout: while a route is being drawn, a pill shows the in-progress route's
// stops + length + validity (NIMBY/CS show length on the ghost; the $ cost is filled in by the
// sim cost-preview query). It subscribes DIRECTLY to game.onChange (a draft refreshes per
// mousemove, client-side, sub-100ms — no sim tick) rather than via a GameContext snapshot field,
// so it updates with the cursor without widening the shared UI snapshot.
import { useEffect, useReducer } from "react";
import { useGame, useGameUI } from "./GameContext";

export function BuildHud() {
  const game = useGame();
  const ui = useGameUI();
  const [, bump] = useReducer((n: number) => n + 1, 0);

  useEffect(() => {
    game.onChange.push(bump);
    return () => {
      const i = game.onChange.indexOf(bump);
      if (i >= 0) game.onChange.splice(i, 1);
    };
  }, [game]);

  if (ui.mode !== "build" || game.draft.length < 1) return null;

  const p = game.draftPreview();
  const km = p.lengthKm < 10 ? p.lengthKm.toFixed(1) : Math.round(p.lengthKm).toString();

  return (
    <div
      data-testid="build-hud"
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "6px 12px",
        background: p.invalid ? "rgba(214,40,40,.95)" : "rgba(28,32,36,.92)",
        color: "#fff",
        borderRadius: 999,
        font: "600 13px system-ui,sans-serif",
        boxShadow: "var(--ot-shadow)",
        pointerEvents: "none",
        whiteSpace: "nowrap",
        maxWidth: "92vw",
        overflow: "hidden",
        textOverflow: "ellipsis",
      }}
    >
      <span data-testid="build-hud-stops">{p.stops} stop{p.stops === 1 ? "" : "s"}</span>
      <span style={{ opacity: 0.55 }}>·</span>
      <span>~{km} km</span>
      {p.invalid && (
        <>
          <span style={{ opacity: 0.55 }}>·</span>
          <span>⚠ crosses water — elevate/tunnel, or use a ferry</span>
        </>
      )}
    </div>
  );
}
