// The "one more day" beats: a day-rollover report card (the turn punctuation) and milestone
// toasts (reward beats between days). Both are pure reads of the 3 Hz Stats slice — no Commands,
// no sim state, no per-frame work. Mounted by App inside the GameProvider.
//
// Baselines initialise from the FIRST snapshot (loading the real network at coverage 41 must not
// fire eight milestones), and reset silently when simDay moves backwards (undo / load rebuilds
// the world).
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { useStats } from "./GameContext";
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

const CARD: CSSProperties = {
  position: "fixed",
  top: 92,
  left: "50%",
  transform: "translateX(-50%)",
  zIndex: 15,
  background: "rgba(255,255,255,.97)",
  color: "#1c2024",
  borderRadius: 12,
  boxShadow: "0 6px 24px rgba(0,0,0,.25)",
  padding: "10px 14px",
  font: "13px system-ui,sans-serif",
  minWidth: 240,
};

function Row({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", gap: 16, padding: "1px 0" }}>
      <span style={{ color: "#7a818a" }}>{label}</span>
      <b style={{ color: tone ?? "#1c2024", fontVariantNumeric: "tabular-nums" }}>{value}</b>
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
        day: prevDay.current + 1,
        riders: s.ridershipTotal - d.riders,
        coverage: s.coverageScore,
        coverageDelta: s.coverageScore - d.coverage,
        gaveUp: s.abandoned - d.abandoned,
        grewPct,
        fares: s.fareRevenue - d.fares,
        economyOn: s.economyEnabled,
      });
      audio.place(); // a soft beat, not the celebratory connect chime
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
    <div data-testid="day-report" style={CARD} onClick={() => setReport(null)}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 8, marginBottom: 4 }}>
        <b style={{ fontSize: 14 }}>🌅 Day {report.day} complete</b>
        <span style={{ marginLeft: "auto", color: "#9aa1a9", fontSize: 11 }}>click to dismiss</span>
      </div>
      <Row label="Riders carried" value={sign(Math.round(report.riders))} tone="#0072b2" />
      <Row label="Coverage" value={`${report.coverage}${report.coverageDelta !== 0 ? ` (${sign(report.coverageDelta)})` : ""}`} />
      {report.gaveUp > 0 && <Row label="Gave up waiting" value={sign(Math.round(report.gaveUp))} tone="var(--ot-gauge-bad,#d62828)" />}
      {report.grewPct >= 0.5 && (
        <Row label="🏙 The city grew" value={`+${report.grewPct.toFixed(1)}% demand`} tone="var(--ot-gauge-good,#009e73)" />
      )}
      {report.economyOn && <Row label="Fares" value={`+$${Math.round(report.fares / 1e6)}M`} />}
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
  const run = useRef(bootRun());
  const nextRider = useRef<number | null>(null);
  const lastCovStep = useRef<number | null>(null);
  const beatAnchor = useRef(false);
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
    if (!showing && queue.current.length > 0) {
      setShowing(queue.current.shift() ?? null);
      audio.connect();
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
        background: "linear-gradient(180deg,#fff8e6,#ffefc2)",
        border: "1px solid #e8c96a",
        color: "#5a4a12",
        borderRadius: 999,
        boxShadow: "0 4px 16px rgba(0,0,0,.2)",
        padding: "7px 16px",
        font: "700 13px system-ui,sans-serif",
        whiteSpace: "nowrap",
      }}
    >
      {showing}
    </div>
  );
}
