// BOTTOM DOCK (Paradox-style outliner + event ticker). LEFT ⅔ = a tabbed roster panel
// (Roster / Fleet / Report), each migrated from its former floating panel; RIGHT ⅓ = a read-only
// event TICKER (objective/onboarding one-liner + a rolling event feed + the day-report digest).
//
// Migrated here (stage 6): Panels' LineList → Roster tab; Fleet → Fleet tab; ServiceReport → Report
// tab; Objectives + Onboarding one-liners + the DayReport digest → the ticker. Fantasy adds the
// SpellBar as a roster-adjacent group. Freeing bottom-CENTRE is what lets the build HUD / draft /
// station-confirm controls float at the cursor without colliding (the structural overflow fix).
//
// AGENTS: React owns DOM chrome only; reads via useStats/useGameUI, writes via Game methods; all DOM
// rides the ~3 Hz stats slice (no per-frame work). Every migrated testid is preserved (e2e contract).
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { useGameUI, useStats } from "./GameContext";
import { LineList } from "./Panels";
import { FleetBody } from "./Fleet";
import { ServiceReport } from "./ServiceReport";
import { SpellBar } from "./SpellBar";
import { ObjectivePanel } from "./Objectives";
import type { Scenario } from "../../objectives";

type Tab = "roster" | "fleet" | "report";

const DOCK_STYLE: CSSProperties = {
  height: "100%",
  display: "grid",
  gridTemplateColumns: "2fr 1fr",
  gap: 8,
  padding: "0 14px 8px",
  pointerEvents: "none", // children re-enable; the gaps pass map drags through
  // The dock is a fixed 92px row — clip so the roster/ticker SCROLL inside it instead of overflowing
  // down past the viewport (the bottom cell never grows; its content is bounded + scrollable).
  overflow: "hidden",
  boxSizing: "border-box",
};

const ROSTER_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  minWidth: 0,
  minHeight: 0, // let the inner roster body's overflow:auto take effect (flex child clamp)
  overflow: "hidden",
  padding: "5px 8px",
  pointerEvents: "auto",
};

const TICKER_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  minWidth: 0,
  minHeight: 0,
  padding: "6px 12px",
  pointerEvents: "auto",
  font: "12px system-ui,sans-serif",
  color: "var(--ot-con-ink)",
  overflow: "hidden",
};

function TabButton({ id, label, active, onClick }: { id: Tab; label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      data-testid={`outliner-tab-${id}`}
      id={`outliner-tab-${id}`}
      role="tab" // #13 announce as "tab N of 3, selected" rather than three unrelated buttons
      aria-selected={active}
      aria-controls="outliner-tabpanel"
      className={`ot-key ${active ? "on" : ""}`}
      onClick={onClick}
      style={{ padding: "3px 11px", font: "600 12px system-ui,sans-serif", cursor: "pointer" }}
    >
      {label}
    </button>
  );
}

/** A read-only rolling event feed, derived from the stats slice (day rollovers + cumulative ridership
 *  milestones). No parallel sim state — it diffs the ~3 Hz snapshot, exactly like the Beats toasts do. */
