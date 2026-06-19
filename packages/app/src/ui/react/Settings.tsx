// Settings panel (⚙) — sectioned (item 6): Graphics · Gameplay · Keyboard controls · Audio. Each
// section is a labelled group of token-style toggles; the toggles wire to existing Game/loop state
// (transport-mode gates, economy, demand model, day-night tint, peeps visibility, sound). The
// Keyboard-controls section renders the KEYMAP (keys.tsx) as a read-only legend (item 5c). Opens
// from the bottom-left CornerCluster ⚙; reads Game/Stats state and re-renders on its hook slices.
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useGame, useGameUI, useStats } from "./GameContext";
import { MODES } from "./shared";
import { KEYMAP, Kbd } from "./keys";
import { audio } from "../../fx/audio";

// A token-style switch: 38×22 track + sliding knob (mirrors the vanilla toggleRow look).
function Toggle({
  label,
  testid,
  on,
  onToggle,
}: {
  label: string;
  testid: string;
  on: boolean;
  onToggle: () => void;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: 8,
        padding: "5px 0",
        cursor: "pointer",
      }}
    >
      <span>{label}</span>
      <button
        data-testid={testid}
        onClick={(e) => {
          e.preventDefault();
          onToggle();
        }}
        style={{
          width: 38,
          height: 22,
          borderRadius: 11,
          border: 0,
          cursor: "pointer",
          position: "relative",
          transition: "background .12s",
          flex: "0 0 auto",
          // ON = lit go-green (semantic); OFF = a recessed dark well in the console face.
          background: on ? "var(--ot-con-green)" : "var(--ot-well-bg)",
          boxShadow: on ? "0 0 8px var(--ot-con-green)" : "var(--ot-well)",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            width: 18,
            height: 18,
            borderRadius: "50%",
            background: "#e6ebf2",
            transition: "left .12s",
            boxShadow: "0 1px 2px rgba(0,0,0,.5)",
            left: on ? 18 : 2,
          }}
        />
      </button>
    </label>
  );
}

/** A labelled settings SECTION — a dim heading + a top rule, so the flat toggle list reads as
 *  grouped concerns (Graphics / Gameplay / Keyboard / Audio). The first section omits the top rule. */
function Section({ title, first, children }: { title: string; first?: boolean; children: ReactNode }) {
  return (
    <div
      style={{
        margin: first ? "6px 0 0" : "12px 0 0",
        paddingTop: first ? 0 : 10,
        borderTop: first ? undefined : "1px solid rgba(255,255,255,.08)",
      }}
    >
      <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, fontWeight: 700, letterSpacing: ".04em", textTransform: "uppercase", marginBottom: 4 }}>
        {title}
      </div>
      {children}
    </div>
  );
}

