// S11 PRESTIGE — the "realms saved" meta-count (arcadia campaigns won), localStorage-backed and sim-free
// (the fantasy counterpart to personalBest). Tested with a minimal localStorage shim (the vitest env is
// "node"), plus the graceful-degradation path when storage is unavailable.
import { beforeEach, describe, expect, it } from "vitest";
import { realmsSaved, recordRealmSaved } from "../src/sim/cities";
import { SCENARIOS } from "../src/objectives";

beforeEach(() => {
  const store = new Map<string, string>();
  (globalThis as unknown as { localStorage: Storage }).localStorage = {
    getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
    clear: () => store.clear(),
    key: () => null,
    length: 0,
  } as Storage;
});

describe("prestige (realms saved)", () => {
  it("starts at 0 and increments on each realm saved", () => {
    expect(realmsSaved()).toBe(0);
    expect(recordRealmSaved()).toBe(1);
    expect(recordRealmSaved()).toBe(2);
    expect(realmsSaved()).toBe(2);
  });

  it("degrades gracefully without storage (private mode etc.)", () => {
    delete (globalThis as unknown as { localStorage?: Storage }).localStorage;
    expect(realmsSaved()).toBe(0);
    expect(recordRealmSaved()).toBe(0); // no throw — just doesn't persist
  });

  it("the arcadia campaign is the prestige scenario", () => {
    // Winning "Against the Dark" is what records a realm saved (the Objectives panel calls recordRealmSaved
    // on the sticky win of a prestige scenario). The transit scenarios are NOT prestige.
    expect(SCENARIOS["arcadia-conquest"].prestige).toBe(true);
    expect(SCENARIOS.starter.prestige).toBeUndefined();
  });
});
