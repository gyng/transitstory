// "Follow this commuter": once you hit Follow on the Commuter card, this tracks one citizen's
// whole journey live — waiting → riding → transferring → arrived — with a pulsing locator on the
// map at where they are right now (their platform, or the train they're on). Pure chrome: reads
// the read-only follow query, re-renders on the ~3 Hz stats tick (NOT the render loop — the locator
// updates at stats cadence, like the waiting halos). Located via map.project (the sanctioned anchor
// read, same as DraftControls).
import { useEffect, useReducer, useRef } from "react";
import { mmToLngLat, metersToLngLat } from "../../coords/geo";
import { useGame, useStats } from "./GameContext";
import { hex } from "./shared";
import type { FollowView } from "../../types";

export function FollowCard() {
  const game = useGame();
  const stats = useStats(); // live refresh on the 3 Hz snapshot + the authoritative renege signal (abandoned)
  const [, bump] = useReducer((n: number) => n + 1, 0);
  const lastSeen = useRef<FollowView | null>(null); // last live state, to infer the trip's outcome
  const abandonedSeen = useRef(0); // #21 the global renege count at the last LIVE sighting of this rider
  const verdict = useRef<boolean | null>(null); // #21 the trip outcome, LATCHED once at live→gone (null = undetermined)
  useEffect(() => {
    game.onChange.push(bump);
    return () => {
      const i = game.onChange.indexOf(bump);
      if (i >= 0) game.onChange.splice(i, 1);
    };
  }, [game]);

  const id = game.followedCitizen;
  if (id === null) return null;
  const f = game.bridge.followCitizen(id);
  if (f) {
    lastSeen.current = f;
    abandonedSeen.current = stats.abandoned; // snapshot the global renege count while we can still see them
    verdict.current = null; // #21 live again — clear any latched outcome
  } else if (verdict.current === null && lastSeen.current) {
    // #21 LATCH the outcome ONCE at the live→gone transition — don't recompute it each 3 Hz render, or the
    // city-wide `abandoned` counter climbing (from ANY unrelated rider's renege) would retroactively flip a
    // settled "arrived" into "gave up". Computed from the authoritative signal at the instant they vanished.
    const prev = lastSeen.current;
    verdict.current = !prev.onboard && prev.waitMin >= 9 && stats.abandoned > abandonedSeen.current;
  }

  // Not in transit right now — infer the outcome from the last live state. A rider who vanished
  // from a long queue gave up (reneged); otherwise they arrived. The card stays so when their next
  // commute spawns, `followCitizen` finds them again and the live view resumes (the daily routine).
  if (!f) {
    const prev = lastSeen.current;
    const gaveUp = verdict.current === true; // #21 the outcome latched once at the live→gone transition (above), never recomputed
    return (
      <Card>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <b data-testid="follow-name">{gaveUp ? "😞" : "🏁"} {prev?.name ?? "Commuter"}</b>
          <Stop onClick={() => game.clearFollowed()} />
        </div>
        <div data-testid="follow-status" style={{ fontWeight: 600, marginTop: 3, color: gaveUp ? "var(--ot-con-red)" : "var(--ot-gauge-good,#009e73)" }}>
          {gaveUp ? `gave up waiting at ${prev?.at}` : `✅ arrived at ${prev?.dest ?? "their destination"}`}
        </div>
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, marginTop: 3, lineHeight: 1.3 }}>
          {gaveUp
            ? "Service was too infrequent — a shorter headway or more capacity would keep them."
            : "They'll set off again on their next commute."}
        </div>
      </Card>
    );
  }

  // Locate them on screen (their station, or the train they're riding).
  let screen: { x: number; y: number } | null = null;
  if (!f.onboard && f.station >= 0) {
    const sv = game.bridge.stationsView()[f.station];
    if (sv) screen = game.map.project(mmToLngLat([sv.xMm, sv.yMm]) as [number, number]);
  } else if (f.onboard && f.vehicle >= 0) {
    const pos = game.bridge.vehiclePositions();
    if (f.vehicle * 2 + 1 < pos.length) {
      screen = game.map.project(metersToLngLat([pos[f.vehicle * 2], pos[f.vehicle * 2 + 1]]) as [number, number]);
    }
  }

  const elapsed = f.journeyMin >= 1 ? `${Math.round(f.journeyMin)} min` : "<1 min";
  const status = f.onboard ? `🚆 riding ${f.at}` : `⏳ waiting at ${f.at}`;

  return (
    <>
      <Card>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
          <b data-testid="follow-name">👁 {f.name}</b>
          <Stop onClick={() => game.clearFollowed()} />
        </div>
        {(f.home || f.work) && (
          <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, margin: "2px 0 5px" }}>
            🏠 {f.home || "—"} <span style={{ color: "var(--ot-con-ink-dim)" }}>→</span> 🏢 {f.work || "—"}
          </div>
        )}
        <div data-testid="follow-status" style={{ fontWeight: 600, color: f.onboard ? "var(--ot-con-accent)" : "var(--ot-con-amber)" }}>
          {status}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 3, margin: "5px 0" }}>
          {f.legs.map((leg, i) => (
            <div key={i} style={{ display: "flex", alignItems: "center", gap: 5, opacity: i < f.leg ? 0.4 : 1, fontSize: 11.5 }}>
              <span style={{ width: 10, height: 10, borderRadius: 3, flex: "none", background: hex(leg.lineColor) }} />
              <span style={{ flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{leg.lineName}</span>
              <span style={{ color: "var(--ot-con-ink-dim)", whiteSpace: "nowrap" }}>{i === f.leg ? "● now" : `→ ${leg.alight}`}</span>
            </div>
          ))}
        </div>
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11 }}>
          → {f.dest} · trip so far {elapsed}
        </div>
      </Card>

      {/* Pulsing locator at their current position (DOM marker, stats-cadence — not per frame). */}
      {screen && (
        <div
          data-testid="follow-locator"
          style={{
            position: "fixed",
            left: screen.x,
            top: screen.y,
            transform: "translate(-50%,-50%)",
            zIndex: 8,
            width: 26,
            height: 26,
            borderRadius: "50%",
            border: `3px solid ${hex(f.lineColor)}`,
            boxShadow: `0 0 0 2px rgba(255,255,255,.9), 0 0 12px ${hex(f.lineColor)}`,
            pointerEvents: "none",
            animation: "ot-follow-pulse 1.1s ease-out infinite",
          }}
        />
      )}
      <style>{"@keyframes ot-follow-pulse{0%{transform:translate(-50%,-50%) scale(.7);opacity:1}100%{transform:translate(-50%,-50%) scale(1.5);opacity:.2}}"}</style>
    </>
  );
}

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div
      data-testid="follow-card"
      className="ot-console"
      style={{
        position: "fixed",
        top: 52,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 14,
        width: 280,
        padding: "10px 13px",
        font: "12px system-ui,sans-serif",
      }}
    >
      {children}
    </div>
  );
}

function Stop({ onClick }: { onClick: () => void }) {
  return (
    <button
      data-testid="follow-stop"
      className="ot-key"
      onClick={onClick}
      title="Stop following"
      style={{ cursor: "pointer", font: "700 12px system-ui", padding: "3px 8px" }}
    >
      ✕ stop
    </button>
  );
}
