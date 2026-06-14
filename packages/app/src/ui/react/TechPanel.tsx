// The realm's TECH panel (fantasy/arcadia, S11): spend TRIBUTE (the war-chest that also funds legions)
// on permanent upgrades. Pure outer-ring chrome — reads the ~3 Hz stats slice (tribute + the techUnlocked
// bitset) and emits ONE Command per purchase via Game.unlockTech (the core afford-gates + rejects a
// repeat/broke unlock, so the panel just resyncs from the next snapshot — no optimistic sim state).
// Bottom-left anchor; only mounts in arcadia (returns null otherwise), so it's never transit chrome.
import { useGame, useStats } from "./GameContext";
import { TECHS, techUnlocked } from "../../commands/codec";

export function TechPanel() {
  const s = useStats();
  const game = useGame();
  if (s.ruleset !== "arcadia") return null;
  const tribute = Math.round(s.tribute);

  return (
    <div
      data-testid="tech-panel"
      style={{
        position: "fixed",
        bottom: 10,
        left: 10,
        zIndex: 9,
        width: 196,
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
        <span title="Tribute available to spend" style={{ color: "#7a818a", fontVariantNumeric: "tabular-nums" }}>
          ⚜ {tribute}
        </span>
      </div>
      {TECHS.map((t) => {
        const owned = techUnlocked(s.techUnlocked, t.id);
        const affordable = !owned && tribute >= t.cost;
        return (
          <button
            key={t.id}
            data-testid={`tech-${t.id}`}
            data-owned={owned ? "1" : "0"}
            disabled={!affordable}
            onClick={() => game.unlockTech(t.id)}
            title={t.blurb}
            style={{
              display: "block",
              width: "100%",
              textAlign: "left",
              margin: "4px 0",
              padding: "6px 8px",
              border: owned ? "1px solid var(--ot-gauge-good,#009e73)" : "1px solid #d7dade",
              borderRadius: 7,
              background: owned ? "rgba(0,158,115,.10)" : affordable ? "#fff" : "#f3f4f6",
              color: owned ? "#0a7d5c" : affordable ? "#1c2024" : "#9aa1a9",
              cursor: affordable ? "pointer" : "default",
              font: "inherit",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", fontWeight: 600 }}>
              <span>{t.name}</span>
              <span style={{ fontVariantNumeric: "tabular-nums" }}>{owned ? "✓ owned" : `⚜ ${t.cost}`}</span>
            </div>
            <div style={{ fontSize: 11, color: owned ? "#3a9b7e" : affordable ? "#7a818a" : "#aab0b7" }}>{t.blurb}</div>
          </button>
        );
      })}
    </div>
  );
}
