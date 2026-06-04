// Top-centre HUD: the headline Ridership counter + a 0–100 Coverage/Satisfaction gauge.
// Fed by the ~3 Hz stats throttle (useStats — never per frame). One number + one gauge,
// details on demand elsewhere (AGENTS IA). React reconciles, so no last-value caching.
import { useStats } from "./GameContext";
import { fmtMoney } from "./shared";

export function StatsBar() {
  const s = useStats();

  const hh = Math.floor(s.simHour);
  const mm = Math.floor((s.simHour - hh) * 60);
  const clock = `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;

  const r = Math.round(s.ridershipTotal);
  const c = Math.round(s.coverageScore);
  const w = Math.round(s.waitingTotal);
  const lost = Math.round(s.abandoned);
  const denied = Math.round(s.deniedBoardings);
  const avgWaitMin = s.avgWaitMs / 60000;
  const avgTripMin = s.avgJourneyMs / 60000;
  // Service-quality detail lives on the waiting readout's tooltip (progressive disclosure),
  // not as new always-on HUD numbers (AGENTS IA: one number + one gauge). The terminal
  // "gave up" count is the visible pressure; the full-train denial count is in the tooltip.
  const waitTip =
    `Avg wait ~${avgWaitMin.toFixed(1)} min · Avg trip ~${avgTripMin.toFixed(1)} min` +
    (denied > 0 ? ` · ${denied} passed by full trains` : "");

  // Bar fills left→right; hue shifts good→bad as coverage drops.
  const coverageColor = c >= 60 ? "var(--ot-gauge-good)" : c >= 30 ? "#e69f00" : "var(--ot-gauge-bad)";

  return (
    <div
      id="stats-bar"
      data-testid="stats-bar"
      style={{
        position: "fixed",
        top: "10px",
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: "16px",
        padding: "8px 14px",
        background: "rgba(255,255,255,.95)",
        borderRadius: "10px",
        boxShadow: "var(--ot-shadow)",
        zIndex: 9,
        font: "13px system-ui,sans-serif",
        color: "#1c2024",
      }}
    >
      <div>
        <b data-testid="clock" style={{ fontVariantNumeric: "tabular-nums" }}>
          {clock}
        </b>{" "}
        <span data-testid="period" style={{ color: "#7a818a" }}>
          {s.period}
        </span>
      </div>
      <div style={{ width: "1px", alignSelf: "stretch", background: "#e2e5e9" }}></div>
      <div>
        🚇{" "}
        <b data-testid="ridership" style={{ fontSize: "16px" }}>
          {r}
        </b>{" "}
        riders
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        Coverage
        <div
          style={{
            position: "relative",
            width: "90px",
            height: "10px",
            background: "#e7eaee",
            borderRadius: "6px",
            overflow: "hidden",
          }}
        >
          <div
            data-testid="coverage-bar"
            style={{ position: "absolute", inset: `0 ${100 - c}% 0 0`, background: coverageColor }}
          ></div>
        </div>
        <b data-testid="coverage" style={{ width: "26px", textAlign: "right" }}>
          {c}
        </b>
      </div>
      <div style={{ color: "#7a818a", cursor: "help" }} title={waitTip}>
        <span data-testid="waiting">{w}</span> waiting
        {lost > 0 && (
          <span data-testid="left-behind" style={{ color: "var(--ot-gauge-bad)", marginLeft: "6px" }}>
            · {lost} gave up
          </span>
        )}
      </div>
      {/* Build impact left the run HUD — it's a build-time, per-line concern (EditorPanel
          `line-impact`), not a global always-on number. Money mounts only with the economy
          ruleset on, so it's never dead chrome. StatsBar = clock · ridership · gauge · pressure. */}
      {s.economyEnabled && (
        <div
          data-testid="money-box"
          style={{ cursor: "help" }}
          title={`Fares ${fmtMoney(s.fareRevenue)} − capital ${fmtMoney(s.capitalSpent)} − upkeep ${fmtMoney(s.opexSpent)}`}
        >
          💰{" "}
          <b data-testid="money" style={{ color: s.balance < 0 ? "var(--ot-gauge-bad)" : "var(--ot-gauge-good)" }}>
            {fmtMoney(s.balance)}
          </b>
        </div>
      )}
    </div>
  );
}
