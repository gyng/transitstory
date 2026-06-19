// Bottom-left "Network" dashboard: surfaces the rich service-quality telemetry the sim now
// produces (journey/wait times, demand served, the full-train + renege pressure, and the
// per-mode ridership split) in one scannable card — the abstract-state-in-panels half of the
// AGENTS IA, so the top StatsBar can stay "one number + one gauge". Pure chrome: reads only the
// ~3 Hz `stats` slice, issues no Commands. Collapsible, but default-open so the info is visible
// (a discoverable channel, not a buried hover-tooltip).
import { useState, type CSSProperties } from "react";
import type { Stats } from "../../types";
import { useStats, useGameUI } from "./GameContext";
import { SIM_MS_PER_CLOCK_MIN, MODES } from "./shared";

// The travel-demand model in one line, keyed to the time of day: homes (trip origins) generate
// trips, jobs (destinations) attract them, and the rush direction flips AM↔PM (sim `tod::work_bias`).
// Driven off the sim's own `period` label so the arrow never disagrees with the clock.
function demandFlow(period: string): string {
  switch (period) {
    case "AM rush":
      return "🏠 → 💼  morning commute";
    case "PM rush":
    case "Evening":
      return "💼 → 🏠  heading home";
    case "Night":
      return "🌙  quiet";
    default:
      return "🏠 ⇄ 💼  both ways";
  }
}

// Heat-key swatch matching the demand overlay's colours (render.ts demandColor): warm = unserved,
// cool = covered. A small <span>, so the legend reads next to the live map layer.
function Swatch({ rgb }: { rgb: string }) {
  return (
    <span
      style={{ display: "inline-block", width: 10, height: 10, borderRadius: 3, background: rgb, verticalAlign: -1, marginRight: 5 }}
    />
  );
}

// Layout only — the brushed-graphite face (bg / border / bevel / light ink) comes from the
// `.ot-console` className on the panel containers (#28 diegetic theme).
const CARD: CSSProperties = {
  position: "fixed",
  left: 14,
  bottom: 14,
  zIndex: 9,
  width: 248,
  font: "12px system-ui,sans-serif",
  overflow: "hidden",
};

const fmtMin = (ms: number): string => (ms > 0 ? `${(ms / SIM_MS_PER_CLOCK_MIN).toFixed(1)} min` : "—"); // clock minutes

/** One label/value row; `tone` colours the value for pressure readouts. */
function Row({ label, value, tone, testid }: { label: string; value: string; tone?: string; testid?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "2px 0" }}>
      <span style={{ color: "var(--ot-con-ink-dim)" }}>{label}</span>
      <b data-testid={testid} style={{ fontVariantNumeric: "tabular-nums", color: tone ?? "var(--ot-con-ink)" }}>
        {value}
      </b>
    </div>
  );
}

function Divider() {
  return <div style={{ height: 1, background: "rgba(255,255,255,.08)", margin: "6px 0" }} />;
}

/** The fantasy (arcadia) realm panel — the supply→conquest→decadence ledger, replacing the transit
 *  service telemetry. Reuses the same CARD/Row chrome so the two modes read alike. `embedded` (stage 6)
 *  drops the fixed card so it flows in the Outliner's Report tab. */
