// Rolling time-series of the Stats snapshot, accumulated on the frontend (the sim is stateless
// about history — determinism keeps no time-series in hashed state, so trend charts live here).
// A module-level ring buffer fed by an always-mounted <StatsRecorder/>, so history keeps building
// even while the dashboard is closed. One sample per SAMPLE_EVERY_MS of SIM time (not wall time),
// so a chart spans many sim-hours regardless of game speed. Pure read of the existing useStats
// slice — no new sim coupling, no per-frame work (the recorder runs on the ~3 Hz stats cadence).
import { useEffect, useState } from "react";
import type { Stats } from "../../types";
import { useStats } from "./GameContext";

export interface Sample {
  clockMs: number;
  ridership: number;
  balance: number;
  fareRevenue: number;
  opexSpent: number;
  capitalSpent: number;
  coverage: number;
  waiting: number;
  abandoned: number;
  deniedBoardings: number;
  avgLoad: number;
  avgWaitMs: number;
}

/** Max retained samples; older ones are dropped (the chart window slides). */
const MAX = 360;
/** Minimum SIM-time gap between samples (1 sim-minute) — bounds buffer growth + chart density. */
const SAMPLE_EVERY_MS = 60_000;

let HISTORY: Sample[] = [];
let lastClock = Number.NEGATIVE_INFINITY;
const subs = new Set<() => void>();

function emit() {
  for (const f of subs) f();
}

/** Record a sample if the sim clock has advanced enough. Resets if the clock went backwards
 *  (a new game, or an undo that rebuilds the world from seed + log) so charts don't smear runs. */
export function recordStats(s: Stats): void {
  const clockMs = s.simClockMs;
  if (clockMs < lastClock) {
    HISTORY = [];
    lastClock = Number.NEGATIVE_INFINITY;
  }
  if (HISTORY.length > 0 && clockMs - lastClock < SAMPLE_EVERY_MS) return;
  lastClock = clockMs;
  HISTORY.push({
    clockMs,
    ridership: s.ridershipTotal,
    balance: s.balance,
    fareRevenue: s.fareRevenue,
    opexSpent: s.opexSpent,
    capitalSpent: s.capitalSpent,
    coverage: s.coverageScore,
    waiting: s.waitingTotal,
    abandoned: s.abandoned,
    deniedBoardings: s.deniedBoardings,
    avgLoad: s.avgLoadFactor,
    avgWaitMs: s.avgWaitMs,
  });
  if (HISTORY.length > MAX) HISTORY = HISTORY.slice(HISTORY.length - MAX);
  emit();
}

export function getHistory(): Sample[] {
  return HISTORY;
}

/** Invisible component — mount ONCE near the root so history accrues regardless of dashboard
 *  open/closed. Records on every stats-slice change (the ~3 Hz cadence), gated by sim-time. */
export function StatsRecorder(): null {
  const s = useStats();
  useEffect(() => {
    recordStats(s);
  }, [s]);
  return null;
}

/** Subscribe a component to history growth — re-renders it when a new sample lands. */
export function useStatsHistory(): Sample[] {
  const [, bump] = useState(0);
  useEffect(() => {
    const f = () => bump((n) => n + 1);
    subs.add(f);
    return () => {
      subs.delete(f);
    };
  }, []);
  return HISTORY;
}
