// #21 HOVER READOUT — a small bottom-centre status bar naming whatever is under the cursor: a node's brief
// label (★ The Capital — the realm's seat · ⛏ Ore — yield 100 · ⚔ Rival Hold) or the terrain biome
// (Plains · ♣ Forest · ⛰ Mountains · ≈ Water · ✦ Ley line). Recognition-over-recall: you can always read
// what you're pointing at without clicking.
//
// It subscribes to `game.onHover` — a DEDICATED callback the pointer fires on a hover CHANGE — NOT the ui
// slice, so a hover change (which can happen many times a second while panning) re-renders ONLY this tiny
// bar, never the toolbar/panels/roster. Renders nothing when over empty space (a quiet HUD).
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { useGame } from "./GameContext";

const STYLE: CSSProperties = {
  position: "fixed",
  left: "50%",
  bottom: 100, // just above the 92px bottom dock
  transform: "translateX(-50%)",
  padding: "3px 12px",
  font: "600 12px system-ui,sans-serif",
  color: "var(--ot-con-ink)",
  pointerEvents: "none", // never blocks a map drag
  whiteSpace: "nowrap",
  zIndex: 7, // above the shell (z6), below the modals
};

export function HoverReadout() {
  const game = useGame();
  const [label, setLabel] = useState<string | null>(game.hoverLabel);
  useEffect(() => {
    game.onHover = setLabel;
    setLabel(game.hoverLabel);
    return () => {
      if (game.onHover === setLabel) game.onHover = undefined;
    };
  }, [game]);
  if (!label) return null;
  return (
    <div data-testid="hover-readout" className="ot-console" style={STYLE}>
      {label}
    </div>
  );
}
