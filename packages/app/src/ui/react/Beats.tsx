// The "one more day" beats: a day-rollover report card (the turn punctuation) and milestone
// toasts (reward beats between days). Both are pure reads of the 3 Hz Stats slice — no Commands,
// no sim state, no per-frame work. Mounted by App inside the GameProvider.
//
// Baselines initialise from the FIRST snapshot (loading the real network at coverage 41 must not
// fire eight milestones), and reset silently when simDay moves backwards (undo / load rebuilds
// the world).
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { useGame, useStats } from "./GameContext";
import { fmtCount, fmtMoney } from "./shared";
import { audio } from "../../fx/audio";
import { cityById, recordBest } from "../../sim/cities";

/** This run's boot setup, from the URL (set by both deep links and the menu via replaceState).
 *  Score-chase rules: only a from-scratch run (no pre-loaded network) competes with the city's
 *  real-network anchor or records a personal best. */
function bootRun(): { cityId: string; fromScratch: boolean; anchor: number; cityName: string } {
  const q = new URLSearchParams(location.search);
  const entry = cityById(q.get("city"));
  const fromScratch = q.get("network") !== "1" && entry.id !== "globe"; // the globe always loads its board
  return { cityId: entry.id, fromScratch, anchor: entry.realScore, cityName: entry.name };
}

/** Snapshot of the counters a day report diffs against (taken at each day start). */
interface DayStart {
  riders: number;
  coverage: number;
  abandoned: number;
  fares: number;
  demand: number;
}

interface DayReportData {
  day: number; // 1-based display index of the completed day
  riders: number;
  coverage: number;
  coverageDelta: number;
  gaveUp: number;
  grewPct: number; // city growth over the day, in % of total origin demand
  fares: number;
  economyOn: boolean;
}

// The day-report panel face — a brushed-graphite console card (#28 diegetic theme); the .ot-console
// class owns its surface/border/shadow/ink, so only layout lives inline.
const CARD: CSSProperties = {
  position: "fixed",
  top: 92,
  left: "50%",
  transform: "translateX(-50%)",
  zIndex: 15,
  padding: "10px 14px",
  font: "13px system-ui,sans-serif",
  minWidth: 240,
};

function Row({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", gap: 16, padding: "1px 0" }}>
      <span style={{ color: "var(--ot-con-ink-dim)" }}>{label}</span>
      <b style={{ color: tone ?? "var(--ot-con-ink)", fontVariantNumeric: "tabular-nums" }}>{value}</b>
    </div>
  );
}

/** Day-rollover report: when simDay advances, show what the completed day did — riders carried,
 *  coverage movement, riders lost, and how much the city grew (transit-oriented growth). */
export function DayReport() {
  const s = useStats();
  const prevDay = useRef<number | null>(null);
  const dayStart = useRef<DayStart | null>(null);
  const [report, setReport] = useState<DayReportData | null>(null);

  const snap = (): DayStart => ({
    riders: s.ridershipTotal,
    coverage: s.coverageScore,
    abandoned: s.abandoned,
    fares: s.fareRevenue,
    demand: s.demandOriginTotal,
  });

  useEffect(() => {
    if (prevDay.current === null || s.simDay < prevDay.current) {
      // First snapshot, or the world was rebuilt (undo/load): baseline silently.
      prevDay.current = s.simDay;
      dayStart.current = snap();
      return;
    }
    if (s.simDay > prevDay.current && dayStart.current) {
      const d = dayStart.current;
      const grewPct = d.demand > 0 ? ((s.demandOriginTotal - d.demand) / d.demand) * 100 : 0;
      setReport({
        // #12 label by the just-completed day INDEX (= the new simDay), not prevDay+1 — on a >1-day jump (3× speed
        // or a throttled background tab) the diffs below cover several days, so "Day 1" would mislabel a Day-3 report.
        day: s.simDay,
        riders: s.ridershipTotal - d.riders,
        coverage: s.coverageScore,
        coverageDelta: s.coverageScore - d.coverage,
        gaveUp: s.abandoned - d.abandoned,
        grewPct,
        fares: s.fareRevenue - d.fares,
        economyOn: s.economyEnabled,
      });
      audio.day(); // the day-rollover chime (a page turning) under the report card
      prevDay.current = s.simDay;
      dayStart.current = snap();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s]);

  // Auto-dismiss after 10 s (a beat, not a modal — the game keeps running underneath).
  useEffect(() => {
    if (!report) return;
    const id = window.setTimeout(() => setReport(null), 10_000);
    return () => window.clearTimeout(id);
  }, [report]);

  if (!report) return null;
  const sign = (n: number) => (n > 0 ? `+${n}` : `${n}`);
  return (
    <div data-testid="day-report" className="ot-console" style={CARD} onClick={() => setReport(null)}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 8, marginBottom: 4 }}>
        <b style={{ fontSize: 14, color: "var(--ot-con-ink)" }}>🌅 Day {report.day} complete</b>
        <span style={{ marginLeft: "auto", color: "var(--ot-con-ink-dim)", fontSize: 11 }}>click to dismiss</span>
      </div>
      <Row label="Riders carried" value={`+${fmtCount(report.riders)}`} tone="var(--ot-con-accent)" />
      <Row label="Coverage" value={`${report.coverage}${report.coverageDelta !== 0 ? ` (${sign(report.coverageDelta)})` : ""}`} />
      {report.gaveUp > 0 && <Row label="Gave up waiting" value={`+${fmtCount(report.gaveUp)}`} tone="var(--ot-con-red)" />}
      {report.grewPct >= 0.5 && (
        <Row label="🏙 The city grew" value={`+${report.grewPct.toFixed(1)}% demand`} tone="var(--ot-gauge-good,#009e73)" />
      )}
      {report.economyOn && <Row label="Fares" value={`+${fmtMoney(report.fares)}`} />}
    </div>
  );
}

