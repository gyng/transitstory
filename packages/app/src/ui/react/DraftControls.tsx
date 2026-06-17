// Floating draft controls: while a line is being drawn, a small card anchored at the last
// placed stop offers a discoverable ✓ Place / ✕ Cancel (instead of relying on double-click /
// Enter), with a live cost + length + validity summary right where the eye is — not pinned to
// the bottom toolbar. Pure chrome over the existing draft: reads game.draft / draftPreview and
// projects the stop to screen via the map (the one sanctioned read of map.project for an anchor),
// calls game.commitDraft() / cancelDraft(). Repositions on game.onChange (the per-mousemove draft
// refresh — client-side, sub-100 ms), never on a sim tick.
import { useEffect, useReducer } from "react";
import { mmToLngLat } from "../../coords/geo";
import { useGame, useGameUI } from "./GameContext";

export function DraftControls() {
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

  // Anchor at the LAST placed stop (stable — not the moving cursor) projected to screen pixels.
  const lastId = game.draft[game.draft.length - 1];
  const sv = game.bridge.stationsView()[lastId];
  if (!sv) return null;
  const [lng, lat] = mmToLngLat([sv.xMm, sv.yMm]);
  const pt = game.map.project([lng, lat]);

  const p = game.draftPreview();
  const km = p.lengthKm < 10 ? p.lengthKm.toFixed(1) : Math.round(p.lengthKm).toString();
  const cost = p.goldCost > 0 ? `${p.goldCost}⬢` : p.costM >= 1000 ? `$${(p.costM / 1000).toFixed(1)}B` : `$${Math.round(p.costM)}M`;
  const short = p.shortM > 0 || p.goldShort > 0;
  const ready = game.draft.length >= 2 && !p.invalid && !short;
  // Extending a committed line: the seed terminus isn't a NEW stop — count and label accordingly.
  const extending = game.extendTarget !== null;
  const stops = extending ? p.stops - 1 : p.stops;

  return (
    <div
      data-testid="draft-controls"
      style={{
        position: "fixed",
        left: pt.x + 16,
        top: pt.y - 18,
        zIndex: 12,
        display: "flex",
        flexDirection: "column",
        gap: 6,
        pointerEvents: "none", // the card frame ignores clicks; only the buttons opt back in
      }}
    >
      {/* Live summary pill — floats with the route head. Diegetic readout strip: brushed-graphite
          well by default; lights danger-red only when the route is invalid / underfunded (semantic). */}
      <div
        className={p.invalid || short ? undefined : "ot-console"}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "5px 10px",
          borderRadius: 999,
          ...(p.invalid || short
            ? {
                background: "var(--ot-con-red)",
                border: "1px solid var(--ot-con-edge)",
                boxShadow: "var(--ot-con-elev)",
              }
            : null),
          color: "#fff",
          font: "600 12px system-ui,sans-serif",
          whiteSpace: "nowrap",
        }}
      >
        <span data-testid="draft-stops">
          {extending ? `+${stops} stop${stops === 1 ? "" : "s"}` : `${stops} stop${stops === 1 ? "" : "s"}`}
        </span>
        <span style={{ opacity: 0.5 }}>·</span>
        <span>~{km} km</span>
        {p.costM > 0 && (
          <>
            <span style={{ opacity: 0.5 }}>·</span>
            <span data-testid="draft-cost">{cost}</span>
          </>
        )}
        {p.invalid && <span style={{ marginLeft: 2 }}>⚠ over water</span>}
        {!p.invalid && short && <span data-testid="draft-short" style={{ marginLeft: 2 }}>⚠ ${Math.ceil(p.shortM)}M short</span>}
      </div>

      {/* Confirm / cancel — the discoverable commit (double-click / Enter still work). */}
      <div style={{ display: "flex", gap: 6, pointerEvents: "auto" }}>
        <button
          data-testid="draft-confirm"
          className={ready ? "ot-key on-good" : "ot-key"}
          onClick={() => game.commitDraft()}
          disabled={!ready}
          title={
            ready
              ? extending
                ? "Extend line"
                : "Place line"
              : p.invalid
                ? "Route crosses water — elevate, tunnel, or use a ferry"
                : short
                  ? `Not enough money — $${Math.ceil(p.shortM)}M short`
                  : extending
                    ? "Chain at least 1 new stop"
                    : "Add at least 2 stops"
          }
          style={{
            borderRadius: 8,
            padding: "6px 12px",
            font: "700 13px system-ui,sans-serif",
            cursor: ready ? "pointer" : "not-allowed",
            ...(ready ? null : { opacity: 0.45, filter: "saturate(0.4)" }),
          }}
        >
          ✓ {extending ? "Extend" : "Place"}
        </button>
        <button
          data-testid="draft-cancel"
          className="ot-key"
          onClick={() => game.stopBuilding()}
          title="Cancel (Esc)"
          style={{
            borderRadius: 8,
            padding: "6px 10px",
            font: "700 13px system-ui,sans-serif",
            cursor: "pointer",
          }}
        >
          ✕
        </button>
      </div>
    </div>
  );
}
