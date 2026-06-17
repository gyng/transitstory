// The realm's SPELL BAR (fantasy/arcadia, S11): cast MANA-funded spells at the live threats. Spells are
// AUTO-TARGETED (the engine picks the target) but PLAYER-cast — the player chooses WHEN, the live tradeoff
// against banking mana for tech (one pool). An AUTOCAST checkbox restores the hands-off Majesty mode.
// Pure outer-ring chrome — reads the ~3 Hz stats slice (mana / autocast / the techUnlocked bitset) and
// emits ONE Command per cast/toggle via Game (the core gates on SPELLCRAFT + afford + a valid target and
// rejects otherwise, surfaced as the Toast). Top-right; arcadia only; mounts once Arcane Awakening is owned.
import { useEffect, useState } from "react";
import { useGame, useStats } from "./GameContext";
import { SPELLS, techUnlocked } from "../../commands/codec";

// The SPELLCRAFT tech id (Arcane Awakening) — mirrors tech.rs SPELLCRAFT; gates the whole arm.
const SPELLCRAFT = 11;

export function SpellBar() {
  const s = useStats();
  const game = useGame();
  // Optimistic local mirror of the autocast toggle so the checkbox flips instantly, then resyncs from the
  // next ~3 Hz snapshot (the committed value) — the uncontrolled-resync pattern, for a checkbox.
  const [auto, setAuto] = useState(s.autocast);
  useEffect(() => setAuto(s.autocast), [s.autocast]);

  if (s.ruleset !== "arcadia" || !techUnlocked(s.techUnlocked, SPELLCRAFT)) return null;
  const mana = Math.round(s.mana);

  return (
    <div
      data-testid="spell-bar"
      className="ot-console"
      style={{
        // Bottom-right ability-bar slot (RTS convention). Clear of the EditorPanel (top-right, on
        // selection), TechPanel (bottom-left), Toolbar (bottom-centre); CommuterCard (bottom-right) is
        // transit-only, so it never co-occurs with the arcadia spell bar.
        position: "fixed",
        bottom: 14,
        right: 14,
        zIndex: 10,
        width: 184,
        padding: "10px 12px",
        font: "13px system-ui,sans-serif",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 6 }}>
        {/* Arcane purple is the spell-arm's identity hue — kept as the diegetic magic accent on the dark console face. */}
        <b style={{ color: "#b388ff" }}>✦ Spells</b>
        <span title="Mana — the cast resource, shared with tech" className="ot-readout" style={{ color: "#b388ff", fontVariantNumeric: "tabular-nums", padding: "1px 7px" }}>
          ✦ {mana}
        </span>
      </div>
      {SPELLS.map((sp) => {
        const affordable = mana >= sp.cost;
        const disabled = !affordable || auto; // autocast on ⇒ the AI casts; manual buttons stand down
        return (
          <button
            key={sp.kind}
            data-testid={`spell-${sp.kind}`}
            className="ot-key"
            disabled={disabled}
            onClick={() => game.castSpell(sp.kind)}
            title={auto ? `${sp.blurb} — autocast is handling this` : sp.blurb}
            style={{
              display: "block",
              width: "100%",
              textAlign: "left",
              margin: "3px 0",
              padding: "5px 8px",
              cursor: disabled ? "default" : "pointer",
              font: "inherit",
              ...(disabled ? { opacity: 0.5, filter: "saturate(0.4)" } : null),
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", fontWeight: 600, color: "var(--ot-con-ink)" }}>
              <span>{sp.glyph} {sp.name}</span>
              <span style={{ fontVariantNumeric: "tabular-nums", color: affordable ? "#b388ff" : "var(--ot-con-ink-dim)" }}>✦ {sp.cost}</span>
            </div>
            <div style={{ fontSize: 11, color: "var(--ot-con-ink-dim)" }}>{sp.blurb}</div>
          </button>
        );
      })}
      <label
        data-testid="autocast-toggle"
        title="Autocast — let your magi fire spells automatically at the biggest threat (hands-off). Off = you choose when, banking mana for tech."
        style={{ display: "flex", alignItems: "center", gap: 6, marginTop: 6, cursor: "pointer", color: "var(--ot-con-ink-dim)", fontSize: 12 }}
      >
        <input
          type="checkbox"
          checked={auto}
          onChange={(e) => {
            setAuto(e.target.checked);
            game.setAutocast(e.target.checked);
          }}
        />
        Autocast at threats
      </label>
    </div>
  );
}
