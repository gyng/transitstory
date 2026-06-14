// Objectives / scenario layer — a PURE outer-ring goal evaluator. It reads only the Stats
// snapshot the sim already produces (ridership, coverage, abandoned, balance, clock) plus a
// committed scenario definition, and decides active / won / lost. Zero sim coupling: no Command,
// no core change, no new build gesture — it turns the sandbox into a scored attempt.
import type { Stats } from "./types";

export interface Goal {
  // transit goals: ridership (cumulative boardings) + coverage (the 0–100 gauge). arcadia goals (S11):
  // towns (conquered) + tribute (accumulated supply) + standing (= the arcadia coverage/standing gauge).
  kind: "ridership" | "coverage" | "towns" | "tribute" | "standing";
  /** Reach >= this value. */
  target: number;
  label: string;
}

export interface Scenario {
  id: string;
  title: string;
  blurb: string;
  goals: Goal[];
  /** If set, this scenario is offered ONLY for that city id (e.g. the globe air board); scenarios
   *  with no cityId are universal challenges shown for every city. */
  cityId?: string;
  /** Win must happen before this much ELAPSED sim time (ms), else the scenario is lost. */
  deadlineMs?: number;
  /** Lose if cumulative left-behind (abandoned) exceeds this. */
  failIfAbandonedOver?: number;
  /** Lose if the (enabled) economy balance goes negative. */
  failBankrupt?: boolean;
  /** Lose if the realm falls — decadence overruns the capital (arcadia campaign, S11). */
  failIfRealmLost?: boolean;
  /** Winning this scenario records a PRESTIGE ("realm saved", S11) — a localStorage meta-count shown in
   *  the menu. Sim-free outer-ring meta; set on the arcadia campaign victory. */
  prestige?: boolean;
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
  // Coverage targets are calibrated against measured anchors on the re-denominated gauge
  // (coverage = sqrt(% of the WHOLE city's demand served well)): one good first line ≈ 7,
  // the full real Singapore MRT ≈ 41, the globe's flagship air board ≈ 64 (re-measured post clock-unification).
  "coverage-sprint": {
    id: "coverage-sprint",
    title: "Coverage Sprint",
    blurb: "Cover the city fast — and don't strand too many riders.",
    goals: [
      { kind: "coverage", target: 25, label: "Reach 25 coverage" },
      { kind: "ridership", target: 1500, label: "Carry 1,500 riders" },
    ],
    deadlineMs: 25 * 60_000,
    failIfAbandonedOver: 400,
  },
  metropolis: {
    id: "metropolis",
    title: "Metropolis",
    blurb: "Build a network worthy of the name — bigger than the real one.",
    goals: [
      { kind: "coverage", target: 45, label: "Reach 45 coverage (the real MRT scores ~41)" },
      { kind: "ridership", target: 8000, label: "Carry 8,000 riders" },
    ],
    deadlineMs: 45 * 60_000,
  },
  "globe-airline": {
    id: "globe-airline",
    title: "Global Airline",
    blurb: "Span the planet by air — open routes between the world's metros and connect them all.",
    cityId: "globe",
    goals: [
      { kind: "coverage", target: 50, label: "Connect the world (50 coverage)" },
      { kind: "ridership", target: 20000, label: "Fly 20,000 intercity passengers" },
    ],
    deadlineMs: 60 * 60_000,
  },
  // The arcadia campaign's scored VICTORY (S11): the fork has a lose state (the realm falls) but the win
  // was open-ended — this closes the loop. Supply your towns, field legions, conquer the continent, and
  // hold standing — all before the decadence overruns the capital. No deadline: the rot IS the clock.
  "arcadia-conquest": {
    id: "arcadia-conquest",
    title: "Against the Dark",
    blurb: "Conquer the continent and hold your standing — before the decadence reaches the capital.",
    cityId: "fantasy",
    goals: [
      { kind: "towns", target: 3, label: "Conquer 3 towns" },
      { kind: "standing", target: 20, label: "Reach 20 realm standing" },
    ],
    failIfRealmLost: true,
    prestige: true,
  },
};

export function getScenario(id: string | null | undefined): Scenario | null {
  return id ? SCENARIOS[id] ?? null : null;
}

/** Evaluate goal progress + hard-fail conditions against a stats snapshot (pure). */
export function evalScenario(scenario: Scenario, stats: Stats): ScenarioEval {
  const goalValue = (kind: Goal["kind"]): number => {
    switch (kind) {
      case "ridership": return stats.ridershipTotal;
      case "towns": return stats.townsCaptured;
      case "tribute": return stats.tribute;
      // "standing" is the arcadia name for the same 0–100 coverage gauge (supply reach + conquest).
      case "coverage":
      case "standing": return stats.coverageScore;
    }
  };
  const goals: GoalState[] = scenario.goals.map((goal) => {
    const current = goalValue(goal.kind);
    return { goal, current, met: current >= goal.target };
  });
  const allMet = goals.every((g) => g.met);

  let failed = false;
  let failReason: string | null = null;
  if (scenario.failIfRealmLost && stats.realmLost) {
    failed = true;
    failReason = "The realm has fallen — decadence overran the capital";
  } else if (scenario.failIfAbandonedOver !== undefined && stats.abandoned > scenario.failIfAbandonedOver) {
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
