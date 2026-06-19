// TOP-CENTRE alert tray (Paradox/Anno alert pings): clickable, severity-sorted status pings that
// each point at a real pressure on the map. Clicking one flies the IMPERATIVE map camera to the
// offending station (game.flyToAlert → the existing MapLibre seam, NOT a React rAF) and arms the
// tool that fixes it. Absorbs the floating `notice` toast (it's now a ping like any other).
//
// HARD RULE (the spec's, enforced here): EVERY alert derives ONLY from the sim's stats slice —
// `perStation[]` / `perLine[]` pressure fields, plus the top-level fantasy lose-meter fields and
// the ui.notice string. There is NO parallel JS heuristic, so the badge count, the icon, and the
// fly-to target always agree with what the map shows. CB-safe: icon + count + word, never hue alone.
import type { CSSProperties } from "react";
import type { PerStation, Stats } from "../../types";
import type { Tool } from "../../game";
import { useGame, useGameUI, useStats } from "./GameContext";

/** One derived alert. `station`/`tool` drive the fly-to + tool-arm; `sev` orders the tray (higher
 *  first). `kind` is the stable data-testid suffix; `color` is the CB-safe LED tint (paired w/ text). */
interface Alert {
  kind: string;
  icon: string;
  label: string;
  count: number;
  sev: number;
  color: string;
  station: number | null;
  tool?: Tool;
  /** A short why, surfaced on the ping title (recognition over recall). */
  title: string;
}

const RED = "var(--ot-gauge-bad)";
const AMBER = "var(--ot-con-amber)";
const ROT = "#8fc95f"; // toxic-green = decadence/raider (matches the gauge breach tint)

/** Pick the single worst station for a `perStation` pressure field, so the ping's fly-to lands on
 *  the place that most needs the fix (not just the first). Returns null if none qualifies. */
function worstStation(
  perStation: PerStation[],
  metric: (s: PerStation) => number,
  threshold: number,
): { id: number | null; count: number } {
  let count = 0;
  let bestId: number | null = null;
  let bestVal = threshold;
  for (const s of perStation) {
    const v = metric(s);
    if (v > threshold) {
      count += 1;
      if (v >= bestVal) {
        bestVal = v;
        bestId = s.stationId;
      }
    }
  }
  return { id: bestId, count };
}

/** Derive the active alerts from the stats slice alone (the hard rule). Transit + fantasy both read
 *  their pressure from `perStation[]`/`perLine[]` + the top-level fields; `notice` rides along. */
export function deriveAlerts(s: Stats, notice: string | null): Alert[] {
  const out: Alert[] = [];
  const arcadia = s.ruleset === "arcadia";

  // The transient afford-gate/rejection notice — a ping like any other (absorbs the floating toast).
  if (notice) {
    out.push({ kind: "notice", icon: "✖", label: notice, count: 1, sev: 80, color: RED, station: null, title: notice });
  }

  if (!arcadia) {
    // Left-behind: riders passed by a full train (per-station `denied` is the cumulative pressure).
    const left = worstStation(s.perStation, (st) => st.denied, 0);
    if (left.count > 0) {
      out.push({
        kind: "left-behind",
        icon: "⊘",
        label: "left behind",
        count: left.count,
        sev: 70,
        color: RED,
        station: left.id,
        tool: "service",
        title: "Riders passed by full trains — add capacity or shorten the headway. Click to fly there.",
      });
    }
    // Overbooked / starved stations: a big standing queue with no service is the coverage gap.
    const starved = worstStation(s.perStation, (st) => (st.serving === 0 ? st.waiting : 0), 4);
    if (starved.count > 0) {
      out.push({
        kind: "overbooked",
        icon: "◍",
        label: "unserved queue",
        count: starved.count,
        sev: 60,
        color: AMBER,
        station: starved.id,
        tool: "service",
        title: "Stations with a queue but no service — run a line through them. Click to fly there.",
      });
    }
    // Water: a surface line crossing water is PARKED until elevated/tunnelled (perLine.crossesWater).
    const water = s.perLine.filter((l) => l.crossesWater);
    if (water.length > 0) {
      // Anchor to the first stop of the first offending line (perLine has no station list; the line's
      // editor is where the fix lives, so we surface the line — fly-to uses its first stop via select).
      out.push({
        kind: "water-warning",
        icon: "≈",
        label: "over water",
        count: water.length,
        sev: 75,
        color: RED,
        station: null,
        title: "Surface track crosses water — the line is parked until you Elevate or Tunnel it (line panel).",
      });
    }
  } else {
    // FANTASY pressure — all from the top-level lose-meter + raider fields (the stats slice).
    if (s.realmLost) {
      out.push({ kind: "realm-lost", icon: "☠", label: "REALM FALLEN", count: 1, sev: 100, color: RED, station: null, title: "The Decadence overran your capital — the realm has fallen." });
    }
    const d = Math.round(s.decadencePct);
    if (d >= 66 && !s.realmLost) {
      out.push({ kind: "decadence-breach", icon: "☠", label: "decadence high", count: d, sev: 90, color: RED, station: null, title: `The Decadence is at ${d}% — conquer towns and Purge before it reaches the capital.` });
    }
    const raiders = Math.round(s.raiderCount);
    if (raiders > 0) {
      out.push({ kind: "raiders", icon: "☣", label: "raiders", count: raiders, sev: 65, color: ROT, station: null, title: `${raiders} decadence raider${raiders === 1 ? "" : "s"} loose — your station cordon cuts them down; cover the approaches.` });
    }
    // Towns under siege / unreachable frontier towns: a town with resistance left but not yet captured
    // is the conquest pressure (per-station `townResistance` + `captured`).
    const siege = worstStation(s.perStation, (st) => (!st.captured && st.townResistance > 0 ? st.townResistance : 0), 0);
    if (siege.count > 0) {
      out.push({
        kind: "decadence-eta",
        icon: "⚔",
        label: "contested towns",
        count: siege.count,
        sev: 50,
        color: AMBER,
        station: siege.id,
        tool: "line",
        title: "Towns still holding out — rail to them and field legions to take them. Click to fly there.",
      });
    }
  }

  return out.sort((a, b) => b.sev - a.sev);
}

