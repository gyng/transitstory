// Cut-first onboarding (PLAN/T17): a ghost station→station→line gesture + a one-line objective. Issues
// ZERO Commands and touches no sim state — pure #ui chrome that reads the stats slice. Mode-aware: the
// transit sandbox coaches "build your first line" (shown until the first station); the fantasy (arcadia)
// campaign — whose baked world ships ~40 pre-placed source/town nodes, so `stationCount` is never 0 —
// coaches the supply→legions→hold-the-rot loop instead, shown until the first LINE is drawn. Each mode
// remembers it was seen under its own key, so neither nags a returning player.
import { useEffect, useState } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { isDrawTool } from "../../game";

const SEEN_KEY = "transitstory.onboarded.v1";
const SEEN_KEY_ARCADIA = "transitstory.onboarded.arcadia.v1";
const TUTORIAL_DONE_KEY = "transitstory.tutorial.done.v1";

/** Has the guided first-line tutorial already been completed in this browser? Drives the Menu
 *  checkbox's default (off once you've done it) — so it offers itself to newcomers, never nags. */
export function tutorialDone(): boolean {
  try {
    return localStorage.getItem(TUTORIAL_DONE_KEY) === "1";
  } catch {
    return false;
  }
}

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

  // Transit: nudge toward the station tool. Fantasy: a DRAW tool — Track or Service (stations are pre-placed).
  const onStationTool = game.tool === "station";
  const onLineTool = isDrawTool(game.tool);

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
              ① {onLineTool ? "Rail" : "Pick Service (or Track), then rail"} a resource → a town to deliver supply ·
              ② Raise legions at your capital and conquer towns to grow Standing · ③ Hold ahead of the ☠ Decadence
              (you lose if it reaches your capital)
            </div>
          </div>
        ) : (
          <div style={{ lineHeight: 1.35 }}>
            <b style={{ fontSize: 14 }}>Build your first line</b>
            <div style={{ color: "#aeb6bf", marginTop: 2 }}>
              ① {onStationTool ? "Click the map" : "Pick the Station tool, then click"} to place 2 stations · ②
              Run a Service between them (or lay Track + assign trains) · ③ Press ▶ Run
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

// --- Guided first-line tutorial (opt-in via the Menu checkbox) ----------------------------------
// A STEPPED coach that walks a fantasy player through building their first line WITH A SIDING, then
// running it. Pure #ui chrome like OnboardingCoach: issues ZERO Commands, only READS the ui/stats
// slices + the placed-signal count, and advances as the player performs each act on the real controls.
// A "siding" on the fantasy board = a block signal dropped on a single-track span (the passing place
// where opposing carts meet — the Single/Double toggle is hidden under force_single_track), so the
// middle step points the player at exactly that gesture.
const TUT_CSS = `
  #tutorial .ot-step{display:flex;align-items:flex-start;gap:9px;padding:5px 0}
  #tutorial .ot-num{flex:0 0 auto;width:20px;height:20px;border-radius:50%;display:grid;place-items:center;
    font:700 11px system-ui;background:rgba(201,162,74,.18);color:#d9b561;border:1px solid rgba(201,162,74,.4)}
  #tutorial .ot-step.done .ot-num{background:rgba(70,208,140,.2);color:#5fd39a;border-color:rgba(70,208,140,.5)}
  #tutorial .ot-step.active .ot-num{background:rgba(201,162,74,.35);color:#fff;box-shadow:0 0 0 3px rgba(201,162,74,.18)}
  #tutorial .ot-step.todo{opacity:.5}
  #tutorial .ot-stitle{font:600 13px system-ui;color:#eef1f4}
  #tutorial .ot-shint{font:12px system-ui;color:#aeb6bf;margin-top:1px;line-height:1.35}
  @media (prefers-reduced-motion:reduce){#tutorial *{animation:none!important}}
`;

export function TutorialCoach() {
  const game = useGame();
  const stats = useStats();
  const ui = useGameUI();
  const [dismissed, setDismissed] = useState(false);

  // The siding = a placed block signal. placedSignals() is a flat Float64Array, 6 fields per signal.
  let signalCount = 0;
  try {
    signalCount = Math.floor(game.bridge.placedSignals().length / 6);
  } catch {
    signalCount = 0;
  }
  const onLineTool = isDrawTool(game.tool);

  const steps = [
    {
      id: "line",
      title: "Draw your first line",
      hint: onLineTool
        ? "Click a resource node, then a town, to rail between them."
        : "Pick the ⛏ Service (or ╱ Track) tool, then click a resource → a town.",
      done: stats.lineCount > 0,
    },
    {
      id: "siding",
      title: "Add a siding",
      hint: "Your track is single — opposing carts must MEET at a passing place. Click a single-track segment of your line to drop a block signal (the siding).",
      done: signalCount > 0,
    },
    {
      id: "run",
      title: "Run the line",
      hint: "Press ▶ Run (top-right) to set your trains — and your supply — in motion.",
      done: ui.mode === "run" || stats.running,
    },
  ];
  const allDone = steps.every((s) => s.done);
  const activeIdx = steps.findIndex((s) => !s.done); // first incomplete = the current step

  // Mark complete + auto-retire a few seconds after the last step lands (a beat to read "done").
  useEffect(() => {
    if (!allDone) return;
    try {
      localStorage.setItem(TUTORIAL_DONE_KEY, "1");
    } catch {
      /* ignore */
    }
    const t = window.setTimeout(() => setDismissed(true), 4500);
    return () => window.clearTimeout(t);
  }, [allDone]);

  if (dismissed) return null;

  return (
    <div
      id="tutorial"
      data-testid="tutorial"
      style={{
        position: "fixed",
        top: 58,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 12,
        width: "min(92vw, 460px)",
        padding: "11px 14px",
        borderRadius: 12,
        background: "rgba(20,27,33,.94)",
        color: "#eef1f4",
        boxShadow: "0 6px 22px rgba(0,0,0,.4)",
      }}
    >
      <style>{TUT_CSS}</style>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4 }}>
        <b style={{ fontSize: 13, letterSpacing: 0.2 }}>
          {allDone ? "✓ Your first line runs — with a siding." : "Tutorial · your first line with a siding"}
        </b>
        <button
          onClick={() => {
            try {
              localStorage.setItem(TUTORIAL_DONE_KEY, "1");
            } catch {
              /* ignore */
            }
            setDismissed(true);
          }}
          aria-label="Dismiss tutorial"
          data-testid="tutorial-dismiss"
          style={{ border: 0, background: "transparent", color: "#aeb6bf", cursor: "pointer", font: "15px system-ui", lineHeight: 1 }}
        >
          ✕
        </button>
      </div>
      {steps.map((s, i) => {
        const state = s.done ? "done" : i === activeIdx ? "active" : "todo";
        return (
          <div key={s.id} className={`ot-step ${state}`} data-testid={`tutorial-step-${s.id}`} data-done={s.done ? "1" : "0"}>
            <span className="ot-num">{s.done ? "✓" : i + 1}</span>
            <div>
              <div className="ot-stitle">{s.title}</div>
              {state === "active" && <div className="ot-shint">{s.hint}</div>}
            </div>
          </div>
        );
      })}
      <div style={{ color: "#8b939c", fontSize: 11, marginTop: 5 }}>Camera: WASD / arrows pan · Q E zoom · Space pause</div>
    </div>
  );
}
