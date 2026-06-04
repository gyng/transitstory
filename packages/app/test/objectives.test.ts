// The objectives evaluator is pure (reads a Stats snapshot, decides progress + fail), so it's
// unit-tested without the sim. The win/lose stickiness lives in nextStatus.
import { describe, expect, it } from "vitest";
import { evalScenario, nextStatus, SCENARIOS } from "../src/objectives";
import type { Stats } from "../src/types";

function stats(p: Partial<Stats>): Stats {
  return {
    ridershipTotal: 0,
    coverageScore: 0,
    abandoned: 0,
    balance: 0,
    economyEnabled: false,
    simClockMs: 0,
    ...p,
  } as unknown as Stats;
}

describe("objectives evaluator", () => {
  it("meets a ridership goal when the target is reached", () => {
    const sc = SCENARIOS.starter;
    expect(evalScenario(sc, stats({ ridershipTotal: 299 })).allMet).toBe(false);
    expect(evalScenario(sc, stats({ ridershipTotal: 300 })).allMet).toBe(true);
  });

  it("requires ALL goals for a multi-goal scenario", () => {
    const sc = SCENARIOS["coverage-sprint"];
    expect(evalScenario(sc, stats({ coverageScore: 55, ridershipTotal: 1499 })).allMet).toBe(false);
    expect(evalScenario(sc, stats({ coverageScore: 55, ridershipTotal: 1500 })).allMet).toBe(true);
  });

  it("fails on too many abandoned riders", () => {
    const sc = SCENARIOS["coverage-sprint"];
    const e = evalScenario(sc, stats({ abandoned: 401 }));
    expect(e.failed).toBe(true);
    expect(e.failReason).toMatch(/left behind/i);
  });

  it("fails when the deadline passes without all goals met", () => {
    const sc = SCENARIOS["coverage-sprint"];
    const past = (sc.deadlineMs ?? 0) + 1;
    expect(evalScenario(sc, stats({ simClockMs: past })).failed).toBe(true);
    // ...but not if everything is already done at the deadline.
    expect(
      evalScenario(sc, stats({ simClockMs: past, coverageScore: 99, ridershipTotal: 9999 })).failed,
    ).toBe(false);
  });

  it("status is sticky: a win never reverts", () => {
    const won = nextStatus("won", evalScenario(SCENARIOS.starter, stats({ ridershipTotal: 0 })));
    expect(won).toBe("won");
    const lost = nextStatus("lost", evalScenario(SCENARIOS.starter, stats({ ridershipTotal: 9999 })));
    expect(lost).toBe("lost");
  });
});
