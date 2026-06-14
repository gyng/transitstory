// Title screen + start menu. Center stage is a randomly-picked, gloriously over-the-top
// light-novel "subtitle" (/title/subN.webp). The clickable station-master pigeon perches in
// the bottom-left corner; the real "Transit Story" wordmark sits in the bottom-right. Then
// pick a city + start mode and boot.
//
// All motion is transform/opacity only (compositor-friendly) and is disabled under
// prefers-reduced-motion. Nothing here overflows the viewport. The mascot hop is a
// self-contained DOM micro-interaction driven through a ref — removing `jump` on animationend
// lets the idle bob resume (a sticky class would leave the pigeon static). The random subtitle
// is chosen ONCE (frontend chrome RNG, not the sim).
import { useRef, useState } from "react";
import { CITIES, personalBest, type CityEntry } from "../../sim/cities";
import { withBase } from "../../config";
import { readSave, type SaveBlob } from "../../sim/save";
import { SCENARIOS } from "../../objectives";

const SUBTITLE_COUNT = 53;
const MASCOT_LINES = [
  "Coo!",
  "Coo coo!",
  "All aboard!",
  "Mind the gap!",
  "Next stop: anywhere!",
  "Tweet~",
  "Right on time!",
  "Tickets, please!",
  "Doors closing!",
  "Flap flap!",
  "Now departing!",
  "Peck peck!",
  "Mind the pigeon!",
  "Brrrt!",
  "Service resumes shortly!",
  "Bread at platform 3!",
];

