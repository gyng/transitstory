// Bottom-left "Network" dashboard: surfaces the rich service-quality telemetry the sim now
// produces (journey/wait times, demand served, the full-train + renege pressure, and the
// per-mode ridership split) in one scannable card — the abstract-state-in-panels half of the
// AGENTS IA, so the top StatsBar can stay "one number + one gauge". Pure chrome: reads only the
// ~3 Hz `stats` slice, issues no Commands. Collapsible, but default-open so the info is visible
// (a discoverable channel, not a buried hover-tooltip).
import { useState, type CSSProperties } from "react";
import { useStats } from "./GameContext";
import { MODES } from "./shared";

const CARD: CSSProperties = {
  position: "fixed",
  left: 14,
  bottom: 14,
  zIndex: 9,
  width: 248,
  borderRadius: 10,
  background: "rgba(255,255,255,.95)",
  boxShadow: "var(--ot-shadow, 0 2px 10px rgba(0,0,0,.12))",
  font: "12px system-ui,sans-serif",
  color: "#1c2024",
  overflow: "hidden",
};

const fmtMin = (ms: number): string => (ms > 0 ? `${(ms / 60000).toFixed(1)} min` : "—");

/** One label/value row; `tone` colours the value for pressure readouts. */
function Row({ label, value, tone, testid }: { label: string; value: string; tone?: string; testid?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "2px 0" }}>
      <span style={{ color: "#7a818a" }}>{label}</span>
      <b data-testid={testid} style={{ fontVariantNumeric: "tabular-nums", color: tone ?? "#1c2024" }}>
        {value}
      </b>
    </div>
  );
}

function Divider() {
  return <div style={{ height: 1, background: "#e7eaee", margin: "6px 0" }} />;
}

export function ServiceReport() {
  const s = useStats();
  const [open, setOpen] = useState(true);

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

  return (
    <div data-testid="service-report" style={CARD}>
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
        }}
      >
        <span>📊 Network</span>
        <span style={{ color: "#9aa3ad" }}>{open ? "▾" : "▸"}</span>
      </button>

      {open && (
        <div style={{ padding: "0 12px 12px" }}>
            {/* Service quality — the journey-time telemetry, previously only in a hover tooltip. */}
            <Row label="Avg wait" value={fmtMin(s.avgWaitMs)} testid="svc-avg-wait" />
            <Row label="Avg trip" value={fmtMin(s.avgJourneyMs)} testid="svc-avg-trip" />
            <Divider />

            {/* Demand served + the time-of-day rush level. */}
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "2px 0" }}>
              <span style={{ color: "#7a818a" }}>Demand served</span>
              <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={{ position: "relative", width: 64, height: 7, background: "#e7eaee", borderRadius: 4, overflow: "hidden" }}>
                  <span style={{ position: "absolute", inset: `0 ${100 - cov}% 0 0`, background: covColor }} />
                </span>
                <b data-testid="svc-coverage" style={{ width: 24, textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{cov}</b>
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
                <div style={{ color: "#7a818a", marginBottom: 4 }}>Riders by mode</div>
                {modeRows.map((m) => (
                  <div key={m.id} data-testid={`svc-mode-${m.id}`} style={{ display: "flex", alignItems: "center", gap: 6, padding: "1px 0" }}>
                    <span style={{ width: 16, textAlign: "center" }}>{m.icon}</span>
                    <span style={{ position: "relative", flex: 1, height: 8, background: "#eef1f4", borderRadius: 4, overflow: "hidden" }}>
                      <span style={{ position: "absolute", inset: 0, width: `${(m.riders / maxMode) * 100}%`, background: m.color, borderRadius: 4 }} />
                    </span>
                    <b style={{ width: 38, textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{m.riders}</b>
                  </div>
                ))}
              </>
            )}

            {/* The new accessibility loop: demand grows where the network reaches well. */}
            <div style={{ color: "#9aa3ad", fontSize: 11, marginTop: 8, lineHeight: 1.3 }}>
              Trips favour destinations your network reaches fastest — extend reach to unlock demand.
            </div>
        </div>
      )}
    </div>
  );
}
