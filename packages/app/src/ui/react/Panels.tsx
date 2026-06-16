// Left LineListPanel (network roster) + right contextual EditorPanel (selected line's
// trainset + headway + track mode). Reads sim snapshots via hooks, emits Commands via Game.
// Headway slider fires one SetHeadway Command on `change` (drag-end); `input` only updates the
// live preview label (local useState) — never an optimistic self-render of sim state.
import { useCallback, useState } from "react";
import type { CSSProperties } from "react";
import type { PerLine, Stats } from "../../types";
import { useGame, useGameUI, useStats } from "./GameContext";
import { AIR_ROSTER, RAIL_ROSTER, railCarCount, PANEL_STYLE, SIM_MS_PER_CLOCK_MIN, hex, loadPip, modeIcon } from "./shared";
import { linePnl, lineSatisfaction, fmtSignedMoney, fmtCount, shortLineName, swatchInk } from "./lineEconomics";

// cssText token → React style object, so PANEL_STYLE stays the single source of truth (no
// duplicated literals) while remaining cssText-equivalent in JSX.
function cssToStyle(css: string): CSSProperties {
  const out: Record<string, string> = {};
  for (const decl of css.split(";")) {
    const i = decl.indexOf(":");
    if (i < 0) continue;
    const prop = decl.slice(0, i).trim();
    const val = decl.slice(i + 1).trim();
    if (!prop) continue;
    const camel = prop.replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());
    out[camel] = val;
  }
  return out as CSSProperties;
}

const LIST_STYLE: CSSProperties = {
  ...cssToStyle(PANEL_STYLE),
  top: "56px",
  left: "14px",
  width: "220px",
  padding: "8px 4px",
  maxHeight: "56vh",
  overflow: "auto",
};

const EDITOR_STYLE: CSSProperties = {
  ...cssToStyle(PANEL_STYLE),
  // Stack below the top-right Objectives card when present (it publishes --ot-objective-h);
  // 0px otherwise, so the panel sits at the top when no scenario is active.
  top: "calc(56px + var(--ot-objective-h))",
  right: "14px",
  width: "230px",
  padding: "12px",
};