// Scoped styles (keyframes + hover/pseudo states can't be inline). `#menu` lives inside #ui
// (pointer-events:none, children auto), so these re-mute the decorative layers and re-enable
// only the controls + the pigeon.
const MENU_CSS = `
    /* ---- living backdrop: a faint drifting station-grid + a slow color glow ---- */
    #menu .ot-bg{position:absolute;inset:0;z-index:0;pointer-events:none;overflow:hidden}
    #menu .ot-bg::before{content:"";position:absolute;inset:-25%;
      background:radial-gradient(circle,rgba(123,176,214,.07) 1.4px,transparent 1.6px) 0 0/36px 36px;
      animation:ot-pan 26s linear infinite}
    #menu .ot-bg::after{content:"";position:absolute;left:50%;top:24%;width:150vmax;height:150vmax;
      transform:translate(-50%,-50%);opacity:.6;
      background:conic-gradient(from 0deg,rgba(26,182,240,.10),rgba(155,89,222,.08),
        rgba(255,176,32,.07),rgba(0,158,115,.08),rgba(26,182,240,.10));
      filter:blur(60px);animation:ot-spin 72s linear infinite}

    /* ---- center stage: the absurd light-novel subtitle ---- */
    /* Fixed stage height reserves the subtitle's vertical space, so cycling to a new image
       (which remounts + loads async) never reflows the menu below it — no layout shift. */
    #menu .ot-stage{position:relative;z-index:1;display:flex;align-items:center;justify-content:center;
      height:42vh;margin-bottom:18px}
    #menu .ot-stage::before{content:"";position:absolute;width:120%;height:120%;border-radius:50%;
      background:radial-gradient(closest-side,rgba(120,180,230,.20),transparent 72%);
      filter:blur(14px);animation:ot-breathe 7s ease-in-out infinite}
    #menu .ot-hero{position:relative;max-width:min(620px,86vw);max-height:100%;width:auto;height:auto;
      display:block;filter:drop-shadow(0 14px 30px rgba(0,0,0,.6));
      animation:ot-pop .7s cubic-bezier(.2,1.3,.4,1) both,ot-float 7s ease-in-out .7s infinite}

    /* ---- controls ---- */
    #menu .ot-card{position:relative;z-index:1;display:flex;flex-direction:column;align-items:center;
      width:min(560px,92vw);pointer-events:none}
    #menu .ot-grid{display:flex;gap:9px;justify-content:center;flex-wrap:wrap;margin-bottom:14px;width:100%}
    #menu .ot-city{pointer-events:auto;flex:1 1 150px;min-width:140px;padding:12px 13px;border-radius:11px;
      border:2px solid transparent;background:#1c232b;color:#eef1f4;cursor:pointer;text-align:left;
      animation:ot-fadeup .5s both;
      transition:transform .14s ease,border-color .14s ease,background .14s ease,box-shadow .14s ease}
    #menu .ot-city:hover{transform:translateY(-3px);background:#222b34;box-shadow:0 10px 22px rgba(0,0,0,.35)}
    #menu .ot-city:active{transform:translateY(-1px) scale(.99)}
    #menu .ot-city.sel{border-color:#1ab6f0;background:#11405a;transform:translateY(-2px);
      box-shadow:0 0 0 1px rgba(26,182,240,.45),0 10px 26px rgba(10,143,204,.4)}
    #menu .ot-mode{display:flex;gap:10px;justify-content:center;margin-bottom:16px;flex-wrap:wrap;
      animation:ot-fadeup .5s .5s both}
    #menu .ot-modebtn{pointer-events:auto;padding:9px 14px;border-radius:9px;border:1px solid #39414a;
      background:#1c232b;color:#eef1f4;cursor:pointer;transition:border-color .14s ease,background .14s ease,transform .14s ease}
    #menu .ot-modebtn:hover{transform:translateY(-2px)}
    #menu .ot-modebtn.sel{background:#11405a;border-color:#1ab6f0}
    #menu .ot-start{pointer-events:auto;padding:13px 34px;border:0;border-radius:11px;
      background:linear-gradient(180deg,#1ab6f0,#0a8fcc);color:#fff;font:700 17px system-ui;cursor:pointer;
      animation:ot-fadeup .5s .6s both,ot-pulse 2.6s 1.2s ease-in-out infinite;
      transition:transform .14s ease}
    #menu .ot-start:hover{transform:translateY(-2px) scale(1.03)}
    #menu .ot-start:active{transform:translateY(0) scale(.98)}

    /* ---- PRIMARY mode: the fantasy campaign (the fork's headline) ---- */
    #menu .ot-primary{pointer-events:auto;width:100%;margin-bottom:16px;padding:18px 20px;border-radius:14px;
      border:2px solid rgba(155,89,222,.55);text-align:left;position:relative;overflow:hidden;color:#f3eefb;
      background:linear-gradient(135deg,rgba(58,30,86,.94),rgba(26,28,58,.94));
      box-shadow:0 12px 30px rgba(80,40,140,.32);animation:ot-fadeup .5s .2s both;cursor:not-allowed}
    #menu .ot-primary[disabled]{opacity:.92}
    #menu .ot-primary .ot-badge{position:absolute;top:12px;right:14px;font:700 10px system-ui;letter-spacing:.06em;
      text-transform:uppercase;color:#e3d4ff;background:rgba(155,89,222,.28);padding:3px 9px;border-radius:999px;
      border:1px solid rgba(155,89,222,.55)}
    #menu .ot-primary h2{margin:0 0 5px;font:800 22px system-ui}
    #menu .ot-primary p{margin:0;color:#c6bfdb;font-size:13px;line-height:1.45;max-width:48ch}

    /* ---- SECONDARY section header: real cities, demoted below the campaign ---- */
    #menu .ot-secthead{width:100%;display:flex;align-items:center;gap:10px;margin:0 0 10px;
      color:#8a93a3;font:700 11px system-ui;letter-spacing:.06em;text-transform:uppercase;
      animation:ot-fadeup .5s .3s both}
    #menu .ot-secthead::after{content:"";flex:1;height:1px;background:linear-gradient(90deg,#39414a,transparent)}
    #menu .ot-grid .ot-city{opacity:.9}

    /* ---- bottom-left: clickable pigeon ---- */
    #menu .ot-mascot-wrap{position:fixed;left:clamp(8px,2vw,26px);bottom:clamp(6px,1.6vw,18px);
      width:clamp(96px,12vw,150px);z-index:2;pointer-events:none;animation:ot-pop .55s .75s cubic-bezier(.2,1.4,.4,1) both}
    #menu .ot-mascot{position:relative;display:block;width:100%;background:none;border:0;padding:0;
      cursor:pointer;pointer-events:auto;transform-origin:50% 100%;
      animation:ot-bob 3.2s ease-in-out infinite;-webkit-tap-highlight-color:transparent}
    #menu .ot-mascot img{width:100%;display:block;filter:drop-shadow(0 8px 13px rgba(0,0,0,.5))}
    #menu .ot-mascot.jump{animation:ot-jump .6s cubic-bezier(.3,1.6,.5,1)}
    #menu .ot-bubble{position:absolute;left:50%;top:-8px;transform:translate(-50%,-100%) scale(.6);
      background:#fff;color:#1c2024;font:600 12px system-ui;padding:5px 10px;border-radius:12px;
      white-space:nowrap;opacity:0;pointer-events:none;box-shadow:0 4px 12px rgba(0,0,0,.3)}
    #menu .ot-bubble::after{content:"";position:absolute;left:50%;bottom:-6px;transform:translateX(-50%);
      border:6px solid transparent;border-top-color:#fff;border-bottom:0}
    #menu .ot-bubble.show{animation:ot-bubble 1.5s ease forwards}

    /* ---- bottom-right: the real wordmark ---- */
    #menu .ot-brand{position:fixed;right:clamp(10px,2.4vw,30px);bottom:clamp(8px,1.8vw,20px);
      width:clamp(150px,17vw,232px);z-index:2;pointer-events:none;
      animation:ot-pop .55s .85s cubic-bezier(.2,1.4,.4,1) both}
    #menu .ot-logo{width:100%;display:block;filter:drop-shadow(0 7px 15px rgba(0,0,0,.5));
      animation:ot-sway 5s ease-in-out 1.4s infinite}

    @keyframes ot-fadeup{from{opacity:0;transform:translateY(14px)}to{opacity:1;transform:none}}
    @keyframes ot-pop{from{opacity:0;transform:translateY(12px) scale(.9)}to{opacity:1;transform:none}}
    @keyframes ot-float{0%,100%{transform:translateY(0) rotate(-.3deg)}50%{transform:translateY(-9px) rotate(.3deg)}}
    @keyframes ot-sway{0%,100%{transform:translateY(0)}50%{transform:translateY(-4px)}}
    @keyframes ot-bob{0%,100%{transform:translateY(0)}50%{transform:translateY(-6px)}}
    @keyframes ot-jump{0%{transform:translateY(0) scaleY(1)}18%{transform:translateY(2px) scaleY(.86)}
      45%{transform:translateY(-40px) scaleY(1.08)}70%{transform:translateY(0) scaleY(.94)}100%{transform:translateY(0) scaleY(1)}}
    @keyframes ot-breathe{0%,100%{transform:scale(.95);opacity:.55}50%{transform:scale(1.05);opacity:.85}}
    @keyframes ot-pulse{0%,100%{box-shadow:0 6px 18px rgba(10,143,204,.45)}50%{box-shadow:0 8px 30px rgba(26,182,240,.7)}}
    @keyframes ot-pan{to{transform:translate(36px,36px)}}
    @keyframes ot-spin{to{transform:translate(-50%,-50%) rotate(360deg)}}
    @keyframes ot-bubble{0%{opacity:0;transform:translate(-50%,-100%) scale(.6)}
      16%{opacity:1;transform:translate(-50%,-100%) scale(1)}
      78%{opacity:1;transform:translate(-50%,-100%) scale(1)}100%{opacity:0;transform:translate(-50%,-100%) scale(1)}}
    @media (prefers-reduced-motion:reduce){
      #menu *{animation:none!important}}
  `;

