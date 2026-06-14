// Top-centre HUD: the headline Ridership counter + a 0–100 Coverage/Satisfaction gauge.
// Fed by the ~3 Hz stats throttle (useStats — never per frame). One number + one gauge,
// details on demand elsewhere (AGENTS IA). React reconciles, so no last-value caching.
import type { CSSProperties } from "react";
import type { Stats } from "../../types";
import { useStats } from "./GameContext";
import { SIM_MS_PER_CLOCK_MIN, fmtMoney, loadPip } from "./shared";
import { useTweenedNumber } from "./useTween";
import { cityById } from "../../sim/cities";
import { cashTrend } from "./statsHistory";

/** This run's city anchor (the real network's coverage score) — read once; the URL is stable
 *  after boot (deep link, or the menu mirrors the start into it). */
const CITY_ANCHOR = () => cityById(new URLSearchParams(location.search).get("city")).realScore;

// Stable formatters (module-level so the tween hook's dep identity never changes per render).
const fmtInt = (n: number): string => `${Math.round(n)}`;

/** Sun/moon glyph for the hour — makes the time-of-day (and the day/night map wash) legible in TEXT,
 *  not hue alone (recognition-over-recall, colour-blind-safe). Bands match the sky.ts palette. */
function todGlyph(hour: number): string {
  const h = ((hour % 24) + 24) % 24;
  if (h < 6 || h >= 20) return "🌙";
  if (h < 8) return "🌅";
  if (h >= 18) return "🌇";
  return "☀️";
}

/** The HUD shell — shared chrome (position/style) so transit + fantasy read the same. */
const BAR_STYLE: CSSProperties = {
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
};

/** Fantasy (arcadia) HUD: the supply→conquest→decadence readout — tribute, the lose-meter gauge,
 *  towns taken, legions afield. Replaces riders/coverage; the same `useStats` ~3 Hz slice. */
function FantasyStatsBar({ s, clock }: { s: Stats; clock: string }) {
  const tribute = Math.round(s.tribute);
  // S11 economy split — the two specialised channels show only once they've been earned (a clean HUD for
  // a realm that hasn't built an aether/arms chain yet; they appear the moment one does).
  const mana = Math.round(s.mana);
  const manpower = Math.round(s.manpower);
  const towns = Math.round(s.townsCaptured);
  const armies = Math.round(s.armyCount);
  const d = Math.round(s.decadencePct);
  // Lose-meter: neutral while low, amber mid, red as the rot nears the capital.
  const dColor = d >= 66 ? "var(--ot-gauge-bad)" : d >= 33 ? "#e69f00" : "#7a93ad";
  // Realm STANDING — the progress gauge (supply reach + conquest), the rising counterpart to the
  // decadence gauge ("two gauges, two jobs"): what you've built + hold, vs. the rot you're racing.
  const standing = Math.round(s.coverageScore);
  const sColor = standing >= 60 ? "var(--ot-gauge-good)" : standing >= 30 ? "#e69f00" : "#7a93ad";
  return (
    <div id="stats-bar" data-testid="stats-bar" style={BAR_STYLE}>
      <div>
        <b data-testid="clock" style={{ fontVariantNumeric: "tabular-nums" }}>{clock}</b>{" "}
        <span data-testid="period" style={{ color: "#7a818a" }}>Arcadia</span>
      </div>
      <div style={{ width: "1px", alignSelf: "stretch", background: "#e2e5e9" }} />
      <div data-testid="tribute" title="Gold — every town you supply pays this; it funds legions and general tech.">
        ⚜ <b style={{ fontSize: "16px", fontVariantNumeric: "tabular-nums" }}>{tribute}</b> gold
      </div>
      {mana > 0 && (
        <div data-testid="mana" title="Mana — minted by aether supply chains; funds arcane tech (Sappers)." style={{ color: "#7a4ed2" }}>
          ✦ <b style={{ fontVariantNumeric: "tabular-nums" }}>{mana}</b> mana
        </div>
      )}
      {manpower > 0 && (
        <div data-testid="manpower" title="Manpower — minted by arms (ingot) supply chains; funds military tech (Conscription)." style={{ color: "#b5651d" }}>
          ⚔ <b style={{ fontVariantNumeric: "tabular-nums" }}>{manpower}</b> manpower
        </div>
      )}
      <div
        data-testid="standing-gauge"
        style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "help" }}
        title="Realm standing — how much of the realm you supply and hold. Build rail to towns and conquer them to raise it (the rising counterpart to the decadence you're racing)."
      >
        🛡 Standing
        <div style={{ position: "relative", width: "90px", height: "10px", background: "#e7eaee", borderRadius: "6px", overflow: "hidden" }}>
          <div
            data-testid="standing-bar"
            style={{ position: "absolute", inset: "0 auto 0 0", width: `${standing}%`, background: sColor, borderRadius: "6px", transition: "width .5s var(--ot-ease), background-color .4s linear" }}
          />
        </div>
        <b style={{ width: "26px", textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{standing}</b>
      </div>
      <div
        data-testid="decadence-gauge"
        style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "help" }}
        title="The Decadence — spreading corruption. If it reaches your capital, the realm falls. Conquest holds it back."
      >
        ☠ Decadence
        <div style={{ position: "relative", width: "90px", height: "10px", background: "#e7eaee", borderRadius: "6px", overflow: "hidden" }}>
          <div
            data-testid="decadence-bar"
            style={{ position: "absolute", inset: "0 auto 0 0", width: `${d}%`, background: dColor, borderRadius: "6px", transition: "width .5s var(--ot-ease), background-color .4s linear" }}
          />
        </div>
        <b style={{ width: "26px", textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{d}</b>
      </div>
      <div data-testid="towns-captured" style={{ color: "#7a818a", cursor: "help" }} title="Towns conquered. Each captured town pushes the decadence back.">
        🏰 <b style={{ color: "#1c2024" }}>{towns}</b> taken
      </div>
      {armies > 0 && (
        <div data-testid="armies" style={{ color: "#7a818a" }} title="Legions afield — AI-led, riding your rails, baited by bounties.">
          ⚔ {armies} legion{armies === 1 ? "" : "s"}
        </div>
      )}
      {Math.round(s.raiderCount) > 0 && (
        <div data-testid="raiders" style={{ color: "#5c7a2e", fontWeight: 600 }} title="Decadence raiders marching on your capital. Your rail network cuts them down — cover the approaches, or they deepen the rot.">
          ☣ {Math.round(s.raiderCount)} raider{Math.round(s.raiderCount) === 1 ? "" : "s"}
        </div>
      )}
      {s.realmLost && (
        <div data-testid="realm-lost" style={{ color: "var(--ot-gauge-bad)", fontWeight: 700 }}>
          ☠ THE REALM HAS FALLEN
        </div>
      )}
    </div>
  );
}

