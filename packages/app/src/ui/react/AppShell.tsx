// The viewport CHROME SHELL: a CSS grid pinned over #map that owns the four rigid edges of the
// GSG/tycoon layout (top strip · left construction · right lens/inspector · bottom dock). The grid
// geometry never relayouts between rulesets — only the contents swap (AGENTS frontend: React owns
// DOM chrome only; the #map/deck overlay/rAF loop stay imperative and z-below this).
//
// Pointer discipline (mirrors the existing Toolbar pattern + the #ui > * rule in styles.css): the
// shell + every region wrapper is `pointer-events:none` so map drags pass straight through the gaps;
// only the `.ot-console` (and other interactive) children re-enable pointers. The shell is a direct
// child of #ui, so its `.ot-shell` class beats the `#ui > * { pointer-events:auto }` universal rule.
//
// STAGE 1: the grid + region wrappers exist structurally and host the EXISTING (position:fixed)
// chrome unchanged — proving the scaffold + pointer scoping without any behaviour change. Later
// stages move each group into its region cell as a real grid-flowed child.
import type { CSSProperties, ReactNode } from "react";

const SHELL_STYLE: CSSProperties = {
  position: "fixed",
  inset: 0,
  display: "grid",
  // The four rigid edges: a 44px top strip, the map filling the middle, a 92px bottom dock; the
  // left/right edges size to their content (`auto`) so the map keeps the centre column.
  gridTemplateRows: "44px 1fr 92px",
  gridTemplateColumns: "auto 1fr auto",
  pointerEvents: "none",
  zIndex: 6, // above #map's deck overlay (z-below), the floating chrome still uses its own higher z
};

/** Top strip — spans all three columns (row 1). Resources (L) · alerts (C) · time (R). */
function TopStrip({ children }: { children?: ReactNode }) {
  return (
    <div
      data-region="top"
      style={{
        gridRow: 1,
        gridColumn: "1 / 4",
        display: "flex",
        alignItems: "stretch",
        pointerEvents: "none",
      }}
    >
      {children}
    </div>
  );
}

/** Left edge cell (row 2, col 1) — construction rail. */
function LeftEdge({ children }: { children?: ReactNode }) {
  return (
    <div data-region="left" style={{ gridRow: 2, gridColumn: 1, pointerEvents: "none" }}>
      {children}
    </div>
  );
}

/** Right edge cell (row 2, col 3) — inspector + lens rail. */
function RightEdge({ children }: { children?: ReactNode }) {
  return (
    <div data-region="right" style={{ gridRow: 2, gridColumn: 3, pointerEvents: "none" }}>
      {children}
    </div>
  );
}

/** Bottom edge — spans all three columns (row 3). Roster ⅔ + ticker ⅓. */
function BottomEdge({ children }: { children?: ReactNode }) {
  return (
    <div
      data-region="bottom"
      style={{ gridRow: 3, gridColumn: "1 / 4", pointerEvents: "none" }}
    >
      {children}
    </div>
  );
}

export function AppShell({
  top,
  left,
  right,
  bottom,
}: {
  top?: ReactNode;
  left?: ReactNode;
  right?: ReactNode;
  bottom?: ReactNode;
}) {
  return (
    <div className="ot-shell" style={SHELL_STYLE}>
      <TopStrip>{top}</TopStrip>
      <LeftEdge>{left}</LeftEdge>
      <RightEdge>{right}</RightEdge>
      <BottomEdge>{bottom}</BottomEdge>
    </div>
  );
}
