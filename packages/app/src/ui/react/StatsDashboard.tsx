// The prominent "📊 Network" dashboard: a ledger + detailed stats + trend charts, opened from a
// fixed button (App mounts it). Reads the live Stats slice (useStats) for headline KPIs + the
// rolling history (useStatsHistory) for the charts. Pure read — every number comes from the sim
// snapshot or a frontend derivation that shares lineEconomics.ts with the roster (no drift). The
// recorder that feeds the charts is mounted separately and always-on, so history survives close.
import type { CSSProperties } from "react";
import { useGame, useStats } from "./GameContext";
import { useStatsHistory } from "./statsHistory";
import { ChartCard, BarList } from "./Charts";
import { linePnl, lineSatisfaction, fmtSignedMoney } from "./lineEconomics";
import { SIM_MS_PER_CLOCK_MIN, hex, fmtMoney } from "./shared";

/** Compact count formatter: 980 / 12.3k / 1.4M. */
function fmtCount(v: number): string {
  const a = Math.abs(v);
  if (a >= 1e6) return `${(v / 1e6).toFixed(1)}M`;
  if (a >= 1e4) return `${(v / 1e3).toFixed(0)}k`;
  if (a >= 1e3) return `${(v / 1e3).toFixed(1)}k`;
  return `${Math.round(v)}`;
}
const fmtMins = (ms: number) => (ms > 0 ? `${(ms / SIM_MS_PER_CLOCK_MIN).toFixed(1)} min` : "—"); // clock minutes
/** simHour is a float (e.g. 21.44) — render as a wall clock, same as StatsBar. */
const fmtClock = (h: number) => `${String(Math.floor(h)).padStart(2, "0")}:${String(Math.floor((h % 1) * 60)).padStart(2, "0")}`;

// A KPI tile = a recessed instrument well on the console face (dark, inset). The big number sits in a
// digital readout; the label/sub are etched dim ink. Semantic colours (good/bad/amber) are passed in.
const TILE: CSSProperties = {
  flex: "1 1 0",
  minWidth: 88,
  background: "#14171c",
  borderRadius: 10,
  padding: "10px 12px",
  boxShadow: "var(--ot-well)",
};

function Kpi({ label, value, sub, color, testid }: { label: string; value: string; sub?: string; color?: string; testid?: string }) {
  return (
    <div style={TILE} data-testid={testid}>
      <div style={{ fontSize: 11, color: "var(--ot-con-ink-dim)", marginBottom: 3 }}>{label}</div>
      <div style={{ fontSize: 22, fontWeight: 800, lineHeight: 1, fontFamily: "var(--ot-readout-font)", color: color ?? "var(--ot-con-accent)" }}>{value}</div>
      {sub && <div style={{ fontSize: 11, color: "var(--ot-con-ink-dim)", marginTop: 3 }}>{sub}</div>}
    </div>
  );
}

function LedgerRow({ label, amount, sign }: { label: string; amount: number; sign: "+" | "-" | "=" }) {
  const color = sign === "+" ? "var(--ot-gauge-good,#009e73)" : sign === "-" ? "var(--ot-gauge-bad,#d62828)" : "var(--ot-con-ink)";
  const prefix = sign === "+" ? "+" : sign === "-" ? "−" : "";
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", borderTop: sign === "=" ? "1px solid rgba(255,255,255,.08)" : "none" }}>
      <span style={{ color: "var(--ot-con-ink-dim)", fontWeight: sign === "=" ? 700 : 400 }}>{label}</span>
      <b style={{ color, fontFamily: "var(--ot-readout-font)" }}>{prefix}{fmtMoney(Math.abs(amount))}</b>
    </div>
  );
}

const SECTION_TITLE: CSSProperties = { fontSize: 12, fontWeight: 700, color: "var(--ot-con-ink-dim)", margin: "2px 0 6px", textTransform: "uppercase", letterSpacing: 0.4 };

