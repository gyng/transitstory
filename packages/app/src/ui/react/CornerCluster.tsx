// BOTTOM-LEFT corner utility cluster (Fitts: a screen corner is an infinite-size target): the
// reversibility + meta controls that don't belong to a build category — undo/redo, the network
// dashboard, and settings. Lifted from the old top-left bar (undo/redo/dashboard) + the deleted
// #transport-bar (the ⚙ settings key). Each is a console KEY.
//
// The cluster owns only the BUTTONS + the open flags; the Settings + StatsDashboard MODALS are
// rendered by App as shell SIBLINGS (so they keep their natural #ui-level z above the grid shell,
// not trapped in the shell's stacking context). Reads the ui slice (history depths) so undo/redo
// enable/disable tracks the boundaries; writes via Game methods.
import type { CSSProperties } from "react";
import { useGame, useGameUI } from "./GameContext";
import { Button } from "./keys";

const CLUSTER_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: 7,
  pointerEvents: "auto",
};

export function CornerCluster({
  onOpenDashboard,
  onToggleSettings,
}: {
  onOpenDashboard: () => void;
  onToggleSettings: () => void;
}) {
  const game = useGame();
  useGameUI(); // re-render when history depths / selection move (undo/redo enable edges)

  return (
    <div data-testid="corner-cluster" className="ot-console" style={CLUSTER_STYLE}>
      <Button label="↶" testid="undo" title="Undo last action (Ctrl-Z)" onClick={() => game.undo()} disabled={!game.canUndo()} />
      {/* #25 Redo renders ALWAYS (greyed when empty, like Undo) so the cluster's width is stable — mounting it
          only when canRedo() shifted the adjacent 📊/⚙ keys under the cursor on undo-then-build (target-stability). */}
      <Button label="↷" testid="redo" title="Redo (Ctrl-Shift-Z / Ctrl-Y)" onClick={() => game.redo()} disabled={!game.canRedo()} />
      <Button label="📊" testid="open-dashboard" title="Network dashboard — ledger, ridership, satisfaction, trend charts" onClick={onOpenDashboard} />
      <Button label="⚙" testid="open-settings" title="Settings" onClick={onToggleSettings} />
    </div>
  );
}