const CLUSTER_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "5px 7px",
  pointerEvents: "auto",
  maxWidth: "min(620px, 46vw)",
  flexWrap: "wrap",
  justifyContent: "center",
};

/** The GLOBAL fantasy alerts have no anchor station, but are still click-useful (#18): they fly to the
 *  threat — decadence/realm to the capital, raiders to the nearest marauder. */
const GLOBAL_FLY = new Set(["decadence-breach", "realm-lost", "raiders"]);

function Ping({ a, onClick }: { a: Alert; onClick: () => void }) {
  // notice dismisses the toast; anchored pings fly to their station; global fantasy pings fly to the threat.
  const clickable = a.kind === "notice" || a.station !== null || GLOBAL_FLY.has(a.kind);
  // Preserve the `notice` testid string exactly (e2e contract: the floating toast moved here intact);
  // every other ping uses the stable `alert-<kind>` id.
  const testid = a.kind === "notice" ? "notice" : `alert-${a.kind}`;
  return (
    <button
      data-testid={testid}
      className="ot-key"
      onClick={clickable ? onClick : undefined}
      title={a.title}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 6,
        padding: "4px 9px",
        cursor: clickable ? "pointer" : "default",
        font: "600 12px system-ui,sans-serif",
        whiteSpace: "nowrap",
        maxWidth: 220,
      }}
    >
      <span className="ot-led" aria-hidden style={{ color: a.color, flex: "0 0 auto" }} />
      <span style={{ color: a.color, fontWeight: 700, flex: "0 0 auto" }}>{a.icon}</span>
      <b data-testid={`alert-${a.kind}-count`} style={{ fontVariantNumeric: "tabular-nums", color: "var(--ot-con-ink)", flex: "0 0 auto" }}>
        {a.count}
      </b>
      <span style={{ color: "var(--ot-con-ink-dim)", overflow: "hidden", textOverflow: "ellipsis" }}>{a.label}</span>
    </button>
  );
}

/** The top-centre alert tray. Empty (renders nothing) when the network is healthy — a quiet HUD is a
 *  healthy one (recognition: a ping appearing IS the signal). */
export function AlertCluster() {
  const s = useStats();
  const ui = useGameUI();
  const game = useGame();
  const alerts = deriveAlerts(s, ui.notice);
  // #9 the top alert as a screen-reader announcement. ALWAYS-MOUNTED + separate from the visual cluster (which
  // unmounts when healthy) so it persists across empty→populated and catches the FIRST ping. React reconciliation
  // debounces it — an unchanged string is no DOM mutation, so no re-announce on the ~3 Hz recompose. Assertive
  // only for the critical realm-lost / decadence breach (sev>=90), else polite.
  const top = alerts[0];
  const announce = top ? `Alert: ${top.label}${top.count > 1 ? `, ${top.count}` : ""}` : "";
  return (
    <>
      <div aria-live={top && top.sev >= 90 ? "assertive" : "polite"} aria-atomic="true" className="ot-sr-only">
        {announce}
      </div>
      {alerts.length > 0 && (
        <div data-testid="alert-cluster" className="ot-console" role="group" aria-label="Network alerts" style={CLUSTER_STYLE}>
          {alerts.map((a) => (
            <Ping
              key={a.kind}
              a={a}
              onClick={() => {
                // notice pings dismiss the toast; anchored pings fly to their station + arm the remedy tool;
                // global fantasy pings (#18) fly to the threat (raiders → nearest marauder, else the capital).
                if (a.kind === "notice") game.dismissNotice();
                else if (a.station !== null) game.flyToAlert(a.station, a.tool);
                else if (a.kind === "raiders") game.flyToThreat();
                else if (GLOBAL_FLY.has(a.kind)) game.flyToCapital();
              }}
            />
          ))}
        </div>
      )}
    </>
  );
}
