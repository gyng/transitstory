// RIGHT-EDGE inspector (stage 7): the one contextual properties panel for the current selection,
// docked immediately LEFT of the lens rail (inboard) so the two never share a column — resolving
// the stage 1-4 deviation where the EditorPanel (right:14) overlapped the LensRail (right:14).
//
// Progressive disclosure: EMPTY until something is selected, then shows ONLY the selected object's
// controls — a line's editor (name/color/trains/headway/track/build-mode/extend/branches/model+
// aircraft picker) or a bare station's platform stepper. All of that lives in Panels' Editor /
// StationEditor; this just flows them embedded in the right region cell (no position:fixed).
//
// AGENTS: React owns DOM chrome only; reads ui.selection (mount) + stats (readouts), writes via Game
// methods; headway commits on native `change` only (preserved in the embedded Editor). Below 1024px
// the parent makes this OVERLAY the lens rail (modal) — see App's right-region responsive wrapper.
import type { CSSProperties } from "react";
import { useGameUI } from "./GameContext";
import { Panels } from "./Panels";

const INSPECTOR_STYLE: CSSProperties = {
  // Sized to its content (the embedded panel sets its own width/scroll); flows in the right cell to
  // the LEFT of the lens rail. pointer-events re-enabled by the inner .ot-console (styles.css rule).
  display: "flex",
  alignItems: "flex-start",
  maxHeight: "100%",
  pointerEvents: "none",
};

export function Inspector() {
  const ui = useGameUI();
  // Progressive disclosure: render nothing until a line or bare station is selected.
  if (ui.selectedLine === null && ui.selectedStation === null) return null;
  return (
    <div data-testid="inspector" style={INSPECTOR_STYLE}>
      <Panels embedded />
    </div>
  );
}
