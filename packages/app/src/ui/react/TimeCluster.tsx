// TOP-RIGHT time cluster (GSG top strip, R cell): the Build/Run hard wall + the speed ladder + a
// clock readout. Lifted out of the old bottom #transport-bar so time/flow controls own one corner
// instead of sharing the overflowing bar (the structural overflow fix).
//
//   • Build/Run — a segmented .ot-key (cyan glow = Run, the hard wall). Reads ui.mode, writes
//     game.setMode (Build↔Run is a mode flip, committed via the existing Game seam).
//   • Speed ladder — LOCAL state → loop.setSpeed. Speed is a GameLoop knob, NEVER a Command
//     (AGENTS: "speed is a GameLoop knob, not a Command"). `,`/`.` step the ladder.
//   • Clock — reads the ~3 Hz stats slice (simHour). The clock/period/tod-glyph TESTIDS live HERE
//     now (stage 5 split them off StatsBar, which became the top-left resource strip).
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { useGame, useGameUI, useLoop, useStats } from "./GameContext";
import { Button } from "./keys";

// A doubling gear ladder (1×→8×) for fine control + a max fast-forward. The sim speed is a GameLoop
// knob (loop.setSpeed), never a Command — speed never touches sim state.
const SPEEDS: [number, string][] = [
  [1, "1×"],
  [2, "2×"],
  [4, "4×"],
  [8, "8×"],
  [100, "max"],
];

const CLUSTER_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: 7,
  pointerEvents: "auto",
};

/** Sun/moon glyph for the hour — makes the time-of-day (+ the day/night map wash) legible in TEXT,
 *  not hue alone (recognition-over-recall, colour-blind-safe). Bands match the sky.ts palette. */
function todGlyph(hour: number): string {
  const h = ((hour % 24) + 24) % 24;
  if (h < 6 || h >= 20) return "🌙";
  if (h < 8) return "🌅";
  if (h >= 18) return "🌇";
  return "☀️";
}

export function TimeCluster() {
  const game = useGame();
  const loop = useLoop();
  const ui = useGameUI();
  const stats = useStats();

  const [speed, setSpeed] = useState(1);
  // The keydown handler reads the live speed via a ref so the single window listener stays stable
  // across speed changes (no add/remove churn).
  const speedRef = useRef(speed);
  speedRef.current = speed;

  // `,` / `.` step the speed ladder. (Space/1-5/tool keys still live in Toolbar until stage 4.)
  // Ignored while typing in a field and for ctrl/meta/alt chords.
  useEffect(() => {
    const setSpd = (mult: number) => {
      setSpeed(mult);
      loop.setSpeed(mult);
    };
    const stepSpeed = (dir: number) => {
      const i = SPEEDS.findIndex(([v]) => v === speedRef.current);
      const ni = Math.max(0, Math.min(SPEEDS.length - 1, (i < 0 ? 0 : i) + dir));
      setSpd(SPEEDS[ni][0]);
    };
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key === ",") { stepSpeed(-1); return; }
      if (e.key === ".") { stepSpeed(1); return; }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [loop]);

  const running = ui.mode === "run";

  // Clock readout — owns the clock/period/tod-glyph testids (moved off StatsBar in the stage-5 split).
  // The fantasy ("arcadia") world has no day/night sun glyph and labels its period "Arcadia".
  const arcadia = stats.ruleset === "arcadia";
  const hh = Math.floor(stats.simHour);
  const mm = Math.floor((stats.simHour - hh) * 60);
  const clock = `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;

  return (
    <div
      id="mode-controls"
      data-testid="mode-controls"
      className="ot-console"
      style={CLUSTER_STYLE}
    >
      <span style={{ display: "flex", alignItems: "center", gap: 4, padding: "0 2px" }} title="In-game clock">
        {!arcadia && (
          <span data-testid="tod-glyph" title={stats.period}>
            {todGlyph(stats.simHour)}
          </span>
        )}
        <b data-testid="clock" style={{ font: `600 13px var(--ot-readout-font)`, color: "var(--ot-con-ink)", fontVariantNumeric: "tabular-nums" }}>
          {clock}
        </b>
        <span data-testid="period" style={{ fontSize: 11, color: "var(--ot-con-ink-dim)" }}>
          {arcadia ? "Arcadia" : stats.period}
        </span>
      </span>
      <span style={SEP_STYLE} />
      <Button
        label={running ? "⏸ Build" : "▶ Run"}
        testid="mode-toggle"
        onClick={() => game.setMode(running ? "build" : "run")}
        on={running}
        tone="good"
      />
      <span style={SEP_STYLE} />
      {SPEEDS.map(([mult, label]) => (
        <Button
          key={mult}
          label={label}
          testid={`speed-${mult}`}
          onClick={() => {
            setSpeed(mult);
            loop.setSpeed(mult);
          }}
          on={speed === mult}
        />
      ))}
    </div>
  );
}

// A cut seam between key groups on the console face (a dark groove with a faint top-light).
const SEP_STYLE: CSSProperties = {
  width: 2,
  alignSelf: "stretch",
  background: "linear-gradient(180deg, rgba(0,0,0,.45), rgba(255,255,255,.05))",
  borderRadius: 1,
  margin: "2px 5px",
};