export function Settings({ open, onClose }: { open: boolean; onClose: () => void }) {
  const game = useGame();
  const ui = useGameUI();
  const stats = useStats();
  // Demand model is tracked on Game (no sim-stats field); the toggle is its only mutator.
  const [agentDemand, setAgentDemand] = useState(game.agentDemand);
  // Sound is owned by the audio kit (persisted in localStorage); mirror it in local state.
  const [soundOn, setSoundOn] = useState(!audio.muted);
  // Day/night map tint (default on); the sky module owns the actual divs.
  const [dayNight, setDayNight] = useState(true);

  // #25 Escape closes the panel (the onClose App plumbs was unused) — modal-style panels need a non-hunt exit,
  // matching Onboarding/Tutorial. Listener only mounts while open; harmless no-op otherwise.
  useEffect(() => {
    if (!open) return;
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [open, onClose]);

  if (!open) return null;

  const enabled = new Set(ui.enabledModes);
  const arcadia = ui.ruleset === "arcadia";

  return (
    <div
      id="settings-panel"
      data-testid="settings-panel"
      className="ot-console"
      style={{
        // Anchored near the bottom-LEFT ⚙ trigger in the CornerCluster. Opens upward from just above
        // the corner cluster so the ⚙ and its panel read as one control. Capped height with scroll so
        // it never runs off the top on a short viewport.
        position: "fixed",
        bottom: 60,
        left: 14,
        width: 260,
        maxHeight: "calc(100vh - 80px)",
        overflowY: "auto",
        padding: 14,
        display: "block",
        zIndex: 11,
        font: "13px system-ui,sans-serif",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 2 }}>
        <div style={{ fontWeight: 700, color: "var(--ot-con-ink)" }}>Settings</div>
        <button
          onClick={onClose}
          aria-label="Close settings"
          data-testid="settings-close"
          style={{ border: 0, background: "transparent", color: "var(--ot-con-ink-dim)", cursor: "pointer", font: "15px system-ui", lineHeight: 1, padding: 0 }}
        >
          ✕
        </button>
      </div>

      {/* ── GRAPHICS ── visual-only toggles + a CB-safe / reduced-motion note. */}
      <Section title="Graphics" first>
        <Toggle
          label="🌗  Day / night tint"
          testid="setting-daynight"
          on={dayNight}
          onToggle={() => {
            const on = !dayNight;
            setDayNight(on);
            game.sky.setEnabled(on);
            if (on) game.sky.set(stats.simHour); // re-apply now, don't wait for the next 3 Hz tick
          }}
        />
        <Toggle
          label="🧍  Show peeps"
          testid="setting-peeps"
          on={ui.showPeeps}
          onToggle={() => game.setShowPeeps(!ui.showPeeps)}
        />
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 10, lineHeight: 1.35, marginTop: 4 }}>
          🎨 Line colours are from a colour-blind-safe palette, always paired with a name/swatch.
          Motion respects your system's “reduce motion” setting automatically.
        </div>
      </Section>

      {/* ── GAMEPLAY ── the rules of the sim: economy, demand model, which transport modes are buildable. */}
      <Section title="Gameplay">
        <Toggle
          label="💰  Capital & fares"
          testid="setting-economy"
          on={stats.economyEnabled}
          onToggle={() => game.setEconomy(!stats.economyEnabled)}
        />
        <Toggle
          label="🧍  Citizen agents"
          testid="setting-agents"
          on={agentDemand}
          onToggle={() => {
            const v = !agentDemand;
            setAgentDemand(v);
            game.setDemandMode(v);
          }}
        />
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 10, lineHeight: 1.3, margin: "2px 0 6px" }}>
          Trips come from a population with homes & jobs instead of gravity flow.
        </div>
        <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 11, margin: "4px 0 2px" }}>Transport modes</div>
        {MODES.map((m) => (
          <Toggle
            key={m.id}
            label={`${m.icon}  ${m.name}`}
            testid={`setting-mode-${m.id}`}
            on={enabled.has(m.id)}
            onToggle={() => game.setModeEnabled(m.id, !enabled.has(m.id))}
          />
        ))}
      </Section>

      {/* ── KEYBOARD CONTROLS ── (item 5c) a read-only legend rendered straight off the KEYMAP source
          of truth (keys.tsx), so it can never drift from the actual listeners. */}
      <Section title="Keyboard controls">
        {KEYMAP.map((group) => {
          // Hide fantasy-only shortcuts (Barracks/Bounty) in transit — they're inert there.
          const bindings = group.bindings.filter((b) => !b.fantasyOnly || arcadia);
          if (bindings.length === 0) return null;
          return (
            <div key={group.title} style={{ marginBottom: 6 }}>
              <div style={{ color: "var(--ot-con-ink-dim)", fontSize: 10, fontWeight: 700, margin: "4px 0 2px" }}>{group.title}</div>
              {bindings.map((b) => (
                <div key={`${group.title}-${b.keys}`} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, padding: "2px 0" }}>
                  <span style={{ color: "var(--ot-con-ink)", fontSize: 12 }}>{b.label}</span>
                  <Kbd>{b.keys}</Kbd>
                </div>
              ))}
            </div>
          );
        })}
      </Section>

      {/* ── AUDIO ── */}
      <Section title="Audio">
        <Toggle
          label="🔊  Sound effects"
          testid="setting-sound"
          on={soundOn}
          onToggle={() => {
            const on = !soundOn;
            setSoundOn(on);
            audio.unlock(); // this click is a user gesture — start/resume the context
            audio.setMuted(!on);
            if (on) audio.tick(); // immediate confirmation that sound is back
          }}
        />
      </Section>
    </div>
  );
}