// One roster row — a tight 2-line card (3 with the economy on). Scan order: a colour-coded line
// BADGE (identity anchor, colour+code so it's never hue-alone) + shortened name on line 1, with
// the ONE primary metric — ridership — right-aligned in a tabular-nums column the eye runs down.
// Line 2 is the single health chip (loadPip shape+word — the one health signal, satisfaction folds
// into its tooltip + the Editor) and a muted, abbreviated service line. P&L is a subordinate 3rd
// line shown ONLY when the economy is on (it's meaningless noise when off). Detail lives in the
// Editor on select (progressive disclosure). All data-testids are preserved (e2e contract).
function LineRow({ l, s, selected }: { l: PerLine; s: Stats; selected: boolean }) {
  const game = useGame();
  const { code, short } = shortLineName(l.name, l.lineId);
  const hasTrains = l.trains > 0;
  const pip = hasTrains ? loadPip(l.loadFactor) : null;
  const sat = lineSatisfaction(l, game.lineQueue(l.lineId)); // folded into the chip tooltip + Editor; no competing glyph here
  const pnl = linePnl(l, s);
  const showPnl = s.economyEnabled && hasTrains;
  const headwayMin = Math.max(1, Math.round(l.headwayMs / SIM_MS_PER_CLOCK_MIN));
  // Abbreviated service shape: "12 · 4tr · 5min" (mode glyph prefix only where it discriminates).
  const freq = `${l.mode !== 0 ? `${modeIcon(l.mode)} ` : ""}${l.stops} · ${l.trains}tr · ${headwayMin}min`;
  const chipTitle = pip
    ? `Load ${pip.pct}% — ${pip.word}${sat ? ` · rider satisfaction ${sat.score}% (${sat.word})` : ""}`
    : "";
  return (
    <div
      data-testid={`line-row-${l.lineId}`}
      onClick={() => game.selectLine(l.lineId)}
      style={{
        padding: "7px 8px",
        cursor: "pointer",
        borderTop: "1px solid #eef0f2",
        background: selected ? "#eef4fb" : "transparent",
        // selection reinforces identity with a left rail in the line's OWN colour (no new mark)
        boxShadow: selected ? `inset 3px 0 0 ${hex(l.color)}` : "none",
        opacity: hasTrains ? 1 : 0.92,
      }}
    >
      {/* line 1: colour-code badge + short name + ridership (the primary scan column) */}
      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
        <span
          aria-hidden
          style={{
            flex: "0 0 auto",
            width: 26,
            height: 18,
            borderRadius: 4,
            background: hex(l.color),
            color: swatchInk(l.color),
            font: "700 11px system-ui,sans-serif",
            letterSpacing: 0.2,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          {code}
        </span>
        <span
          style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", font: "600 13px system-ui,sans-serif", color: "#1c2024" }}
          title={l.name || `Line ${l.lineId + 1}`}
        >
          {short}
        </span>
        <span
          data-testid={`line-ridership-${l.lineId}`}
          title="riders carried"
          style={{
            flex: "0 0 auto",
            minWidth: 46,
            textAlign: "right",
            font: "700 13px system-ui,sans-serif",
            fontVariantNumeric: "tabular-nums",
            color: hasTrains ? "#1c2024" : "#9aa1a9",
          }}
        >
          {hasTrains ? fmtCount(l.ridership) : "—"}
        </span>
      </div>

      {/* line 2: the ONE health chip (shape + word — colour-blind-safe) · muted service shape;
          or, with no trains, a quiet amber "connect me" nudge (never the old screaming red). */}
      <div
        data-testid={`line-meta-${l.lineId}`}
        style={{ marginLeft: 33, marginTop: 3, font: "11px system-ui,sans-serif", color: "#9aa1a9", display: "flex", alignItems: "center", gap: 6, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}
      >
        {pip ? (
          <>
            <span data-testid={`line-load-pip-${l.lineId}`} title={chipTitle} style={{ color: pip.color, fontWeight: 600, flex: "0 0 auto" }}>
              {pip.glyph} {pip.word} {pip.pct}%
              {sat && (
                // satisfaction SCORE preserved for the e2e contract, folded into the chip (no glyph)
                <span data-testid={`line-sat-${l.lineId}`} style={{ position: "absolute", width: 0, height: 0, overflow: "hidden" }}>
                  {sat.score}
                </span>
              )}
            </span>
            <span style={{ color: "#cfd4da" }}>·</span>
            <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{freq}</span>
          </>
        ) : (
          <span style={{ color: "#b88a00", fontWeight: 500 }}>{l.stops} stops · no trains yet</span>
        )}
      </div>

      {/* line 3: P&L — subordinate, and ONLY when the economy is on (otherwise it's meaningless
          noise: fares are $0, so it would read a bold −$buildcost on every line). */}
      {showPnl && (
        <div style={{ marginLeft: 33, marginTop: 3 }}>
          <span
            data-testid={`line-pnl-${l.lineId}`}
            title={`Fares earned $${Math.round(pnl.revenue).toLocaleString()} − build cost $${Math.round(pnl.capital).toLocaleString()}`}
            style={{ font: "600 11px system-ui,sans-serif", color: pnl.inBlack ? "var(--ot-gauge-good,#009e73)" : "var(--ot-gauge-bad,#d62828)" }}
          >
            {pnl.inBlack ? "▲" : "▼"} {fmtSignedMoney(pnl.net)}
          </span>
        </div>
      )}
    </div>
  );
}

function LineList() {
  const ui = useGameUI();
  const stats = useStats();
  const lines = stats.perLine;
  return (
    <div id="line-list" data-testid="line-list" style={LIST_STYLE}>
      <div style={{ display: "flex", alignItems: "baseline", padding: "0 8px 6px" }}>
        <span style={{ fontWeight: 700 }}>Lines</span>
        {lines.length > 0 && <span style={{ marginLeft: 6, color: "#9aa1a9", fontSize: 11 }}>{lines.length}</span>}
        <span style={{ marginLeft: "auto", color: "#b3b9c0", fontSize: 10, letterSpacing: 0.3 }}>riders</span>
      </div>
      {lines.length === 0 ? (
        <div style={{ color: "#7a818a", padding: "0 8px" }}>No lines yet — draw one with the ╱ Line tool.</div>
      ) : (
        lines.map((l) => <LineRow key={l.lineId} l={l} s={stats} selected={ui.selectedLine === l.lineId} />)
      )}
    </div>
  );
}

function Editor({ l }: { l: PerLine }) {
  const game = useGame();
  const stats = useStats();
  const id = l.lineId;
  const mins = Math.max(2, Math.min(20, Math.round(l.headwayMs / SIM_MS_PER_CLOCK_MIN)));
  // Local preview of the headway slider — `input` updates only this label; `change` commits.
  const [previewMins, setPreviewMins] = useState<number | null>(null);

  // React's onChange maps to the DOM `input` event, which would commit a Command on every drag
  // tick. To preserve the vanilla `input`(preview) / `change`(drag-end commit) split exactly, we
  // attach real native listeners via callback refs (mirrors panels.ts addEventListener 1:1).
  const sliderRef = useCallback(
    (el: HTMLInputElement | null) => {
      if (!el) return;
      el.oninput = () => setPreviewMins(Number(el.value)); // preview only
      el.onchange = () => game.setHeadwayMs(id, Number(el.value) * SIM_MS_PER_CLOCK_MIN); // commit
    },
    [game, id],
  );
  const trainsRef = useCallback(
    (el: HTMLInputElement | null) => {
      if (!el) return;
      el.onchange = () => game.assignTrainset(id, Math.max(1, Math.min(8, Number(el.value) | 0)));
    },
    [game, id],
  );

  // Track mode (whole line): Surface / Elevated / Tunnel — the built-environment lever.
  // Only rail builds track; bus rides roads, ferry/air have no grade to choose.
  const lv = game.bridge.linesView().find((x) => x.id === id);
  const modes = lv?.spanModes ?? [];
  const allMode = modes.length && modes.every((m) => m === modes[0]) ? modes[0] : -1;
  const tracks = lv?.trackTypes ?? [];
  const allTrack = tracks.length && tracks.every((t) => t === tracks[0]) ? tracks[0] : -1;
  const tight = lv ? lv.minRadiusMm < 100_000 : false;
  const isRail = l.mode === 0;

  const header = (
    <div style={{ fontWeight: 700, marginBottom: "8px" }}>
      {modeIcon(l.mode)} {l.name}
    </div>
  );

  if (l.trains === 0) {
    return (
      <div id="editor-panel" data-testid="editor-panel" style={EDITOR_STYLE}>
        {header}
        <button
          data-testid="assign-trainset"
          onClick={() => game.assignTrainset(id, 2)}
          style={{
            width: "100%",
            padding: "8px",
            border: 0,
            borderRadius: "7px",
            background: "#0072b2",
            color: "#fff",
            font: "600 13px system-ui",
            cursor: "pointer",
          }}
        >
          ▶ Assign trainset
        </button>
        <div style={{ color: "#7a818a", marginTop: "6px" }}>Adds trains and auto-suggests a headway.</div>
      </div>
    );
  }

  const shownMins = previewMins ?? mins;
  // trains > 0 here (the trains===0 branch returned above), so the line always has a load reading.
  const pip = loadPip(l.loadFactor);

  return (
    <div id="editor-panel" data-testid="editor-panel" style={EDITOR_STYLE}>
      {header}
      <label style={{ display: "flex", justifyContent: "space-between", alignItems: "center", margin: "6px 0" }}>
        Trains{" "}
        <input
          key={l.trains}
          ref={trainsRef}
          data-testid="trains-input"
          type="number"
          min="1"
          max="8"
          defaultValue={l.trains}
          style={{ width: "56px", padding: "4px" }}
        />
      </label>
      <label style={{ display: "block", marginTop: "10px" }}>
        Headway: <b data-testid="headway-label">{shownMins} min</b>
      </label>
      <input
        key={mins}
        ref={sliderRef}
        data-testid="headway-slider"
        type="range"
        min="1"
        max="30"
        step="1"
        defaultValue={mins}
        style={{ width: "100%" }}
      />
      <div style={{ color: "#7a818a", margin: "4px 0 10px" }}>Capacity × frequency are your two levers.</div>

      {/* Extend the line from either terminus: arms the line tool with a draft seeded at that
          end (the ghost takes the line's colour). Loop lines have no termini — no buttons. */}
      {(() => {
        const lv = game.bridge.linesView()[id];
        if (!lv || lv.removed || lv.loopLine || lv.stops.length < 2) return null;
        const sv = game.bridge.stationsView();
        const name = (sid: number) => sv[sid]?.name || `Station ${sid + 1}`;
        const endBtn: CSSProperties = {
          flex: 1,
          padding: "5px 4px",
          border: "1px solid #d7dade",
          borderRadius: 7,
          background: "#fff",
          font: "600 11px system-ui",
          cursor: "pointer",
          color: "#1c2024",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        };
        return (
          <div style={{ marginBottom: "10px" }}>
            <div style={{ fontWeight: 600, marginBottom: "4px" }}>Extend line</div>
            <div style={{ display: "flex", gap: 6 }}>
              <button data-testid="extend-head" style={endBtn} title={`Extend from ${name(lv.stops[0])}`} onClick={() => game.startExtend(id, true)}>
                ⇠ {name(lv.stops[0])}
              </button>
              <button data-testid="extend-tail" style={endBtn} title={`Extend from ${name(lv.stops[lv.stops.length - 1])}`} onClick={() => game.startExtend(id, false)}>
                {name(lv.stops[lv.stops.length - 1])} ⇢
              </button>
            </div>
            <div style={{ color: "#9aa3ad", fontSize: 11, marginTop: 3 }}>
              …or press a terminus with the ╱ Line tool. Right-click a station to add it mid-line.
            </div>
          </div>
        );
      })()}

      {/* Aircraft picker (AIR lines): the roster ladder is THE capacity lever for a route —
          a bigger jet lifts more per departure but turns slower at the gate (wider effective
          headway). Index = AssignTrainset.spec; the sim's clamp keeps anything sane. */}
      {l.mode === 3 && (
        <div data-testid="aircraft-picker" style={{ marginBottom: "10px" }}>
          <div style={{ fontWeight: 600, marginBottom: "4px" }}>Aircraft</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {AIR_ROSTER.map((a, i) => {
              const sel = (l.trainsetSpec ?? 0) === i;
              return (
                <button
                  key={a.name}
                  data-testid={`aircraft-${i}`}
                  title={a.blurb}
                  onClick={() => game.setAircraft(id, i)}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 8,
                    padding: "5px 8px",
                    border: sel ? "1px solid #0072b2" : "1px solid #d7dade",
                    borderRadius: 7,
                    background: sel ? "#eef4fb" : "#fff",
                    font: "600 12px system-ui",
                    color: "#1c2024",
                    cursor: "pointer",
                  }}
                >
                  <span>{sel ? "✓ " : ""}{a.name}</span>
                  <span style={{ color: "#7a818a", fontWeight: 400 }}>{a.capacity} seats · {a.turnMin} min turn</span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* Train-model picker (RAIL lines): the depot rework's catalog — buy Standard / Heavy / Express.
          A real capacity ⇄ speed ⇄ cost tradeoff: Heavy hauls twice the load but is slower + pricier,
          Express is fast + cheap but light. Index = AssignTrainset.spec; the sim clamps anything sane. */}
      {l.mode === 0 && (
        <div data-testid="model-picker" style={{ marginBottom: "10px" }}>
          <div style={{ fontWeight: 600, marginBottom: "4px" }}>Train model</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {RAIL_ROSTER.map((m, i) => {
              const sel = (l.trainsetSpec ?? 0) === i;
              return (
                <button
                  key={m.name}
                  data-testid={`model-${i}`}
                  title={m.blurb}
                  onClick={() => game.setAircraft(id, i)}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 8,
                    padding: "5px 8px",
                    border: sel ? "1px solid #0072b2" : "1px solid #d7dade",
                    borderRadius: 7,
                    background: sel ? "#eef4fb" : "#fff",
                    font: "600 12px system-ui",
                    color: "#1c2024",
                    cursor: "pointer",
                  }}
                >
                  <span>{sel ? "✓ " : ""}{m.name}</span>
                  <span style={{ color: "#7a818a", fontWeight: 400 }}>{railCarCount(m.capacity)} cars · {m.capacity} cap · {m.kmh} km/h · ${m.costM}M</span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div data-testid="line-performance" style={{ marginBottom: "10px" }}>
        <div style={{ fontWeight: 600, marginBottom: "4px" }}>Performance</div>
        <div data-testid="line-load" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ color: "#7a818a" }}>Load</span>
          <b style={{ color: pip.color }}>
            {pip.glyph} {pip.pct}% · {pip.word}
          </b>
        </div>
        {/* Rider satisfaction — the full readout (face + word + score) lives HERE (progressive
            disclosure); the roster shows only the load chip, with the score in its tooltip. */}
        {(() => {
          const sat = lineSatisfaction(l, game.lineQueue(l.lineId));
          return sat ? (
            <div data-testid="line-satisfaction" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: "3px" }}>
              <span style={{ color: "#7a818a" }}>Satisfaction</span>
              <b style={{ color: sat.color }}>
                {sat.glyph} {sat.score}% · {sat.word}
              </b>
            </div>
          ) : null;
        })()}
        {/* `line-demand-served` (Serves N% of nearby demand) lands with the OSM demand-model
            pass, gated on a new PerLine field — not added here to avoid double-owning it. */}
      </div>

      {isRail && (
        <div>
          <div style={{ fontWeight: 600, marginBottom: "4px" }}>Track</div>
          <div style={{ display: "flex", gap: "4px" }}>
            {(["Surface", "Elevated", "Tunnel"] as const).map((label, m) => {
              const on = allMode === m;
              return (
                <button
                  key={m}
                  data-testid={`mode-${m}`}
                  onClick={() => game.setLineMode(id, m)}
                  style={{
                    flex: 1,
                    padding: "5px",
                    borderRadius: "6px",
                    border: "1px solid #d7dade",
                    cursor: "pointer",
                    font: "600 12px system-ui",
                    ...(on ? { background: "#0072b2", color: "#fff" } : { background: "#fff", color: "#1c2024" }),
                  }}
                >
                  {label}
                </button>
              );
            })}
          </div>
          {/* Track type (P2): Single track is cheaper to build but lower capacity — opposing trains
              must meet at passing places (stations). HIDDEN in arcadia: all fantasy track is forced SINGLE
              (readability + it makes meets/signalling matter), so there's no Double/Single choice to offer. */}
          {stats.ruleset !== "arcadia" ? (
            <div style={{ display: "flex", gap: "4px", marginTop: "4px" }}>
              {([0, 1] as const).map((t) => {
                const on = allTrack === t;
                return (
                  <button
                    key={t}
                    data-testid={`track-${t}`}
                    title={t === 1 ? "Single track: ~half the cost, lower capacity (trains meet at stations)" : "Double track: full capacity"}
                    onClick={() => game.setLineTrack(id, t)}
                    style={{
                      flex: 1,
                      padding: "5px",
                      borderRadius: "6px",
                      border: "1px solid #d7dade",
                      cursor: "pointer",
                      font: "600 12px system-ui",
                      ...(on ? { background: "#5a3e85", color: "#fff" } : { background: "#fff", color: "#1c2024" }),
                    }}
                  >
                    {t === 1 ? "Single track" : "Double track"}
                  </button>
                );
              })}
            </div>
          ) : (
            <div style={{ marginTop: "4px", color: "#7a818a", fontSize: 11 }}>⚊ Single track — opposing carts meet at stations.</div>
          )}
        </div>
      )}

      {/* Branches (P3): each spur gets a terminus label, a per-branch Track toggle (rail), and a
          bulldoze. Only shown for a branched line (e.g. the Circle Line's Marina Bay spur). */}
      {(() => {
        const blv = game.bridge.linesView()[id];
        if (!blv || blv.removed || !blv.branchTermini || blv.branchTermini.length === 0) return null;
        const bsv = game.bridge.stationsView();
        const bn = (sid: number) => bsv[sid]?.name || `Station ${sid + 1}`;
        const sBtn = (on: boolean): CSSProperties => ({
          width: 22,
          padding: "3px 0",
          borderRadius: 5,
          border: "1px solid #d7dade",
          cursor: "pointer",
          font: "600 11px system-ui",
          ...(on ? { background: "#0072b2", color: "#fff" } : { background: "#fff", color: "#1c2024" }),
        });
        return (
          <div style={{ marginTop: 8 }}>
            <div style={{ fontWeight: 600, marginBottom: "4px" }}>Branches</div>
            {blv.branchTermini.map((term, bi) => (
              <div key={bi} data-testid={`branch-${bi}`} style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 5 }}>
                <span style={{ flex: 1, fontSize: 12, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={`Branch to ${bn(term)}`}>
                  ⑂ → {bn(term)}
                </span>
                {isRail &&
                  (["S", "E", "T"] as const).map((lbl, m) => (
                    <button
                      key={m}
                      data-testid={`branch-${bi}-mode-${m}`}
                      title={["Surface", "Elevated", "Tunnel"][m]}
                      onClick={() => game.setBranchMode(id, bi, m)}
                      style={sBtn(blv.branchModes[bi] === m)}
                    >
                      {lbl}
                    </button>
                  ))}
                <button
                  data-testid={`branch-${bi}-remove`}
                  title="Remove this branch"
                  onClick={() => game.removeBranch(id, bi)}
                  style={{ width: 22, padding: "3px 0", borderRadius: 5, border: "1px solid #e3b7b7", background: "#fff", color: "#c0392b", cursor: "pointer", font: "600 12px system-ui" }}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        );
      })()}

      <div data-testid="line-impact" style={{ marginTop: "8px", fontSize: "12px" }}>
        {l.crossesWater && (
          <div data-testid="water-warning" style={{ color: "#d62828", fontWeight: 600, marginBottom: "4px" }}>
            ⚠ Surface track crosses water — the line is parked (no trains run) until you Elevate or Tunnel.
          </div>
        )}
        {tight && <div style={{ color: "#e69f00" }}>⤳ Tight curves — trains slow here.</div>}
        <div style={{ color: "#7a818a" }}>
          Build impact: <b>{Math.round(l.disruption)}</b> · Cost:{" "}
          {stats.buildGoldDivisor > 0 ? (
            <b>{Math.round(l.capitalCost / stats.buildGoldDivisor)}⬢</b>
          ) : (
            <b>${Math.round(l.capitalCost / 1e6)}M</b>
          )}
        </div>
      </div>
    </div>
  );
}

export function Panels() {
  const ui = useGameUI();
  const perLine = useStats().perLine;
  const id = ui.selectedLine;
  const l = id === null ? undefined : perLine.find((x) => x.lineId === id);
  return (
    <>
      <LineList />
      {l ? <Editor key={l.lineId} l={l} /> : null}
    </>
  );
}
