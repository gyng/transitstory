// TOP-LEFT resource strip (Transport Fever / Anno / Factorio resource bar): the at-a-glance economy
// readouts for the active ruleset — transit riders·coverage·waiting·left-behind·net-load·money;
// fantasy tribute·mana·manpower·standing·decadence·towns·armies·raiders. Each scalar is an
// `.ot-readout`-styled well. Stage 5 split this off the old centred StatsBar: the clock/period/
// tod-glyph testids moved to TimeCluster (top-right), the alert pings to AlertCluster (top-centre),
// and this became the flow-anchored top-LEFT cell of the grid shell (no more position:fixed centre).
//
// Mode-aware off `ui.ruleset`; fed by the ~3 Hz `stats` slice (useStats — never per frame). The two
// oversized gauges (coverage / standing + decadence) keep their bar+tween for glanceability.
import type { CSSProperties } from "react";
import type { Stats } from "../../types";
import { useStats } from "./GameContext";
import { SIM_MS_PER_CLOCK_MIN, coverageColor, fmtCount, fmtMoney, loadPip } from "./shared";
import { useTweenedNumber } from "./useTween";
import { cityById } from "../../sim/cities";
import { cashTrend, channelRates, decadenceTrend } from "./statsHistory";

/** This run's city anchor (the real network's coverage score) — read once; the URL is stable
 *  after boot (deep link, or the menu mirrors the start into it). */
const CITY_ANCHOR = () => cityById(new URLSearchParams(location.search).get("city")).realScore;

// Stable formatters (module-level so the tween hook's dep identity never changes per render).
const fmtInt = (n: number): string => `${Math.round(n)}`;

/** The resource-strip shell — flow-anchored to the top-left grid cell. The console face (dark
 *  gradient / bevel / elevation) comes from `.ot-console`; this carries layout + the ink colour.
 *  Wraps to a 2nd row only inside this L cell (responsive rule) — the C/R cells are unaffected. */
const BAR_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  flexWrap: "wrap",
  gap: "12px",
  rowGap: "4px",
  padding: "7px 12px",
  // Top margin aligns the strip with the Title/TimeCluster row; the LEFT inset now comes from the
  // shared left-cell flex container (gap after the Title) instead of a fixed margin, so the Title and
  // resources sit in one row without an extra offset that could push them apart (item 1 overlap fix).
  margin: "7px 0 0 0",
  pointerEvents: "auto",
  font: "13px system-ui,sans-serif",
  color: "var(--ot-con-ink)",
  maxWidth: "min(640px, 42vw)",
};

/** Manpower a legion costs to field (mirrors the core `army::LAUNCH_COST`) — for the "starved" hint. */
const LEGION_COST = 8;

/** A per-minute flow-rate pill (▲ earning / ▼ draining) for an economy channel — the "am I net-positive?"
 *  legibility a logistics economy needs. Hidden when ~flat (|rate| < 1) so it's never noise. */
function RatePill({ rate, color }: { rate: number | undefined; color: string }) {
  if (rate === undefined || Math.abs(rate) < 1) return null;
  const up = rate > 0;
  return (
    <span style={{ marginLeft: 4, fontSize: 11, color: up ? color : "var(--ot-con-amber)", fontVariantNumeric: "tabular-nums" }}>
      {up ? "▲" : "▼"}{Math.abs(Math.round(rate))}/min
    </span>
  );
}

/** Fantasy (arcadia) resource strip: the supply→conquest→decadence readout — tribute, the lose-meter
 *  gauge, towns taken, legions afield. Replaces riders/coverage; the same `useStats` ~3 Hz slice. */