function FantasyServiceReport({ s, embedded }: { s: Stats; embedded?: boolean }) {
  const decay = Math.round(s.decadencePct);
  const decayColor = decay >= 66 ? "var(--ot-gauge-bad,#d62828)" : decay >= 33 ? "#e69f00" : "var(--ot-con-ink-dim)";
  const wrap: CSSProperties = embedded
    ? { height: "100%", overflowY: "auto", font: "12px system-ui,sans-serif", color: "var(--ot-con-ink)" }
    : CARD;
  return (
    <div data-testid="service-report" className={embedded ? undefined : "ot-console"} style={wrap}>
      <div style={{ padding: "10px 12px 6px", fontWeight: 700, color: "var(--ot-con-ink)" }}>⚜ The Realm</div>
      <div style={{ padding: "0 12px 12px" }}>
        <Row label="Tribute" value={`${Math.round(s.tribute)}`} testid="svc-tribute" />
        {s.goldUpkeepDaily > 0 && (
          <Row label="Upkeep" value={`−${Math.round(s.goldUpkeepDaily)}⬢/day`} tone="var(--ot-con-red)" testid="svc-upkeep" />
        )}
        <Row label="Supply delivered" value={`${Math.round(s.ridershipTotal)}`} />
        <Divider />
        <Row label="🏰 Towns taken" value={`${Math.round(s.townsCaptured)}`} testid="svc-towns" />
        <Row label="⚔ Legions afield" value={`${Math.round(s.armyCount)}`} />
        <Row label="☠ Decadence" value={`${decay}%`} tone={decayColor} testid="svc-decadence" />
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, marginTop: 8, lineHeight: 1.35 }}>
          Supply towns for tribute · field legions from a barracks · post bounties to steer them · hold back the decadence.
        </div>
      </div>
    </div>
  );
}

