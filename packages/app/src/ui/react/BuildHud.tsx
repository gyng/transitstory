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
  // Fantasy gold build economy prices in gold (⬢); transit prices in $. goldShort > 0 ⇒ unaffordable.
  const inGold = p.goldCost > 0;
  const cost = inGold ? `${p.goldCost}⬢` : p.costM >= 1000 ? `$${(p.costM / 1000).toFixed(1)}B` : `$${Math.round(p.costM)}M`;
  const unaffordable = p.invalid || p.goldShort > 0;
  // Bill of materials: per-terrain cell count + cost share (fantasy grid; the work the track entails).
  const total = inGold ? p.goldCost : Math.round(p.costM);
  const unit = inGold ? "⬢" : "M";
  const bom = p.stops >= 2 ? game.draftBom(total) : [];

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 5, pointerEvents: "none" }}>
      <div
        data-testid="build-hud"
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "6px 12px",
          // Floating console readout pill: graphite face + beveled edge. Unaffordable flips to the
          // semantic danger tone (red wash) so the warning still reads at a glance.
          background: unaffordable
            ? "linear-gradient(180deg, rgba(214,40,40,.96), rgba(168,28,28,.96))"
            : "linear-gradient(180deg, #2f343c, var(--ot-con-solid))",
          color: unaffordable ? "#fff" : "var(--ot-con-ink)",
          border: "1px solid var(--ot-con-edge)",
          borderRadius: 999,
          font: "600 13px system-ui,sans-serif",
          boxShadow: unaffordable
            ? "var(--ot-con-elev), 0 0 12px rgba(214,40,40,.4)"
            : "var(--ot-con-elev)",
          whiteSpace: "nowrap",
          maxWidth: "92vw",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        <span data-testid="build-hud-stops">{p.stops} stop{p.stops === 1 ? "" : "s"}</span>
        <span style={{ opacity: 0.55 }}>·</span>
        <span>~{km} km</span>
        {(p.costM > 0 || p.goldCost > 0) && (
          <>
            <span style={{ opacity: 0.55 }}>·</span>
            <span data-testid="build-hud-cost">{cost}</span>
          </>
        )}
        {p.goldShort > 0 && !p.invalid && (
          <>
            <span style={{ opacity: 0.55 }}>·</span>
            <span>⚠ {p.goldShort}⬢ short</span>
          </>
        )}
        {p.invalid && (
          <>
            <span style={{ opacity: 0.55 }}>·</span>
            <span>⚠ crosses water — elevate/tunnel, or use a ferry</span>
          </>
        )}
      </div>
      {/* Bill of materials: the work the track entails, per terrain (count + cost share). */}
      {bom.length > 0 && (
        <div
          data-testid="build-hud-bom"
          style={{ display: "flex", flexWrap: "wrap", justifyContent: "center", gap: 5, maxWidth: "92vw" }}
        >
          {bom.map((b) => (
            <span
              key={b.kind}
              data-testid={`bom-${b.kind}`}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 5,
                padding: "3px 9px",
                borderRadius: 999,
                // Console chip face: graphite solid + beveled edge, matching the readout pill above.
                background: "var(--ot-con-solid)",
                color: "var(--ot-con-ink)",
                border: "1px solid var(--ot-con-edge)",
                font: "600 11px system-ui,sans-serif",
                boxShadow: "var(--ot-con-elev)",
              }}
            >
              {/* per-terrain identity tint — keep its hue */}
              <span style={{ width: 8, height: 8, borderRadius: 2, background: `rgb(${b.tint})`, flex: "0 0 auto" }} />
              {b.count}× {b.kind}
              {b.cost > 0 && <span style={{ color: "var(--ot-con-ink-dim)" }}>{b.cost}{unit}</span>}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