/** Cumulative-rider thresholds worth a beat. Coverage beats fire every +5 from 10. */
const RIDER_MILESTONES = [100, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000, 250_000];

/** Milestone toasts: small celebratory pills for new progress only (baseline-aware). Also the
 *  score-chase bookkeeping: from-scratch runs record a per-city personal best, and crossing the
 *  city's real-network anchor is the headline beat. */
export function Milestones() {
  const s = useStats();
  const game = useGame();
  const run = useRef(bootRun());
  const nextRider = useRef<number | null>(null);
  const lastCovStep = useRef<number | null>(null);
  const beatAnchor = useRef(false);
  const dayNightBeat = useRef(false); // #daynight: teach the march/camp mechanic once, the first night legions camp
  const queue = useRef<string[]>([]);
  const [showing, setShowing] = useState<string | null>(null);

  useEffect(() => {
    if (nextRider.current === null || lastCovStep.current === null) {
      // Baseline: only milestones BEYOND the starting state count (a loaded real network
      // starts at coverage ~40 — that's the city's achievement, not the player's).
      nextRider.current = RIDER_MILESTONES.findIndex((m) => s.ridershipTotal < m);
      if (nextRider.current < 0) nextRider.current = RIDER_MILESTONES.length;
      lastCovStep.current = Math.floor(s.coverageScore / 5) * 5;
      beatAnchor.current = s.coverageScore > run.current.anchor; // pre-passed never re-fires
      return;
    }
    while (nextRider.current < RIDER_MILESTONES.length && s.ridershipTotal >= RIDER_MILESTONES[nextRider.current]) {
      queue.current.push(`🏅 ${RIDER_MILESTONES[nextRider.current].toLocaleString()} riders carried!`);
      nextRider.current += 1;
    }
    const step = Math.floor(s.coverageScore / 5) * 5;
    if (step > lastCovStep.current && step >= 10) {
      queue.current.push(`📈 Coverage ${step} — the city feels it`);
      lastCovStep.current = step;
    }
    if (run.current.fromScratch) {
      recordBest(run.current.cityId, s.coverageScore);
      if (!beatAnchor.current && s.coverageScore > run.current.anchor) {
        beatAnchor.current = true;
        queue.current.push(`🏆 You beat the real ${run.current.cityName} network (${run.current.anchor})!`);
      }
    }
    // #daynight: teach the march/camp rule the first time legions are afield AND it's night — they hold
    // camp till dawn (your rail keeps moving), so the player learns to expect the cyclic advance.
    const afield = s.armyAfield ?? s.armyCount ?? 0;
    if (!dayNightBeat.current && afield > 0 && !(s.simHour >= 6 && s.simHour < 20)) {
      dayNightBeat.current = true;
      queue.current.push("🌙 Your legions make camp for the night — they march on at dawn (your rail runs all night)");
    }
    if (!showing && queue.current.length > 0) {
      setShowing(queue.current.shift() ?? null);
      audio.milestone(); // a brighter arpeggio than `connect` — the achievement reads as one
      game.celebrateMilestone(); // a celebration spray at the busiest station (reduced-motion → a single ack)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s, showing]);

  useEffect(() => {
    if (!showing) return;
    const id = window.setTimeout(() => setShowing(null), 3_500);
    return () => window.clearTimeout(id);
  }, [showing]);

  if (!showing) return null;
  return (
    <div
      data-testid="milestone"
      style={{
        position: "fixed",
        top: 148,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 15,
        pointerEvents: "none",
        // A celebratory amber beat on the operator's desk: graphite face, the achievement glowing in
        // console-amber (kept its gold meaning, grounded dark so it reads light-on-dark like the chrome).
        background: "var(--ot-con-panel)",
        border: "1px solid var(--ot-con-edge)",
        color: "var(--ot-con-amber)",
        borderRadius: 999,
        boxShadow: "var(--ot-con-elev), 0 0 14px rgba(241,173,68,.35)",
        textShadow: "0 1px 2px rgba(0,0,0,.5)",
        padding: "7px 16px",
        font: "700 13px system-ui,sans-serif",
        whiteSpace: "nowrap",
      }}
    >
      {showing}
    </div>
  );
}
