// The starting CUTSCENE — a super-short isekai prologue played ONCE before the fantasy campaign
// boots ("you were summoned across to Arcadia to forge its rails"). Pure #ui chrome: issues ZERO
// Commands, touches no sim state (it runs BEFORE the GameProvider exists). Skippable always; honours
// prefers-reduced-motion (shows the whole premise at once, no auto-advance); a localStorage seen-flag
// so returning players go straight to the board. Motion is transform/opacity only (compositor-safe);
// it never touches deck.gl or the map rAF. Mounts in App's phase machine between Menu and boot().
import { useEffect, useRef, useState } from "react";
import { audio } from "../../fx/audio";

const SEEN_KEY = "transitstory.cutscene.v1";

/** Has the prologue already played in this browser? (so it greets once, never nags). */
export function cutsceneSeen(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === "1";
  } catch {
    return false; // storage blocked → treat as unseen (it's skippable anyway)
  }
}
function markSeen(): void {
  try {
    localStorage.setItem(SEEN_KEY, "1");
  } catch {
    /* ignore */
  }
}

// The premise, one card per beat. Kept terse — "super short" is the brief.
const BEATS: { line: string; sub?: string }[] = [
  { line: "In your world, you were no one.", sub: "A commuter. A face on a late train." },
  { line: "Then the rails called you across.", sub: "" },
  { line: "Wake, Director.", sub: "Arcadia's lines are yours to forge — before the rot takes the realm." },
];

const BEAT_MS = 2300; // per card; ~7 s total — a prologue, not a movie

export function Cutscene({ onDone }: { onDone: () => void }) {
  const reduce = typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
  const [i, setI] = useState(0);
  const [leaving, setLeaving] = useState(false);
  const done = useRef(false);

  const finish = () => {
    if (done.current) return;
    done.current = true;
    markSeen();
    setLeaving(true);
    // let the fade-out play, then hand off to boot (instant under reduced motion)
    window.setTimeout(onDone, reduce ? 0 : 420);
  };

  // Auto-advance the beats (a PURE updater — just step + cap). Reduced motion shows everything at once.
  useEffect(() => {
    if (reduce) return;
    audio.unlock(); // the menu click already unlocked WebAudio; this is belt-and-suspenders
    const id = window.setInterval(() => {
      setI((prev: number) => (prev >= BEATS.length - 1 ? prev : prev + 1));
    }, BEAT_MS);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reduce]);

  // Once the LAST beat is showing, hold it a beat, then hand off to boot. A separate effect (not a side
  // effect inside the updater) so the hand-off timer ties to the effect lifecycle — StrictMode-safe.
  useEffect(() => {
    if (reduce || i < BEATS.length - 1) return;
    const t = window.setTimeout(finish, BEAT_MS);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i, reduce]);

  // Enter / Space / click-through also advances past the prologue immediately.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " " || e.key === "Escape") {
        e.preventDefault();
        finish();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div
      data-testid="cutscene"
      onClick={finish}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 50,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        // a deep portal void — a cold rift opening, warm at the seam (the summons)
        background:
          "radial-gradient(120% 90% at 50% 42%, #1a2740 0%, #0c1019 48%, #05070b 100%)",
        color: "#e7ecf5",
        cursor: "pointer",
        opacity: leaving ? 0 : 1,
        transition: reduce ? "none" : "opacity 400ms ease",
        userSelect: "none",
        overflow: "hidden",
      }}
    >
      <style>{`
        @keyframes ot-cut-rise { from { opacity: 0; transform: translateY(14px); } to { opacity: 1; transform: none; } }
        @keyframes ot-cut-portal { 0%,100% { opacity: .55; transform: scale(1); } 50% { opacity: .8; transform: scale(1.04); } }
        @media (prefers-reduced-motion: reduce) { .ot-cut-rise, .ot-cut-portal { animation: none !important; } }
      `}</style>

      {/* the rift glow — a slow breathing portal behind the words */}
      {!reduce && (
        <div
          aria-hidden
          className="ot-cut-portal"
          style={{
            position: "absolute",
            width: "46vmin",
            height: "46vmin",
            borderRadius: "50%",
            background: "radial-gradient(circle, rgba(120,170,255,0.42) 0%, rgba(90,130,220,0.12) 45%, transparent 70%)",
            filter: "blur(6px)",
            animation: "ot-cut-portal 4.5s ease-in-out infinite",
          }}
        />
      )}

      {/* the beats */}
      <div style={{ position: "relative", textAlign: "center", maxWidth: "min(80vw, 720px)", padding: "0 24px" }}>
        {reduce ? (
          // reduced motion: the whole premise at once, no animation
          BEATS.map((b, k) => (
            <div key={k} style={{ margin: "0 0 18px" }}>
              <div style={{ fontSize: 26, fontWeight: 700, letterSpacing: 0.2 }}>{b.line}</div>
              {b.sub && <div style={{ fontSize: 15, opacity: 0.72, marginTop: 6 }}>{b.sub}</div>}
            </div>
          ))
        ) : (
          <div key={i} className="ot-cut-rise" style={{ animation: "ot-cut-rise 700ms ease both" }}>
            <div style={{ fontSize: "clamp(22px, 3.4vw, 34px)", fontWeight: 700, letterSpacing: 0.2, textShadow: "0 2px 18px rgba(0,0,0,0.6)" }}>
              {BEATS[i].line}
            </div>
            {BEATS[i].sub && (
              <div style={{ fontSize: "clamp(13px, 1.6vw, 16px)", opacity: 0.75, marginTop: 10, textShadow: "0 1px 10px rgba(0,0,0,0.6)" }}>
                {BEATS[i].sub}
              </div>
            )}
          </div>
        )}
      </div>

      {/* progress dots + skip */}
      {!reduce && (
        <div aria-hidden style={{ position: "absolute", bottom: 88, display: "flex", gap: 8 }}>
          {BEATS.map((_, k) => (
            <span
              key={k}
              style={{
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: k <= i ? "rgba(150,185,255,0.9)" : "rgba(255,255,255,0.18)",
                transition: "background 300ms ease",
              }}
            />
          ))}
        </div>
      )}
      <button
        data-testid="cutscene-skip"
        onClick={(e) => {
          e.stopPropagation();
          finish();
        }}
        style={{
          position: "absolute",
          bottom: 36,
          padding: "9px 20px",
          background: "rgba(255,255,255,0.08)",
          border: "1px solid rgba(255,255,255,0.18)",
          borderRadius: 999,
          color: "#dfe6f2",
          font: "600 13px system-ui, sans-serif",
          letterSpacing: 0.3,
          cursor: "pointer",
        }}
      >
        {reduce ? "Enter Arcadia ▶" : "Skip ▶"}
      </button>
    </div>
  );
}
