// Objective tracker + win/lose banner — pure outer-ring chrome. Reads the ~3 Hz stats slice,
// evaluates a Scenario, and renders progress + a sticky end-state. Issues no Commands; the sim
// is untouched. The card sits top-right (the EditorPanel's top-right space is empty until a
// selection); the end banner is a centred dismissible overlay so the player can keep building.
import { useEffect, useRef, useState } from "react";
import { useStats } from "./GameContext";
import { evalScenario, nextStatus, type Scenario, type Status } from "../../objectives";
import { recordRealmSaved } from "../../sim/cities";

export function ObjectivePanel({ scenario }: { scenario: Scenario }) {
  const stats = useStats();
  const [status, setStatus] = useState<Status>("active");
  const [dismissedBanner, setDismissedBanner] = useState(false);
  const statusRef = useRef<Status>("active");

  const e = evalScenario(scenario, stats);

  // Status is sticky: the first win/loss freezes it (a later dip never reverts it).
  useEffect(() => {
    const ns = nextStatus(statusRef.current, e);
    if (ns !== statusRef.current) {
      // S11 PRESTIGE: the first WIN of a prestige scenario (the arcadia campaign) records a "realm saved"
      // — once, on the sticky transition (statusRef guards against the 3 Hz re-fire). Sim-free meta.
      if (ns === "won" && scenario.prestige) recordRealmSaved();
      statusRef.current = ns;
      setStatus(ns);
    }
  }, [e.allMet, e.failed]);

  const showBanner = status !== "active" && !dismissedBanner;

  // Publish this card's (data-derived, not DOM-measured) height so the transient EditorPanel
  // can stack BELOW it via `top: calc(56px + var(--ot-objective-h))` — they share the top-right
  // anchor and used to overlap. Reset to 0px on unmount (no scenario → editor returns to top).
  const goalCount = e.goals.length;
  const hasDeadline = scenario.deadlineMs !== undefined;
  useEffect(() => {
    const h = 78 + goalCount * 20 + (hasDeadline ? 22 : 0); // base + per-goal row + deadline, incl. gap
    document.documentElement.style.setProperty("--ot-objective-h", `${h}px`);
    return () => document.documentElement.style.setProperty("--ot-objective-h", "0px");
  }, [goalCount, hasDeadline]);

  return (
    <>
      <div
        data-testid="objectives"
        className="ot-console"
        style={{
          position: "fixed",
          top: 56,
          right: 10,
          zIndex: 9,
          width: 220,
          padding: "10px 12px",
          font: "13px system-ui,sans-serif",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
          <b>🎯 {scenario.title}</b>
          <span
            data-testid="objective-status"
            style={{
              fontSize: 11,
              color: status === "won" ? "var(--ot-gauge-good,#009e73)" : status === "lost" ? "var(--ot-gauge-bad,#d62828)" : "var(--ot-con-ink-dim)",
            }}
          >
            {status === "active" ? "in progress" : status}
          </span>
        </div>
        {/* The end state leaves a persistent retry affordance (the banner is dismissible, the
            outcome isn't): reload re-boots this exact city+scenario via the URL params. */}
        {status !== "active" && (
          <button
            data-testid="objective-retry"
            className="ot-key"
            onClick={() => window.location.reload()}
            style={{
              width: "100%",
              margin: "6px 0 2px",
              padding: "5px 0",
              font: "600 12px system-ui",
              cursor: "pointer",
            }}
          >
            ↻ {status === "won" ? "Play it again" : "Retry the challenge"}
          </button>
        )}
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, margin: "2px 0 8px" }}>{scenario.blurb}</div>
        {e.goals.map((g, i) => (
          <div key={i} data-testid={`objective-goal-${g.goal.kind}`} style={{ display: "flex", justifyContent: "space-between", padding: "2px 0" }}>
            <span style={{ color: g.met ? "var(--ot-gauge-good,#009e73)" : "var(--ot-con-ink)" }}>
              {g.met ? "✓" : "○"} {g.goal.label}
            </span>
            <span data-testid={`objective-goal-${g.goal.kind}-current`} style={{ color: "var(--ot-con-ink-dim)", fontVariantNumeric: "tabular-nums" }}>
              {Math.round(g.current)}/{g.goal.target}
            </span>
          </div>
        ))}
        {scenario.deadlineMs !== undefined && status === "active" && (
          <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, marginTop: 6 }}>
            {/* Deadlines are SESSION time (sim-ms ≈ wall-ms at 1×), not in-game clock minutes —
                disambiguated since every other duration now reads in clock units. */}
            ⏳ {Math.max(0, Math.ceil((scenario.deadlineMs - stats.simClockMs) / 60_000))} min left (real time)
          </div>
        )}
      </div>

      {showBanner && (
        <div
          data-testid="objective-banner"
          onClick={() => setDismissedBanner(true)}
          style={{
            position: "fixed",
            inset: 0,
            zIndex: 30,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,.35)",
            cursor: "pointer",
          }}
        >
          <div
            className="ot-console"
            style={{
              padding: "26px 34px",
              textAlign: "center",
              maxWidth: 360,
            }}
          >
            <div style={{ fontSize: 40 }}>{status === "won" ? "🎉" : "⏱️"}</div>
            <h2 style={{ margin: "8px 0 4px", color: status === "won" ? "var(--ot-gauge-good,#009e73)" : "var(--ot-gauge-bad,#d62828)" }}>
              {status === "won" ? `${scenario.title} complete!` : "Challenge failed"}
            </h2>
            <p style={{ margin: "0 0 14px", color: "var(--ot-con-ink-dim)", fontSize: 14 }}>
              {status === "won" ? "Every objective met. Keep building, or start a fresh challenge." : e.failReason ?? "Better luck next time."}
            </p>
            <div style={{ display: "flex", gap: 8, justifyContent: "center" }}>
              <button
                data-testid="banner-retry"
                // On a loss the retry is the primary action (lit key); on a win it's the encore (plain key).
                className={`ot-key ${status === "lost" ? "on" : ""}`}
                onClick={(ev) => {
                  ev.stopPropagation();
                  window.location.reload();
                }}
                style={{
                  padding: "9px 22px",
                  font: "700 14px system-ui",
                  cursor: "pointer",
                }}
              >
                ↻ {status === "won" ? "Play again" : "Retry"}
              </button>
              <button
                className={`ot-key ${status === "won" ? "on" : ""}`}
                style={{
                  padding: "9px 22px",
                  font: "700 14px system-ui",
                  cursor: "pointer",
                }}
              >
                Keep building
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