function FantasyStatsBar({ s }: { s: Stats }) {
  const tribute = Math.round(s.tribute);
  // S11 economy split — the two specialised channels show only once they've been earned (a clean HUD for
  // a realm that hasn't built an aether/arms chain yet; they appear the moment one does).
  const mana = Math.round(s.mana);
  const manpower = Math.round(s.manpower);
  // Per-minute flow rates (velocity), from the rolling history — pairs each stock with its trend.
  const rates = channelRates();
  const towns = Math.round(s.townsCaptured);
  // AFIELD count (#war): marching+besieging, not the inflated all-slots army_count (which counts dead garrisons).
  const armies = Math.round(s.armyAfield ?? s.armyCount);
  const d = Math.round(s.decadencePct);
  // #war: the slice of the rot the RAIDERS pushed on by reaching the capital (vs. the tide creep) — surfaced
  // as a toxic-green tip on the gauge so the player can tell raider pressure from front advance (opposite fixes).
  const breachPct = Math.round(s.raiderBreachPct ?? 0);
  // Lose-meter: neutral while low, amber mid, red as the rot nears the capital.
  const dColor = d >= 66 ? "var(--ot-gauge-bad)" : d >= 33 ? "var(--ot-con-amber)" : "var(--ot-gauge-low)";
  // Threat projection: how fast the rot is rising + a sim-minute ETA to a fallen realm (only while it's
  // actually rising). The doom clock made legible — and the pulse below escalates as the ETA shortens.
  const dt = decadenceTrend(s.decadencePct);
  const eta = dt?.etaMin ?? null;
  const critical = d >= 66 && eta !== null; // rising AND deep → the gauge pulses (escalating dread)
  // Realm STANDING — the progress gauge (supply reach + conquest), the rising counterpart to the
  // decadence gauge ("two gauges, two jobs"): what you've built + hold, vs. the rot you're racing.
  const standing = Math.round(s.coverageScore);
  const sColor = standing >= 60 ? "var(--ot-gauge-good)" : standing >= 30 ? "var(--ot-con-amber)" : "var(--ot-gauge-low)";
  return (
    <div id="stats-bar" data-testid="stats-bar" className="ot-console" style={BAR_STYLE}>
      <div data-testid="tribute" title="Gold — every town you supply pays this; it funds bounties and building.">
        ⚜ <b style={{ fontSize: "16px", fontVariantNumeric: "tabular-nums" }}>{fmtCount(tribute)}</b> gold
        <RatePill rate={rates?.gold} color="var(--ot-con-ink)" />
      </div>
      {mana > 0 && (
        <div data-testid="mana" title="Mana — minted by aether/fuel supply chains; the sole tech resource AND your spell fuel." style={{ color: "#b69bef" }}>
          ✦ <b style={{ fontVariantNumeric: "tabular-nums" }}>{fmtCount(mana)}</b> mana
          <RatePill rate={rates?.mana} color="#b69bef" />
        </div>
      )}
      {/* #war: show manpower even when STARVED (0) once the realm is warring — the player must see "can't
          field a legion" exactly when it bites (it used to hide at 0). ~8 manpower = one legion. */}
      {(manpower > 0 || armies > 0) && (
        <div
          data-testid="manpower"
          title={`Manpower — minted by grain/arms supply chains; each legion costs ~${LEGION_COST} to field, then EATS a little upkeep per day while afield (a standing army needs feeding — keep the arms/ingot flowing).${manpower < LEGION_COST ? " STARVED: too little to field a legion — supply more grain/arms." : ""}`}
          style={{ color: manpower < LEGION_COST ? "var(--ot-gauge-bad)" : "#d39a5c", fontWeight: manpower < LEGION_COST ? 700 : undefined }}
        >
          ⚔ <b style={{ fontVariantNumeric: "tabular-nums" }}>{fmtCount(manpower)}</b> manpower{manpower < LEGION_COST ? " ⚠" : ""}
          <RatePill rate={rates?.manpower} color="#d39a5c" />
        </div>
      )}
      <div
        data-testid="standing-gauge"
        role="meter"
        aria-label="Realm standing"
        aria-valuenow={standing}
        aria-valuemin={0}
        aria-valuemax={100}
        style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "help" }}
        title="Realm standing — how much of the realm you supply and hold. Build rail to towns and conquer them to raise it (the rising counterpart to the decadence you're racing)."
      >
        🛡 Standing
        <div style={{ position: "relative", width: "90px", height: "10px", background: "var(--ot-well-bg)", boxShadow: "var(--ot-well)", borderRadius: "6px", overflow: "hidden" }}>
          <div
            data-testid="standing-bar"
            style={{ position: "absolute", inset: "0 auto 0 0", width: `${standing}%`, background: sColor, borderRadius: "6px", transition: "width .5s var(--ot-ease), background-color .4s linear" }}
          />
        </div>
        <b style={{ width: "26px", textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{standing}</b>
      </div>
      <div
        data-testid="decadence-gauge"
        role="meter"
        aria-label="Decadence"
        aria-valuenow={d}
        aria-valuemin={0}
        aria-valuemax={100}
        className={critical ? "ot-pulse" : undefined}
        // #25 ESCALATING dread: the pulse PERIOD shortens as the ETA-to-fall drops (the comment always promised
        // this; the computed eta was thrown away). Updates on the ~3 Hz stats slice, no per-frame work; .ot-pulse
        // reads var(--ot-doom). Clamped 0.55-1.3s so it stays a heartbeat, never a strobe.
        style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "help", padding: "2px 6px", borderRadius: 7, "--ot-doom": `${Math.max(0.55, Math.min(1.3, (eta ?? 30) / 26)).toFixed(2)}s` } as CSSProperties}
        title={
          (eta !== null
            ? `The Decadence — spreading corruption rising ${dt!.perMin.toFixed(1)}/min. At this rate the realm falls in ~${Math.max(1, Math.round(eta))} min. Conquer towns and Purge to hold it back.`
            : "The Decadence — spreading corruption. If it overruns your capital, the realm falls. Conquest and Purge hold it back.") +
          (breachPct > 0 ? ` The green tip (${breachPct}) is RAIDER BREACH — rot from raiders reaching your capital; cover the approaches to heal it (the rest is the tide front — Purge/conquer that).` : "")
        }
      >
        ☠ Decadence
        <div style={{ position: "relative", width: "90px", height: "10px", background: "var(--ot-well-bg)", boxShadow: "var(--ot-well)", borderRadius: "6px", overflow: "hidden" }}>
          <div
            data-testid="decadence-bar"
            style={{ position: "absolute", inset: "0 auto 0 0", width: `${d}%`, background: dColor, borderRadius: "6px", transition: "width .5s var(--ot-ease), background-color .4s linear" }}
          />
          {breachPct > 0 && (
            <div
              data-testid="decadence-breach"
              title="Raider breach"
              style={{ position: "absolute", top: 0, bottom: 0, left: `${Math.max(0, d - breachPct)}%`, width: `${Math.min(breachPct, d)}%`, background: "repeating-linear-gradient(45deg, #6aa83c, #6aa83c 2px, #8fc95f 2px, #8fc95f 4px)", transition: "left .5s var(--ot-ease), width .5s var(--ot-ease)" }}
            />
          )}
        </div>
        <b style={{ width: "26px", textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{d}</b>
        {eta !== null && (
          <span data-testid="decadence-eta" style={{ fontSize: 11, color: critical ? "var(--ot-gauge-bad)" : "var(--ot-con-amber)", fontVariantNumeric: "tabular-nums", whiteSpace: "nowrap" }}>
            ⏳~{Math.max(1, Math.round(eta))}m
          </span>
        )}
      </div>
      <div data-testid="towns-captured" style={{ color: "var(--ot-con-ink-dim)", cursor: "help" }} title="Towns conquered. Each captured town pushes the decadence back — your main brake on the doom clock.">
        🏰 <b style={{ color: "var(--ot-con-ink)" }}>{towns}</b> taken
      </div>
      {armies > 0 && (
        <div data-testid="armies" style={{ color: "var(--ot-con-ink-dim)" }} title="Legions afield — AI-led, riding your rails, baited by bounties.">
          ⚔ {armies} legion{armies === 1 ? "" : "s"}
        </div>
      )}
      {Math.round(s.raiderCount) > 0 && (
        <div data-testid="raiders" style={{ color: "#8fc95f", fontWeight: 600 }} title="Decadence raiders, in three roles (watch their badge): ☣ breachers march your capital (deepen the rot), ✂ saboteurs cut your over-extended rail, ⚑ reclaimers re-take towns you haven't railed to. Your station cordon cuts them down — cover the approaches + hold your ground.">
          ☣ {Math.round(s.raiderCount)} raider{Math.round(s.raiderCount) === 1 ? "" : "s"}
        </div>
      )}
      {Math.round(s.spellsCast) > 0 && (
        <div data-testid="spells" style={{ color: "#b69bef", fontWeight: 600 }} title="Spells cast (Purge / Smite / Warpath), drawn from your mana — cast them from the spell bar, or toggle autocast.">
          ✦ {Math.round(s.spellsCast)} cast
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
  const ridershipRef = useTweenedNumber(s.ridershipTotal, fmtCount); // abbreviate the headline (5.7k, not 5747)
  const coverageRef = useTweenedNumber(s.coverageScore, fmtInt); // 0..100 — no abbreviation

  // Mode-aware: the fantasy campaign shows its own supply/conquest/decadence readout.
  if (s.ruleset === "arcadia") return <FantasyStatsBar s={s} />;

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
    (denied > 0 ? ` · ${fmtCount(denied)} passed by full trains` : "");

  // Bar fills left→right. Coverage is a progression dial (it starts near 0 on a fresh map), so
  // the low band is neutral — not failure-red — and the hue only turns good once it's earned.
  const covColor = coverageColor(c); // #25 the shared single-source band (this was the canonical neutral-low one)

  // Network strain: the fleet's mean load (avgLoadFactor) as a loadPip, with the live train count
  // in its tooltip. Mounts only once trains run, so it's never dead chrome (like the money box).
  // Reuses the loadPip shape/colour language so "crush" reads the same here as on a train or line.
  const trains = s.vehicleCount;
  const netPip = loadPip(s.avgLoadFactor);

  return (
    <div id="stats-bar" data-testid="stats-bar" className="ot-console" style={BAR_STYLE}>
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
            background: "var(--ot-well-bg)",
            boxShadow: "var(--ot-well)",
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
              background: covColor,
              borderRadius: "6px",
              // Gauge fill eases toward each new coverage value; hue cross-fades good→bad (juice).
              transition: "width .5s var(--ot-ease), background-color .4s linear",
            }}
          ></div>
        </div>
        <b ref={coverageRef} data-testid="coverage" style={{ width: "26px", textAlign: "right", fontVariantNumeric: "tabular-nums" }} />
      </div>
      <div style={{ color: "var(--ot-con-ink-dim)", cursor: "help" }} title={waitTip}>
        <span data-testid="waiting">{fmtCount(w)}</span> waiting
        {lost > 0 && (
          <span data-testid="left-behind" style={{ color: "var(--ot-gauge-bad)", marginLeft: "6px" }}>
            · {fmtCount(lost)} gave up
          </span>
        )}
      </div>
      {/* Network strain (run only): mean fleet load + live train count. Part of the pressure
          cluster, not a new headline — surfaces avgLoadFactor + vehicleCount which were computed
          but never shown. Mounts only with trains running, so it's never dead chrome. */}
      {trains > 0 && (
        <div
          data-testid="net-load"
          style={{ color: "var(--ot-con-ink-dim)", cursor: "help" }}
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
          ruleset on, so it's never dead chrome. */}
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
