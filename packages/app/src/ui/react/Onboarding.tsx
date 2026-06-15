// Cut-first onboarding (PLAN/T17): a ghost station→station→line gesture + a one-line objective. Issues
// ZERO Commands and touches no sim state — pure #ui chrome that reads the stats slice. Mode-aware: the
// transit sandbox coaches "build your first line" (shown until the first station); the fantasy (arcadia)
// campaign — whose baked world ships ~40 pre-placed source/town nodes, so `stationCount` is never 0 —
// coaches the supply→legions→hold-the-rot loop instead, shown until the first LINE is drawn. Each mode
// remembers it was seen under its own key, so neither nags a returning player.
import { useEffect, useState } from "react";
import { useGame, useStats } from "./GameContext";

const SEEN_KEY = "transitstory.onboarded.v1";
const SEEN_KEY_ARCADIA = "transitstory.onboarded.arcadia.v1";

const ONBOARD_CSS = `
  #onboarding .ot-ghost{position:relative;width:74px;height:30px;flex:0 0 auto}
  #onboarding .ot-dot{position:absolute;top:9px;width:12px;height:12px;border-radius:50%;
    background:#1ab6f0;box-shadow:0 0 0 3px rgba(26,182,240,.25)}
  #onboarding .ot-dot.a{left:2px;animation:ot-ob-a 3s ease-in-out infinite}
  #onboarding .ot-dot.b{right:2px;animation:ot-ob-b 3s ease-in-out infinite}
  #onboarding .ot-line{position:absolute;top:14px;left:14px;height:3px;border-radius:2px;
    background:#1ab6f0;width:0;animation:ot-ob-line 3s ease-in-out infinite}
  #onboarding.arcadia .ot-dot{background:#c9a24a;box-shadow:0 0 0 3px rgba(201,162,74,.25)}
  #onboarding.arcadia .ot-line{background:#c9a24a}
  @keyframes ot-ob-a{0%,8%{transform:scale(0);opacity:0}16%,100%{transform:scale(1);opacity:1}}
  @keyframes ot-ob-b{0%,28%{transform:scale(0);opacity:0}38%,100%{transform:scale(1);opacity:1}}
  @keyframes ot-ob-line{0%,40%{width:0}60%,92%{width:46px}100%{width:46px;opacity:.25}}
  @media (prefers-reduced-motion:reduce){#onboarding *{animation:none!important}
    #onboarding .ot-dot,#onboarding .ot-line{opacity:1;transform:none;width:46px}}
`;

export function OnboardingCoach() {
  const game = useGame();
  const stats = useStats();
  const arcadia = stats.ruleset === "arcadia";
  const key = arcadia ? SEEN_KEY_ARCADIA : SEEN_KEY;
  // The loop is "discovered" once the player takes its first action: a station in transit, a LINE in the
  // pre-populated fantasy campaign (its stations are baked in, so the first real act is railing them).
  const done = arcadia ? stats.lineCount > 0 : stats.stationCount > 0;

  // Per-key in-session dismissals (the ✕). localStorage is the cross-session memory, re-read each render
  // for the CURRENT key so a ruleset switch after boot resolves to the right "seen" flag.
  const [dismissedKeys, setDismissedKeys] = useState<Set<string>>(() => new Set());
  const seen = (() => {
    try {
      return localStorage.getItem(key) === "1";
    } catch {
      return false;
    }
  })();

  const markSeen = () => {
    try {
      localStorage.setItem(key, "1");
    } catch {
      /* private mode — fine, it just shows again next time */
    }
    setDismissedKeys((s) => new Set(s).add(key));
  };

  // The moment the player takes the first action, step aside (and remember it).
  useEffect(() => {
    if (done && !seen && !dismissedKeys.has(key)) markSeen();
  }, [done, seen, key, dismissedKeys]);

  if (done || seen || dismissedKeys.has(key)) return null;

  // Transit: nudge toward the station tool. Fantasy: the line tool (stations are pre-placed).
  const onStationTool = game.tool === "station";
  const onLineTool = game.tool === "line";

  return (
    <div
      id="onboarding"
      className={arcadia ? "arcadia" : undefined}
      data-testid="onboarding"
      style={{
        position: "fixed",
        top: 58,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 12,
        display: "flex",
        alignItems: "center",
        gap: 14,
        padding: "10px 14px",
        borderRadius: 12,
        background: "rgba(20,27,33,.92)",
        color: "#eef1f4",
        font: "13px system-ui,sans-serif",
        boxShadow: "0 6px 22px rgba(0,0,0,.35)",
        maxWidth: "min(92vw, 580px)",
      }}
    >
      <style>{ONBOARD_CSS}</style>
      <div className="ot-ghost" aria-hidden>
        <span className="ot-dot a" />
        <span className="ot-line" />
        <span className="ot-dot b" />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        {arcadia ? (
          <div style={{ lineHeight: 1.35 }}>
            <b style={{ fontSize: 14 }}>Forge your dominion ⚜</b>
            <div style={{ color: "#aeb6bf", marginTop: 2 }}>
              ① {onLineTool ? "Draw rail" : "Pick Draw line, then rail"} a resource → a town to deliver supply
              (→ tribute) · ② Raise legions at your capital + post a Bounty to aim them · ③ Hold the ☠ Decadence
              back before it reaches your capital
            </div>
          </div>
        ) : (
          <div style={{ lineHeight: 1.35 }}>
            <b style={{ fontSize: 14 }}>Build your first line</b>
            <div style={{ color: "#aeb6bf", marginTop: 2 }}>
              ① {onStationTool ? "Click the map" : "Pick the Station tool, then click"} to place 2 stations · ②
              Draw a line between them · ③ Press ▶ Run
            </div>
          </div>
        )}
        {/* Camera + global keys (one line, cut-first). Tool keys are surfaced in each tool's popover hint. */}
        <div style={{ color: "#8b939c", fontSize: 11 }}>
          Camera: WASD / arrows pan · Q E zoom · Space pause · , . speed
        </div>
      </div>
      <button
        onClick={markSeen}
        aria-label="Dismiss onboarding"
        data-testid="onboarding-dismiss"
        style={{
          marginLeft: 2,
          border: 0,
          background: "transparent",
          color: "#aeb6bf",
          cursor: "pointer",
          font: "16px system-ui",
          lineHeight: 1,
        }}
      >
        ✕
      </button>
    </div>
  );
}
