// The realm's TECH panel (fantasy/arcadia, S11): spend MANA (the sole tech resource — minted by aether
// chains) on permanent upgrades. Pure outer-ring chrome — reads the ~3 Hz stats slice (mana + the
// techUnlocked bitset) and emits ONE Command per purchase via Game.unlockTech (the core afford-gates +
// prereq-gates + rejects a repeat, so the panel just resyncs from the next snapshot). Bottom-left; arcadia
// only. Techs gate behind their tier-1 prereq, so the panel reads as a small tree.
import { useGame, useStats } from "./GameContext";
import { TECHS, techUnlocked } from "../../commands/codec";

export function TechPanel() {
  const s = useStats();
  const game = useGame();
  if (s.ruleset !== "arcadia") return null;
  const mana = Math.round(s.mana);
  const owned = (id: number) => techUnlocked(s.techUnlocked, id);
  const prereqMet = (prereq: number) => prereq < 0 || owned(prereq);

  return (
    <div
      data-testid="tech-panel"
      style={{
        position: "fixed",
        bottom: 10,
        left: 10,
        zIndex: 9,
        width: 224,
        maxHeight: "62vh",
        overflowY: "auto",
        padding: "10px 12px",
        borderRadius: 10,
        background: "rgba(255,255,255,.95)",
        boxShadow: "var(--ot-shadow, 0 2px 10px rgba(0,0,0,.12))",
        font: "13px system-ui,sans-serif",
        color: "#1c2024",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 6 }}>
        <b>⚒ Forge of Ages</b>
        <span title="Mana — the tech resource, minted by aether" style={{ color: "#7a4ed2", fontVariantNumeric: "tabular-nums" }}>
          ✦ {mana}
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
              border: own ? "1px solid var(--ot-gauge-good,#009e73)" : "1px solid #d7dade",
              borderRadius: 7,
              background: own ? "rgba(0,158,115,.10)" : affordable ? "#fff" : "#f3f4f6",
              color: own ? "#0a7d5c" : affordable ? "#1c2024" : dim ? "#b3b8bf" : "#9aa1a9",
              cursor: affordable ? "pointer" : "default",
              font: "inherit",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", fontWeight: 600 }}>
              <span>{t.tier > 1 ? "↳ " : ""}{t.name}</span>
              <span style={{ fontVariantNumeric: "tabular-nums" }}>{own ? "✓" : dim && prereqName ? "🔒" : `✦ ${t.cost}`}</span>
            </div>
            <div style={{ fontSize: 11, color: own ? "#3a9b7e" : "#8a909a" }}>{t.blurb}</div>
          </button>
        );
      })}
    </div>
  );
}
