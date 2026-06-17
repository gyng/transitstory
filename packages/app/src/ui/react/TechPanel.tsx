// The realm's TECH panel (fantasy/arcadia, S11): spend MANA (the sole tech resource — minted by aether
// chains) on permanent upgrades. Pure outer-ring chrome — reads the ~3 Hz stats slice (mana + the
// techUnlocked bitset) and emits ONE Command per purchase via Game.unlockTech (the core afford-gates +
// prereq-gates + rejects a repeat, so the panel just resyncs from the next snapshot). Bottom-left; arcadia
// only. Techs gate behind their tier-1 prereq, so the panel reads as a small tree.
import { useState } from "react";
import { useGame, useStats } from "./GameContext";
import { TECHS, techUnlocked } from "../../commands/codec";

// `open`/`onOpenChange` (stage 7): the TECH construction category key now CONTROLS the panel's open
// state (the deviation fix) — arming TECH opens it, closing it disarms back to the rail. When the
// props are omitted the panel falls back to its own launcher button (legacy/standalone use).
export function TechPanel({ open: controlledOpen, onOpenChange }: { open?: boolean; onOpenChange?: (open: boolean) => void } = {}) {
  const s = useStats();
  const game = useGame();
  // Collapsed by default — the tech tree opens from the TECH category key (or its own launcher when
  // uncontrolled) instead of always occupying the bottom-left (trial feedback #2). Arcadia only.
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const controlled = controlledOpen !== undefined;
  const open = controlled ? controlledOpen : uncontrolledOpen;
  const setOpen = (v: boolean) => {
    if (!controlled) setUncontrolledOpen(v);
    onOpenChange?.(v);
  };
  if (s.ruleset !== "arcadia") return null;
  const mana = Math.round(s.mana);
  const owned = (id: number) => techUnlocked(s.techUnlocked, id);
  const prereqMet = (prereq: number) => prereq < 0 || owned(prereq);

  if (!open) {
    // CONTROLLED + closed: the TECH category key in the construction rail IS the launcher (the deviation
    // fix) — render no floating launcher (that would double the affordance + clutter the bottom-left).
    if (controlled) return null;
    // UNCONTROLLED (standalone) fallback: the floating Forge launcher. How many techs are affordable right
    // now → a subtle "you can spend" nudge on the launcher.
    const buyable = TECHS.filter((t) => !owned(t.id) && prereqMet(t.prereq) && mana >= t.cost).length;
    return (
      <button
        data-testid="tech-launcher"
        className="ot-key"
        onClick={() => setOpen(true)}
        title="Forge of Ages — spend mana on permanent upgrades"
        style={{
          // Stacked ABOVE the bottom-left Realm ledger (ServiceReport) so they don't overlap/intercept.
          position: "fixed", bottom: 215, left: 14, zIndex: 10, padding: "7px 12px",
          font: "600 13px system-ui,sans-serif", cursor: "pointer",
        }}
      >
        ⚒ Forge <span style={{ color: "#b794f6", fontVariantNumeric: "tabular-nums" }}>✦ {mana}</span>
        {buyable > 0 && (
          <span data-testid="tech-buyable" style={{ marginLeft: 6, background: "var(--ot-gauge-good,#009e73)", color: "#fff", borderRadius: 8, padding: "0 6px", fontSize: 11 }}>
            {buyable}
          </span>
        )}
      </button>
    );
  }

  return (
    <div
      data-testid="tech-panel"
      className="ot-console"
      style={{
        position: "fixed",
        bottom: 215, // stacked above the Realm ledger (bottom-left), like the launcher
        left: 14,
        zIndex: 10,
        width: 224,
        maxHeight: "62vh",
        overflowY: "auto",
        padding: "10px 12px",
        font: "13px system-ui,sans-serif",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
        <b>⚒ Forge of Ages</b>
        <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span title="Mana — the tech resource, minted by aether" style={{ color: "#b794f6", fontVariantNumeric: "tabular-nums" }}>
            ✦ {mana}
          </span>
          <button
            data-testid="tech-close"
            onClick={() => setOpen(false)}
            title="Close"
            style={{ border: 0, background: "transparent", color: "var(--ot-con-ink-dim)", cursor: "pointer", font: "15px system-ui", lineHeight: 1, padding: 0 }}
          >
            ✕
          </button>
        </span>
      </div>
      {TECHS.map((t) => {
        const own = owned(t.id);
        const unlocked = prereqMet(t.prereq);
        const affordable = !own && unlocked && mana >= t.cost;
        const dim = !own && !unlocked; // prereq not met → locked
        const prereqName = t.prereq >= 0 ? TECHS[t.prereq]?.name : null;
        return (
          <button
            key={t.id}
            data-testid={`tech-${t.id}`}
            data-owned={own ? "1" : "0"}
            disabled={!affordable}
            onClick={() => game.unlockTech(t.id)}
            title={dim && prereqName ? `${t.blurb} — needs ${prereqName}` : t.blurb}
            style={{
              display: "block",
              width: "100%",
              textAlign: "left",
              margin: "3px 0",
              marginLeft: t.tier > 1 ? 10 : 0,
              padding: "5px 8px",
              border: own
                ? "1px solid var(--ot-gauge-good,#009e73)"
                : affordable
                ? "1px solid rgba(56,198,220,.45)"
                : "1px solid rgba(255,255,255,.08)",
              borderRadius: 7,
              background: own
                ? "rgba(0,158,115,.16)"
                : affordable
                ? "rgba(56,198,220,.14)"
                : "rgba(255,255,255,.04)",
              color: own ? "var(--ot-gauge-good,#009e73)" : affordable ? "#fff" : dim ? "var(--ot-con-ink-dim)" : "var(--ot-con-ink-dim)",
              opacity: dim ? 0.7 : 1,
              cursor: affordable ? "pointer" : "default",
              font: "inherit",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", fontWeight: 600 }}>
              <span>{t.tier > 1 ? "↳ " : ""}{t.name}</span>
              <span style={{ fontVariantNumeric: "tabular-nums" }}>{own ? "✓" : dim && prereqName ? "🔒" : `✦ ${t.cost}`}</span>
            </div>
            <div style={{ fontSize: 11, color: own ? "var(--ot-gauge-good,#009e73)" : "var(--ot-con-ink-dim)" }}>{t.blurb}</div>
          </button>
        );
      })}
    </div>
  );
}
