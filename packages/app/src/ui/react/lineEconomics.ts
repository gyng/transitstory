// Per-line economics + rider satisfaction, DERIVED on the frontend from the Stats snapshot the
// sim already produces (no core change, no new wasm query). Framework-free (no JSX) so the line
// roster, the editor, and the stats dashboard all read one source of truth and can't drift.
//
// Why frontend-derived: revenue and capital are *exact* from the snapshot (fares are a flat
// per-boarding charge; capital is per-line). Opex is a single global drain the snapshot doesn't
// split per line, so per-line P&L here is fares − capital (lifetime payback); the ledger shows
// total opex. Satisfaction is a service-quality score from the two per-line levers the snapshot
// exposes: crowding (load factor) and wait (headway/2).
import type { PerLine, Stats } from "../../types";

/** The flat fare ($/boarding) the sim charges, recovered from the global totals so nothing here
 *  hardcodes the core constant (fareRevenue = ridershipTotal × FARE). 0 before any boarding. */
export function effectiveFare(s: Stats): number {
  return s.ridershipTotal > 0 ? s.fareRevenue / s.ridershipTotal : 0;
}

export interface LinePnl {
  /** Cumulative fares this line has earned (exact: line ridership × the sim's fare). */
  revenue: number;
  /** One-time capital it cost to build (track + land + trains). */
  capital: number;
  /** revenue − capital: lifetime P&L — is the line in the black on its build cost yet? */
  net: number;
  inBlack: boolean;
}

/** Per-line profit/loss. Revenue is exact; capital is exact; opex (a global drain) is excluded —
 *  see the file header. `net` is the "has this line earned back what it cost to build" signal. */
export function linePnl(l: PerLine, s: Stats): LinePnl {
  const revenue = l.ridership * effectiveFare(s);
  const capital = l.capitalCost;
  const net = revenue - capital;
  return { revenue, capital, net, inBlack: net >= 0 };
}

export interface Satisfaction {
  /** 0..100 rider-happiness score. */
  score: number;
  /** Colour-blind-safe PRIMARY channel: the face shape carries the band; colour is secondary. */
  glyph: string;
  color: string;
  word: string;
}

/** Rider satisfaction (0..100) for a line, from the two service-quality levers the snapshot
 *  exposes per line — crowding (load factor) and wait (≈ headway/2). A comfortable, frequent line
 *  scores high; a packed or infrequent one scores low. Returns null with no service (no trains). */
export function lineSatisfaction(l: PerLine): Satisfaction | null {
  if (l.trains <= 0) return null;
  // Crowding: comfortable up to ~70% load, then unhappiness ramps; crush (>~110%) is miserable.
  const crowd = l.loadFactor <= 0.7 ? 0 : Math.min(60, (l.loadFactor - 0.7) * 150);
  // Wait: about half the headway. Painless under ~4 min, then ramps.
  const waitMin = l.headwayMs / 2 / 60_000;
  const wait = waitMin <= 4 ? 0 : Math.min(40, (waitMin - 4) * 4);
  const score = Math.max(0, Math.min(100, Math.round(100 - crowd - wait)));
  if (score >= 70) return { score, glyph: "😀", color: "var(--ot-gauge-good,#009e73)", word: "happy" };
  if (score >= 45) return { score, glyph: "😐", color: "#e69f00", word: "ok" };
  return { score, glyph: "😟", color: "var(--ot-gauge-bad,#d62828)", word: "unhappy" };
}

/** Compact signed-money formatter for the roster (tight columns): +$1.2M / −$340k / +$980. */
export function fmtSignedMoney(d: number): string {
  const sign = d < 0 ? "−" : "+";
  const a = Math.abs(d);
  if (a >= 1e9) return `${sign}$${(a / 1e9).toFixed(2)}B`;
  if (a >= 1e6) return `${sign}$${(a / 1e6).toFixed(1)}M`;
  if (a >= 1e3) return `${sign}$${Math.round(a / 1e3)}k`;
  return `${sign}$${Math.round(a)}`;
}