export function StatsDashboard({ open, onClose }: { open: boolean; onClose: () => void }) {
  const game = useGame();
  const s = useStats();
  const history = useStatsHistory();
  if (!open) return null;

  const lines = s.perLine;
  const served = lines.filter((l) => l.trains > 0);
  // Network satisfaction: ridership-weighted mean of per-line satisfaction (busy lines count more).
  let satNum = 0;
  let satDen = 0;
  for (const l of served) {
    const sat = lineSatisfaction(l, game.lineQueue(l.lineId));
    if (sat) {
      const w = Math.max(1, l.ridership);
      satNum += sat.score * w;
      satDen += w;
    }
  }
  const netSat = satDen > 0 ? Math.round(satNum / satDen) : null;
  const satColor = netSat == null ? "var(--ot-con-ink-dim)" : netSat >= 70 ? "var(--ot-gauge-good,#009e73)" : netSat >= 45 ? "#e69f00" : "var(--ot-gauge-bad,#d62828)";

  // Ledger figures (informational even when the economy is off — the sim always tallies fares).
  const fares = s.fareRevenue;
  const capital = s.capitalSpent;
  const opex = s.opexSpent;
  const balance = s.balance;

  const topRidership = [...lines]
    .filter((l) => l.ridership > 0)
    .sort((a, b) => b.ridership - a.ridership)
    .slice(0, 8)
    .map((l) => ({ key: l.lineId, label: l.name || `Line ${l.lineId + 1}`, value: l.ridership, color: hex(l.color) }));

  // Per-line P&L ranking (best earners first; net = fares − capital, shared with the roster).
  const pnl = [...lines]
    .map((l) => ({ l, p: linePnl(l, s) }))
    .sort((a, b) => b.p.net - a.p.net);
  const topPnl = pnl.slice(0, 6);

  return (
    <>
      <div
        data-testid="dashboard-scrim"
        onClick={onClose}
        style={{ position: "fixed", inset: 0, background: "rgba(12,15,19,.35)", zIndex: 40 }}
      />
      <div
        data-testid="stats-dashboard"
        className="ot-console"
        style={{
          position: "fixed",
          top: "50%",
          left: "50%",
          transform: "translate(-50%,-50%)",
          width: "min(760px,94vw)",
          maxHeight: "88vh",
          overflow: "auto",
          zIndex: 41,
          padding: 16,
          font: "13px system-ui,sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", marginBottom: 12 }}>
          <div style={{ fontSize: 17, fontWeight: 800, color: "var(--ot-con-ink)" }}>📊 Network Dashboard</div>
          <div style={{ marginLeft: 10, color: "var(--ot-con-ink-dim)", fontSize: 12 }}>
            {s.period} · {fmtClock(s.simHour)} · {s.running ? "running" : "paused"}
          </div>
          <button
            data-testid="dashboard-close"
            className="ot-key"
            onClick={onClose}
            style={{ marginLeft: "auto", padding: "5px 12px", cursor: "pointer", font: "600 13px system-ui" }}
          >
            ✕ Close
          </button>
        </div>

        {/* Headline gamey KPIs — big and prominent. */}
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 14 }}>
          <Kpi label="Riders carried" value={fmtCount(s.ridershipTotal)} sub={`${lines.length} lines · ${s.vehicleCount} vehicles`} color="var(--ot-con-accent)" testid="kpi-ridership" />
          <Kpi label="Coverage" value={`${Math.round(s.coverageScore)}`} sub="of the whole city" color={s.coverageScore >= 60 ? "var(--ot-gauge-good,#009e73)" : s.coverageScore >= 35 ? "#e69f00" : "var(--ot-con-ink-dim)"} testid="kpi-coverage" />
          <Kpi label="Satisfaction" value={netSat == null ? "—" : `${netSat}%`} sub={netSat == null ? "no service" : netSat >= 70 ? "happy" : netSat >= 45 ? "ok" : "unhappy"} color={satColor} testid="kpi-satisfaction" />
          <Kpi label={s.economyEnabled ? "Balance" : "Balance (info)"} value={fmtMoney(balance)} sub={s.economyEnabled ? "economy ON" : "economy off"} color={balance >= 0 ? "var(--ot-gauge-good,#009e73)" : "var(--ot-gauge-bad,#d62828)"} testid="kpi-balance" />
          <Kpi label="Left behind" value={fmtCount(s.abandoned + s.deniedBoardings)} sub={`${fmtCount(s.waitingTotal)} waiting now`} color={s.abandoned + s.deniedBoardings > 0 ? "var(--ot-gauge-bad,#d62828)" : "var(--ot-con-ink)"} testid="kpi-leftbehind" />
        </div>

        <div style={{ display: "flex", gap: 14, flexWrap: "wrap", alignItems: "flex-start" }}>
          {/* Ledger */}
          <div style={{ flex: "1 1 240px", minWidth: 240 }}>
            <div style={SECTION_TITLE}>Ledger</div>
            <div style={{ background: "#14171c", borderRadius: 10, padding: "10px 12px", boxShadow: "var(--ot-well)" }}>
              <LedgerRow label="Fares collected" amount={fares} sign="+" />
              <LedgerRow label="Capital (build)" amount={capital} sign="-" />
              <LedgerRow label="Opex (upkeep)" amount={opex} sign="-" />
              <LedgerRow label="Balance" amount={balance} sign="=" />
              <div style={{ fontSize: 11, color: "var(--ot-con-ink-dim)", marginTop: 6 }}>
                Avg journey {fmtMins(s.avgJourneyMs)} · avg wait {fmtMins(s.avgWaitMs)} · load {Math.round(s.avgLoadFactor * 100)}%
              </div>
            </div>

            <div style={{ ...SECTION_TITLE, marginTop: 12 }}>Line P&amp;L</div>
            <div style={{ background: "#14171c", borderRadius: 10, padding: "8px 12px", boxShadow: "var(--ot-well)" }}>
              {topPnl.length === 0 ? (
                <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 12 }}>No lines yet.</div>
              ) : (
                topPnl.map(({ l, p }) => (
                  <div key={l.lineId} style={{ display: "flex", alignItems: "center", gap: 6, padding: "2px 0", color: "var(--ot-con-ink)" }}>
                    <span style={{ width: 10, height: 10, borderRadius: 3, flex: "0 0 auto", background: hex(l.color) }} />
                    <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{l.name || `Line ${l.lineId + 1}`}</span>
                    <b style={{ color: p.inBlack ? "var(--ot-gauge-good,#009e73)" : "var(--ot-gauge-bad,#d62828)" }}>{fmtSignedMoney(p.net)}</b>
                  </div>
                ))
              )}
            </div>
          </div>

          {/* Per-line ridership ranking */}
          <div style={{ flex: "1 1 240px", minWidth: 240 }}>
            <div style={SECTION_TITLE}>Ridership by line</div>
            <div style={{ background: "#14171c", borderRadius: 10, padding: "10px 12px", boxShadow: "var(--ot-well)" }}>
              {topRidership.length === 0 ? (
                <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 12 }}>No riders carried yet.</div>
              ) : (
                <BarList items={topRidership} format={fmtCount} />
              )}
            </div>
          </div>
        </div>

        {/* Trend charts over sim time */}
        <div style={{ ...SECTION_TITLE, marginTop: 14 }}>Trends</div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit,minmax(220px,1fr))", gap: 10 }}>
          <ChartCard testid="chart-ridership" title="Riders carried" values={history.map((h) => h.ridership)} color="#0072b2" format={fmtCount} />
          <ChartCard testid="chart-balance" title={s.economyEnabled ? "Balance ($)" : "Balance ($, info)"} values={history.map((h) => h.balance)} color="#7b3ff2" format={fmtMoney} zeroLine />
          <ChartCard testid="chart-coverage" title="Coverage" values={history.map((h) => h.coverage)} color="#009e73" format={(v) => `${Math.round(v)}`} />
          <ChartCard testid="chart-waiting" title="Waiting now" values={history.map((h) => h.waiting)} color="#e69f00" format={fmtCount} />
          <ChartCard testid="chart-leftbehind" title="Gave up (cumulative)" values={history.map((h) => h.abandoned)} color="#d62828" format={fmtCount} />
          <ChartCard testid="chart-fares" title="Fares collected ($)" values={history.map((h) => h.fareRevenue)} color="#009e73" format={fmtMoney} />
        </div>
        <div style={{ fontSize: 11, color: "var(--ot-con-ink-dim)", marginTop: 8 }}>
          Trends sample once per sim-minute; the window slides as you play. P&amp;L = fares − build cost; opex is a network-wide drain shown in the ledger.
        </div>
      </div>
    </>
  );
}
