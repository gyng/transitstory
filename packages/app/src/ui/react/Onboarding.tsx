// Cut-first onboarding (PLAN/T17): a ghost station→station→line gesture + a one-line objective,
// shown only on the first boot of an EMPTY sandbox (stationCount === 0). It issues ZERO Commands
// and touches no sim state — pure #ui chrome that reads the stats slice. Auto-dismisses on the
// first real placement and remembers it was seen, so it never nags a returning player.
import { useEffect, useState } from "react";
import { useGame, useStats } from "./GameContext";

const SEEN_KEY = "transitstory.onboarded.v1";

const ONBOARD_CSS = `
  #onboarding .ot-ghost{position:relative;width:74px;height:30px;flex:0 0 auto}
  #onboarding .ot-dot{position:absolute;top:9px;width:12px;height:12px;border-radius:50%;
    background:#1ab6f0;box-shadow:0 0 0 3px rgba(26,182,240,.25)}
  #onboarding .ot-dot.a{left:2px;animation:ot-ob-a 3s ease-in-out infinite}
  #onboarding .ot-dot.b{right:2px;animation:ot-ob-b 3s ease-in-out infinite}
  #onboarding .ot-line{position:absolute;top:14px;left:14px;height:3px;border-radius:2px;
    background:#1ab6f0;width:0;animation:ot-ob-line 3s ease-in-out infinite}
  @keyframes ot-ob-a{0%,8%{transform:scale(0);opacity:0}16%,100%{transform:scale(1);opacity:1}}
  @keyframes ot-ob-b{0%,28%{transform:scale(0);opacity:0}38%,100%{transform:scale(1);opacity:1}}
  @keyframes ot-ob-line{0%,40%{width:0}60%,92%{width:46px}100%{width:46px;opacity:.25}}
  @media (prefers-reduced-motion:reduce){#onboarding *{animation:none!important}
    #onboarding .ot-dot,#onboarding .ot-line{opacity:1;transform:none;width:46px}}
`;

export function OnboardingCoach() {
  const game = useGame();
  const stats = useStats();
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem(SEEN_KEY) === "1";
    } catch {
      return false;
    }
  });

  const markSeen = () => {
    try {
      localStorage.setItem(SEEN_KEY, "1");
    } catch {
      /* private mode — fine, it just shows again next time */
    }
    setDismissed(true);
  };

  // The moment the player places their first station, the loop is discovered — step aside.
  useEffect(() => {
    if (stats.stationCount > 0 && !dismissed) markSeen();
  }, [stats.stationCount, dismissed]);

  if (dismissed || stats.stationCount > 0) return null;

  // Nudge the player toward the station tool if they're not already on it.
  const onStationTool = game.tool === "station";

  return (
    <div
      id="onboarding"
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
        maxWidth: "min(92vw, 560px)",
      }}
    >
      <style>{ONBOARD_CSS}</style>
      <div className="ot-ghost" aria-hidden>
        <span className="ot-dot a" />
        <span className="ot-line" />
        <span className="ot-dot b" />
      </div>
      <div style={{ lineHeight: 1.35 }}>
        <b style={{ fontSize: 14 }}>Build your first line</b>
        <div style={{ color: "#aeb6bf", marginTop: 2 }}>
          ① {onStationTool ? "Click the map" : "Pick the Station tool, then click"} to place 2 stations · ② Draw a
          line between them · ③ Press ▶ Run
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