export function StatsBar() {
  const s = useStats();
  // Rolling headline counters — the value eases toward each new ~3 Hz snapshot instead of snapping
  // (juice). Ref-owned textContent, so these <b> carry NO JSX child (React must not clobber them).
  const ridershipRef = useTweenedNumber(s.ridershipTotal, fmtInt);
  const coverageRef = useTweenedNumber(s.coverageScore, fmtInt);

  const hh = Math.floor(s.simHour);
  const mm = Math.floor((s.simHour - hh) * 60);
  const clock = `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;

  // Mode-aware: the fantasy campaign shows its own supply/conquest/decadence readout.
  if (s.ruleset === "arcadia") return <FantasyStatsBar s={s} clock={clock} />;

  const c = Math.round(s.coverageScore);
  const w = Math.round(s.waitingTotal);
  const lost = Math.round(s.abandoned);
  const denied = Math.round(s.deniedBoardings);
  const avgWaitMin = s.avgWaitMs / SIM_MS_PER_CLOCK_MIN;
  const avgTripMin = s.avgJourneyMs / SIM_MS_PER_CLOCK_MIN;
  // Service-quality detail lives on the waiting readout's tooltip (progressive disclosure),
  // not as new always-on HUD numbers (AGENTS IA: one number + one gauge). The terminal
  // "gave up" count is the visible pressure; the full-train denial count is in the tooltip.
  const waitTip =
    `Avg wait ~${avgWaitMin.toFixed(1)} min · Avg trip ~${avgTripMin.toFixed(1)} min` +
    (denied > 0 ? ` · ${denied} passed by full trains` : "");

  // Bar fills left→right. Coverage is a progression dial (it starts near 0 on a fresh map), so
  // the low band is neutral — not failure-red — and the hue only turns good once it's earned.
  const coverageColor = c >= 60 ? "var(--ot-gauge-good)" : c >= 30 ? "#e69f00" : "#7a93ad";

  // Network strain: the fleet's mean load (avgLoadFactor) as a loadPip, with the live train count
  // in its tooltip. Mounts only once trains run, so it's never dead chrome (like the money box).
  // Reuses the loadPip shape/colour language so "crush" reads the same here as on a train or line.
  const trains = s.vehicleCount;
  const netPip = loadPip(s.avgLoadFactor);

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
        <span data-testid="tod-glyph" title={s.period} style={{ marginRight: "5px" }}>
          {todGlyph(s.simHour)}
        </span>
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
        <b ref={ridershipRef} data-testid="ridership" style={{ fontSize: "16px", fontVariantNumeric: "tabular-nums" }} />{" "}
        riders
      </div>
      <div
        style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "help" }}
        title={`How much of the whole city's demand your network serves well — grows as you expand. The city's real network scores ~${CITY_ANCHOR()}.`}
      >
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
            style={{
              position: "absolute",
              top: 0,
              bottom: 0,
              left: 0,
              width: `${c}%`,
              background: coverageColor,
              borderRadius: "6px",
              // Gauge fill eases toward each new coverage value; hue cross-fades good→bad (juice).
              transition: "width .5s var(--ot-ease), background-color .4s linear",
            }}
          ></div>
        </div>
        <b ref={coverageRef} data-testid="coverage" style={{ width: "26px", textAlign: "right", fontVariantNumeric: "tabular-nums" }} />
      </div>
      <div style={{ color: "#7a818a", cursor: "help" }} title={waitTip}>
        <span data-testid="waiting">{w}</span> waiting
        {lost > 0 && (
          <span data-testid="left-behind" style={{ color: "var(--ot-gauge-bad)", marginLeft: "6px" }}>
            · {lost} gave up
          </span>
        )}
      </div>
      {/* Network strain (run only): mean fleet load + live train count. Part of the pressure
          cluster, not a new headline — surfaces avgLoadFactor + vehicleCount which were computed
          but never shown. Mounts only with trains running, so it's never dead chrome. */}
      {trains > 0 && (
        <div
          data-testid="net-load"
          style={{ color: "#7a818a", cursor: "help" }}
          title={`${trains} train${trains === 1 ? "" : "s"} running · mean load ${netPip.pct}% (${netPip.word})`}
        >
          <span style={{ color: netPip.color, fontWeight: 700 }}>
            {netPip.glyph} {netPip.pct}%
          </span>{" "}
          load
        </div>
      )}
      {/* Build impact left the run HUD — it's a build-time, per-line concern (EditorPanel
          `line-impact`), not a global always-on number. Money mounts only with the economy
          ruleset on, so it's never dead chrome. StatsBar = clock · ridership · gauge · pressure. */}
      {s.economyEnabled && (() => {
        // Operating-cash trend (fares − opex per day; capital excluded): the "am I dying?"
        // affordance the audit asked for — the drain is visible BEFORE the afford-gate fires.
        const trend = cashTrend(s.balance);
        const burning = trend !== null && trend.perDay < 0;
        const trendTip =
          trend === null
            ? ""
            : burning
              ? ` · burning ${fmtMoney(-trend.perDay)}/day${trend.runwayDays !== null ? ` — ≈${Math.max(1, Math.round(trend.runwayDays))} days of runway` : ""}`
              : ` · earning ${fmtMoney(trend.perDay)}/day from operations`;
        return (
          <div
            data-testid="money-box"
            style={{ cursor: "help" }}
            title={`Fares ${fmtMoney(s.fareRevenue)} − capital ${fmtMoney(s.capitalSpent)} − upkeep ${fmtMoney(s.opexSpent)}${trendTip}`}
          >
            💰{" "}
            <b data-testid="money" style={{ color: s.balance < 0 ? "var(--ot-gauge-bad)" : "var(--ot-gauge-good)" }}>
              {fmtMoney(s.balance)}
            </b>
            {trend !== null && (
              <span data-testid="money-trend" style={{ marginLeft: 4, fontSize: 11, color: burning ? "var(--ot-gauge-bad)" : "var(--ot-gauge-good)" }}>
                {burning ? "▼" : "▲"}
                {burning && trend.runwayDays !== null && trend.runwayDays < 30 && (
                  <span style={{ marginLeft: 2 }}>≈{Math.max(1, Math.round(trend.runwayDays))}d</span>
                )}
              </span>
            )}
          </div>
        );
      })()}
    </div>
  );
}