function useEventLog(): string[] {
  const s = useStats();
  const [log, setLog] = useState<string[]>([]);
  const prevDay = useRef<number | null>(null);
  const nextRider = useRef<number | null>(null);
  const RIDER_STEPS = useRef([100, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000]);

  useEffect(() => {
    const push = (msg: string) => setLog((l) => [msg, ...l].slice(0, 12));
    // Baseline silently on the first snapshot (and after an undo/load rewinds the day).
    if (prevDay.current === null || s.simDay < prevDay.current) {
      prevDay.current = s.simDay;
      let i = RIDER_STEPS.current.findIndex((m) => s.ridershipTotal < m);
      nextRider.current = i < 0 ? RIDER_STEPS.current.length : i;
      return;
    }
    if (s.simDay > prevDay.current) {
      push(`🌅 Day ${s.simDay} complete`); // #12 label by the new simDay, not prevDay+1 — a >1-day jump must read its true day
      prevDay.current = s.simDay;
    }
    while (nextRider.current! < RIDER_STEPS.current.length && s.ridershipTotal >= RIDER_STEPS.current[nextRider.current!]) {
      push(`🏅 ${RIDER_STEPS.current[nextRider.current!].toLocaleString()} ${s.ruleset === "arcadia" ? "supply delivered" : "riders carried"}`);
      nextRider.current! += 1;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.simDay, s.ridershipTotal]);

  return log;
}

/** The objective digest at the top of the ticker: the full ObjectivePanel (embedded — keeps every
 *  objective-* testid + its win/lose banner) when a scenario is active, else the cut-first onboarding
 *  one-liner until the player takes the loop's first action. */
function ObjectiveLine({ scenario }: { scenario: Scenario | null }) {
  const s = useStats();
  const arcadia = s.ruleset === "arcadia";
  if (scenario) return <ObjectivePanel scenario={scenario} embedded />;
  const done = arcadia ? s.lineCount > 0 : s.stationCount > 0;
  if (done) return null;
  return (
    <div data-testid="onboarding-line" style={{ color: "var(--ot-con-amber)", fontWeight: 600, marginBottom: 6, lineHeight: 1.3 }}>
      {arcadia ? "⚜ Rail a resource → a town to deliver supply, then field legions and hold the Decadence." : "🎯 Place 2 stations, run a Service between them, then press ▶ Run."}
    </div>
  );
}

/** The latest day-report digest line (the turn punctuation, condensed for the ticker). */
function DayDigest() {
  const s = useStats();
  const prevDay = useRef<number | null>(null);
  const startRiders = useRef(0);
  const [digest, setDigest] = useState<string | null>(null);
  useEffect(() => {
    if (prevDay.current === null || s.simDay < prevDay.current) {
      prevDay.current = s.simDay;
      startRiders.current = s.ridershipTotal;
      return;
    }
    if (s.simDay > prevDay.current) {
      const gained = Math.round(s.ridershipTotal - startRiders.current);
      // #12 label by the new simDay (the just-completed day), not prevDay+1, so a >1-day jump attributes its diff correctly.
      setDigest(`Day ${s.simDay}: +${gained} ${s.ruleset === "arcadia" ? "supply" : "riders"} · coverage ${Math.round(s.coverageScore)}`);
      prevDay.current = s.simDay;
      startRiders.current = s.ridershipTotal;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.simDay]);
  if (!digest) return null;
  return (
    <div data-testid="day-digest" style={{ color: "var(--ot-con-accent)", fontSize: 11, marginBottom: 6 }}>
      🌅 {digest}
    </div>
  );
}

export function Outliner({ scenario }: { scenario: Scenario | null }) {
  const ui = useGameUI();
  const arcadia = ui.ruleset === "arcadia";
  const [tab, setTab] = useState<Tab>("roster");
  const log = useEventLog();

  return (
    <div data-testid="outliner" style={DOCK_STYLE}>
      {/* LEFT ⅔ — the tabbed roster dock */}
      <div className="ot-console" style={ROSTER_STYLE}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 6, flex: "0 0 auto" }}>
          <div role="tablist" aria-label="Network panels" style={{ display: "flex", gap: 6 }}>
            <TabButton id="roster" label="Roster" active={tab === "roster"} onClick={() => setTab("roster")} />
            <TabButton id="fleet" label="Fleet" active={tab === "fleet"} onClick={() => setTab("fleet")} />
            <TabButton id="report" label="Report" active={tab === "report"} onClick={() => setTab("report")} />
          </div>
          {/* Fantasy: the spell bar lives roster-adjacent as an embedded popover (its spell-toggle key
              mounts only once SPELLCRAFT is owned). `embedded` is REQUIRED — without it the SpellBar
              falls back to its legacy fixed bottom-right bar and never renders the dock toggle. */}
          {arcadia && <div style={{ marginLeft: "auto" }}><SpellBar embedded /></div>}
        </div>
        <div role="tabpanel" id="outliner-tabpanel" aria-labelledby={`outliner-tab-${tab}`} style={{ flex: 1, minHeight: 0 }}>
          {tab === "roster" && <LineList embedded />}
          {tab === "fleet" && <FleetBody />}
          {tab === "report" && <ServiceReport embedded />}
        </div>
      </div>

      {/* RIGHT ⅓ — the read-only event ticker (everything scrolls together inside the 92px dock) */}
      <div className="ot-console" style={TICKER_STYLE} data-testid="ticker">
        <div style={{ flex: 1, overflowY: "auto", minHeight: 0, lineHeight: 1.5 }}>
          <ObjectiveLine scenario={scenario} />
          <DayDigest />
          <div style={{ height: 1, background: "rgba(255,255,255,.08)", margin: "2px 0 6px" }} />
          {log.length === 0 ? (
            <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11 }}>Events appear here as the world turns.</div>
          ) : (
            log.map((e, i) => (
              <div key={`${i}-${e}`} style={{ color: i === 0 ? "var(--ot-con-ink)" : "var(--ot-con-ink-dim)", fontSize: 11 }}>
                {e}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