export function ServiceReport({ embedded = false }: { embedded?: boolean } = {}) {
  const s = useStats();
  const ui = useGameUI();
  // Embedded in the dock tab → always open (the tab IS the disclosure); standalone keeps the toggle.
  const [open, setOpen] = useState(true);
  // Mode-aware: the fantasy campaign shows the realm ledger, not transit service telemetry.
  if (ui.ruleset === "arcadia") return <FantasyServiceReport s={s} embedded={embedded} />;

  // Aggregate ridership by transport mode from the per-line snapshot (no new sim field needed).
  const byMode = new Map<number, number>();
  for (const l of s.perLine) byMode.set(l.mode, (byMode.get(l.mode) ?? 0) + l.ridership);
  const modeRows = MODES.map((m) => ({ ...m, riders: Math.round(byMode.get(m.id) ?? 0) }))
    .filter((m) => m.riders > 0)
    .sort((a, b) => b.riders - a.riders);
  const maxMode = Math.max(1, ...modeRows.map((m) => m.riders));

  const cov = Math.round(s.coverageScore);
  const covColor = cov >= 60 ? "var(--ot-gauge-good,#009e73)" : cov >= 30 ? "#e69f00" : "var(--ot-gauge-bad,#d62828)";
  const waiting = Math.round(s.waitingTotal);
  const denied = Math.round(s.deniedBoardings);
  const gaveUp = Math.round(s.abandoned);

  const showBody = embedded || open;
  const wrap: CSSProperties = embedded
    ? { height: "100%", overflowY: "auto", font: "12px system-ui,sans-serif", color: "var(--ot-con-ink)" }
    : CARD;
  return (
    <div data-testid="service-report" className={embedded ? undefined : "ot-console"} style={wrap}>
      {!embedded && (
        <button
          data-testid="service-toggle"
          onClick={() => setOpen((o) => !o)}
          style={{
            all: "unset",
            boxSizing: "border-box",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            width: "100%",
            padding: "8px 12px",
            cursor: "pointer",
            font: "600 13px system-ui,sans-serif",
            color: "var(--ot-con-ink)",
          }}
        >
          <span>📊 Network</span>
          <span style={{ color: "var(--ot-con-ink-dim)" }}>{open ? "▾" : "▸"}</span>
        </button>
      )}

      {showBody && (
        <div style={{ padding: "0 12px 12px" }}>
            {/* What demand IS — the conceptual key, so every served/pressure number below has meaning.
                Homes (origins) generate trips, jobs (destinations) attract them; the rush flips AM↔PM.
                This is the "what is demand / are trips directional pairs" answer, in the demand panel. */}
            <div data-testid="demand-explainer" style={{ marginBottom: 6 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                <b style={{ color: "var(--ot-con-ink)" }}>Demand</b>
                <span data-testid="demand-flow" style={{ color: "var(--ot-con-ink-dim)", fontSize: 11 }}>{demandFlow(s.period)}</span>
              </div>
              <div style={{ color: "var(--ot-con-ink-dim)", marginTop: 2 }}>🏠 homes start trips · 💼 jobs pull them in</div>
              {ui.showDemand && (
                <div data-testid="demand-key" style={{ marginTop: 4, lineHeight: 1.55 }}>
                  <div><Swatch rgb="rgb(196,96,46)" />unserved — build here</div>
                  <div><Swatch rgb="rgb(90,130,170)" />covered by a station</div>
                  <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, marginTop: 2 }}>
                    Pin a station → arcs show where its riders travel.
                  </div>
                </div>
              )}
            </div>
            <Divider />

            {/* Service quality — the journey-time telemetry, previously only in a hover tooltip. */}
            <Row label="Avg wait" value={fmtMin(s.avgWaitMs)} testid="svc-avg-wait" />
            <Row label="Avg trip" value={fmtMin(s.avgJourneyMs)} testid="svc-avg-trip" />
            <Divider />

            {/* Coverage (% of the whole city's demand served well) + the time-of-day rush level. */}
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "2px 0" }}>
              <span style={{ color: "var(--ot-con-ink-dim)" }} title="How much of the whole city's demand your network serves well — grows as you expand">City coverage</span>
              <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ position: "relative", width: 64, height: 7, background: "var(--ot-well-bg)", boxShadow: "var(--ot-well)", borderRadius: 4, overflow: "hidden" }}>
                  <span style={{ position: "absolute", inset: `0 ${100 - cov}% 0 0`, background: covColor }} />
                </span>
                <b data-testid="svc-coverage" className="ot-readout" style={{ minWidth: 24, padding: "0 5px", textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{cov}</b>
              </span>
            </div>
            <Row label={s.period} value={`×${s.demandMultiplier.toFixed(1)} demand`} testid="svc-rush" />
            <Divider />

            {/* Pressure — the only difficulty source in the money-free game (AGENTS game design). */}
            <Row label="Waiting now" value={`${waiting}`} tone={waiting > 200 ? "#e69f00" : undefined} testid="svc-waiting" />
            <Row label="Passed by full trains" value={`${denied}`} tone={denied > 0 ? "var(--ot-gauge-bad,#d62828)" : undefined} testid="svc-denied" />
            <Row label="Gave up waiting" value={`${gaveUp}`} tone={gaveUp > 0 ? "var(--ot-gauge-bad,#d62828)" : undefined} testid="svc-gaveup" />

            {/* Ridership by mode — makes the multi-modal network legible at a glance. */}
            {modeRows.length > 0 && (
              <>
                <Divider />
                <div style={{ color: "var(--ot-con-ink-dim)", marginBottom: 4 }}>Riders by mode</div>
                {modeRows.map((m) => (
                  <div key={m.id} data-testid={`svc-mode-${m.id}`} style={{ display: "flex", alignItems: "center", gap: 6, padding: "1px 0" }}>
                    <span style={{ width: 16, textAlign: "center" }}>{m.icon}</span>
                    <span style={{ position: "relative", flex: 1, height: 8, background: "var(--ot-well-bg)", boxShadow: "var(--ot-well)", borderRadius: 4, overflow: "hidden" }}>
                      <span style={{ position: "absolute", inset: 0, width: `${(m.riders / maxMode) * 100}%`, background: m.color, borderRadius: 4 }} />
                    </span>
                    <b style={{ width: 38, textAlign: "right", fontVariantNumeric: "tabular-nums", color: "var(--ot-con-ink)" }}>{m.riders}</b>
                  </div>
                ))}
              </>
            )}

            {/* The new accessibility loop: demand grows where the network reaches well. */}
            <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, marginTop: 8, lineHeight: 1.3 }}>
              Trips favour destinations your network reaches fastest — extend reach to unlock demand.
            </div>
        </div>
      )}
    </div>
  );
}