export function Menu({
  onStart,
  onResume,
}: {
  onStart: (city: CityEntry, withNetwork: boolean, scenario: string | null) => void;
  onResume?: (save: SaveBlob) => void;
}) {
  // Hero subtitle: start on a random one (frontend chrome RNG, not the sim); click to cycle.
  const [subIdx, setSubIdx] = useState(() => 1 + Math.floor(Math.random() * SUBTITLE_COUNT));
  const heroSrc = withBase(`/title/sub${subIdx}.webp`);
  const cycleSub = () => setSubIdx((i) => (i % SUBTITLE_COUNT) + 1);
  const [selected, setSelected] = useState<CityEntry>(CITIES[0]);
  const [withNetwork, setWithNetwork] = useState(true);
  const [scenario, setScenario] = useState<string | null>(null);
  // A resumable autosave, read once at mount (frontend-only; not the sim).
  const [save] = useState<SaveBlob | null>(() => readSave());
  const mascotRef = useRef<HTMLButtonElement>(null);
  const bubbleRef = useRef<HTMLDivElement>(null);

  // Restart the hop + bubble by re-adding their classes (CSS-restart idiom). Removing `jump`
  // on animationend lets the idle bob resume — a sticky class would leave the pigeon static.
  const pokeMascot = () => {
    const m = mascotRef.current;
    const b = bubbleRef.current;
    if (!m || !b) return;
    m.classList.remove("jump");
    void m.offsetWidth; // restart the animation
    m.classList.add("jump");
    b.textContent = MASCOT_LINES[Math.floor(Math.random() * MASCOT_LINES.length)];
    b.classList.remove("show");
    void b.offsetWidth;
    b.classList.add("show");
  };

  return (
    <div
      id="menu"
      data-testid="menu"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 50,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "6px",
        padding: "18px",
        overflow: "hidden",
        color: "#eef1f4",
        fontFamily: "system-ui,-apple-system,Segoe UI,Roboto,sans-serif",
        background: "radial-gradient(120% 90% at 50% 22%,#28313a 0%,#161b21 55%,#0c0f13 100%)",
      }}
    >
      <style>{MENU_CSS}</style>

      <div className="ot-bg" />

      {/* Center stage: a random over-the-top light-novel subtitle — click it to cycle to the
          next. `key={subIdx}` remounts the img so the entrance pop replays on each cycle. */}
      <div className="ot-stage">
        <img
          key={subIdx}
          className="ot-hero"
          data-testid="subtitle"
          alt="Transit Story"
          decoding="async"
          src={heroSrc}
          role="button"
          aria-label="Show another subtitle"
          title="Click for another"
          onClick={cycleSub}
          style={{ cursor: "pointer" }}
        />
      </div>

      <div className="ot-card">
        {/* PRIMARY mode: the fantasy campaign — the fork's headline. Not yet playable (no fantasy
            ruleset/world baked), so it renders as a prominent, disabled "in development" hero card.
            When the fantasy world is baked this becomes a real selectable mode that calls onStart. */}
        <button
          className="ot-primary"
          data-testid="mode-fantasy"
          disabled
          aria-disabled="true"
          title="The fantasy campaign is in development"
        >
          <span className="ot-badge">In development</span>
          <h2>⚔️ Arcadia — Fantasy Campaign</h2>
          <p>
            Supply your towns, forge armies, and conquer the realm from a lone capital. The new
            primary mode — coming soon.
          </p>
        </button>

        {/* SECONDARY mode: the classic real-map transit builder, demoted below the campaign. */}
        <div className="ot-secthead">Real cities · classic transit</div>

        <div className="ot-grid">
          {CITIES.map((c, i) => (
            <button
              key={c.id}
              className={`ot-city${selected.id === c.id ? " sel" : ""}`}
              data-testid={`city-${c.id}`}
              style={{ animationDelay: `${0.24 + i * 0.05}s` }}
              onClick={() => {
                setSelected(c);
                // Drop a city-specific challenge that no longer applies to the new city.
                const sc = scenario ? SCENARIOS[scenario] : null;
                if (sc?.cityId && sc.cityId !== c.id) setScenario(null);
              }}
            >
              <div style={{ font: "600 16px system-ui" }}>{c.name}</div>
              <div style={{ color: "#9aa3ad", fontSize: "12px", marginTop: "3px" }}>{c.blurb}</div>
              {/* The score-chase line: the city's real network is the bar; the personal best is
                  the player's standing against it (from-scratch runs only). */}
              <div data-testid={`city-score-${c.id}`} style={{ fontSize: "11px", marginTop: "4px", color: "#8a93a3" }}>
                real network ~{c.realScore}
                {(() => {
                  const best = personalBest(c.id);
                  if (best === null) return null;
                  const beat = best > c.realScore;
                  return (
                    <span style={{ color: beat ? "#1ab560" : "#8a93a3" }}>
                      {" "}· your best {best}{beat ? " 🏆" : ""}
                    </span>
                  );
                })()}
              </div>
            </button>
          ))}
        </div>

        {/* Start mode toggle. */}
        <div className="ot-mode">
          <button
            className={`ot-modebtn${withNetwork ? " sel" : ""}`}
            data-testid="mode-network"
            onClick={() => setWithNetwork(true)}
          >
            Start with the real network
          </button>
          <button
            className={`ot-modebtn${!withNetwork ? " sel" : ""}`}
            data-testid="mode-sandbox"
            onClick={() => setWithNetwork(false)}
          >
            Empty sandbox
          </button>
        </div>

        {/* Optional challenge: Free Play, or a scored scenario (objectives layer). */}
        <div className="ot-mode" style={{ marginTop: "-4px" }}>
          <button
            className={`ot-modebtn${scenario === null ? " sel" : ""}`}
            data-testid="scenario-none"
            onClick={() => setScenario(null)}
          >
            Free play
          </button>
          {Object.values(SCENARIOS)
            .filter((sc) => !sc.cityId || sc.cityId === selected.id)
            .map((sc) => (
            <button
              key={sc.id}
              className={`ot-modebtn${scenario === sc.id ? " sel" : ""}`}
              data-testid={`scenario-${sc.id}`}
              title={sc.blurb}
              onClick={() => setScenario(sc.id)}
            >
              🎯 {sc.title}
            </button>
          ))}
        </div>

        <button className="ot-start" data-testid="start" onClick={() => onStart(selected, withNetwork, scenario)}>
          ▶ Start
        </button>

        {save && onResume && (
          <button
            data-testid="resume"
            onClick={() => onResume(save)}
            style={{
              pointerEvents: "auto",
              marginTop: "12px",
              padding: "9px 22px",
              border: "1px solid #39414a",
              borderRadius: "9px",
              background: "#1c232b",
              color: "#eef1f4",
              font: "600 14px system-ui",
              cursor: "pointer",
            }}
          >
            ↩ Resume {save.cityName}
          </button>
        )}
      </div>

      {/* Bottom-left: the clickable station-master pigeon — hops + chirps when poked. */}
      <div className="ot-mascot-wrap">
        <button
          ref={mascotRef}
          className="ot-mascot"
          data-testid="mascot"
          aria-label="Mascot"
          onClick={pokeMascot}
          onAnimationEnd={(e) => {
            if (e.animationName === "ot-jump") mascotRef.current?.classList.remove("jump");
          }}
        >
          <div ref={bubbleRef} className="ot-bubble" />
          <img src={withBase("/title/mascot.webp")} alt="Station-master pigeon" decoding="async" />
        </button>
      </div>

      {/* Bottom-right: the real wordmark. */}
      <div className="ot-brand">
        <img className="ot-logo" src={withBase("/title/logo.webp")} alt="Transit Story" decoding="async" />
      </div>
    </div>
  );
}
