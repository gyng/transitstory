// Objectives / scenario layer — a PURE outer-ring goal evaluator. It reads only the Stats
// snapshot the sim already produces (ridership, coverage, abandoned, balance, clock) plus a
// committed scenario definition, and decides active / won / lost. Zero sim coupling: no Command,
// no core change, no new build gesture — it turns the sandbox into a scored attempt.
import type { Stats } from "./types";

export interface Goal {
  kind: "ridership" | "coverage";
  /** Reach >= this value. ridership = cumulative boardings; coverage = the 0–100 gauge. */
  target: number;
  label: string;
}

export interface Scenario {
  id: string;
  title: string;
  blurb: string;
  goals: Goal[];
  /** Win must happen before this much ELAPSED sim time (ms), else the scenario is lost. */
  deadlineMs?: number;
  /** Lose if cumulative left-behind (abandoned) exceeds this. */
  failIfAbandonedOver?: number;
  /** Lose if the (enabled) economy balance goes negative. */
  failBankrupt?: boolean;
}

export type Status = "active" | "won" | "lost";

export interface GoalState {
  goal: Goal;
  current: number;
  met: boolean;
}

export interface ScenarioEval {
  goals: GoalState[];
  allMet: boolean;
  /** True once a hard fail condition (deadline / abandonment / bankruptcy) has tripped. */
  failed: boolean;
  failReason: string | null;
}

/** Committed, city-agnostic challenges (frontend content — deterministic, not the sim). */
export const SCENARIOS: Record<string, Scenario> = {
  starter: {
    id: "starter",
    title: "First Line",
    blurb: "Get a line moving and carry your first passengers.",
    goals: [{ kind: "ridership", target: 300, label: "Carry 300 riders" }],
  },
  "coverage-sprint": {
    id: "coverage-sprint",
    title: "Coverage Sprint",
    blurb: "Cover the city fast — and don't strand too many riders.",
    goals: [
      { kind: "coverage", target: 55, label: "Reach 55 coverage" },
      { kind: "ridership", target: 1500, label: "Carry 1,500 riders" },
    ],
    deadlineMs: 25 * 60_000,
    failIfAbandonedOver: 400,
  },
  metropolis: {
    id: "metropolis",
    title: "Metropolis",
    blurb: "Build a network worthy of the name.",
    goals: [
      { kind: "coverage", target: 80, label: "Reach 80 coverage" },
      { kind: "ridership", target: 8000, label: "Carry 8,000 riders" },
    ],
    deadlineMs: 45 * 60_000,
  },
};

export function getScenario(id: string | null | undefined): Scenario | null {
  return id ? SCENARIOS[id] ?? null : null;
}

/** Evaluate goal progress + hard-fail conditions against a stats snapshot (pure). */
export function evalScenario(scenario: Scenario, stats: Stats): ScenarioEval {
  const goals: GoalState[] = scenario.goals.map((goal) => {
    const current = goal.kind === "ridership" ? stats.ridershipTotal : stats.coverageScore;
    return { goal, current, met: current >= goal.target };
  });
  const allMet = goals.every((g) => g.met);

  let failed = false;
  let failReason: string | null = null;
  if (scenario.failIfAbandonedOver !== undefined && stats.abandoned > scenario.failIfAbandonedOver) {
    failed = true;
    failReason = `Too many riders left behind (${Math.round(stats.abandoned)})`;
  } else if (scenario.failBankrupt && stats.economyEnabled && stats.balance < 0) {
    failed = true;
    failReason = "Bankrupt";
  } else if (scenario.deadlineMs !== undefined && stats.simClockMs > scenario.deadlineMs && !allMet) {
    failed = true;
    failReason = "Out of time";
  }

  return { goals, allMet, failed, failReason };
}

/** Fold the previous status forward (status is sticky: a win/loss never reverts). */
export function nextStatus(prev: Status, e: ScenarioEval): Status {
  if (prev !== "active") return prev;
  if (e.allMet) return "won";
  if (e.failed) return "lost";
  return "active";
}
