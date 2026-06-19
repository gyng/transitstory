// Per-line economics + rider satisfaction, DERIVED on the frontend from the Stats snapshot the
// sim already produces (no core change, no new wasm query). Framework-free (no JSX) so the line
// roster, the editor, and the stats dashboard all read one source of truth and can't drift.
//
// Why frontend-derived: revenue and capital are *exact* from the snapshot (fares are a flat
// per-boarding charge; capital is per-line). The sim now ALSO exposes a per-line running-cost rate
// (LineStat.opexPerDay — its trains + track-km bucketed from the global opex drain), so `net` is the
// lifetime fares − capital payback AND `opexPerDay` is the operating burn. Satisfaction is a
// service-quality score from the two per-line levers the snapshot exposes: crowding + wait.
import type { PerLine, Stats } from "../../types";
import { SIM_MS_PER_CLOCK_MIN } from "./shared";

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
  /** Running cost ($/in-game-day) — this line's share of the network opex drain (trains + track-km).
   *  The operating burn, distinct from the one-time capital: a line in the black on its build can
   *  still cost more to RUN than it earns. 0 with no trains / no track. */
  opexPerDay: number;
}

/** Per-line profit/loss. Revenue + capital are exact; `net` is lifetime payback (fares − capital);
 *  `opexPerDay` is the operating burn the sim now buckets per line (see the file header). */
export function linePnl(l: PerLine, s: Stats): LinePnl {
  const revenue = l.ridership * effectiveFare(s);
  const capital = l.capitalCost;
  const net = revenue - capital;
  return { revenue, capital, net, inBlack: net >= 0, opexPerDay: l.opexPerDay ?? 0 };
}

export interface Satisfaction {
  /** 0..100 rider-happiness score. */
  score: number;
  /** Colour-blind-safe PRIMARY channel: the face shape carries the band; colour is secondary. */
  glyph: string;
  color: string;
  word: string;
}

/** Mean live queue at a line's stops — the third satisfaction input. Computed from the same
 *  snapshot as everything else (perStation.waiting is a LEVEL, not a cumulative counter, so the
 *  pressure self-corrects as soon as service improves). A station shared by several lines counts
 *  toward each — attribution is genuinely ambiguous there, and "my platform is crowded" is true
 *  for every line that calls at it. */
export function meanStopQueue(stops: number[], perStation: Map<number, { waiting: number }>): number {
  if (stops.length === 0) return 0;
  let sum = 0;
  for (const id of stops) sum += perStation.get(id)?.waiting ?? 0;
  return sum / stops.length;
}

/** Rider satisfaction (0..100) for a line, from the service-quality signals the snapshot exposes —
 *  crowding (load factor), wait (≈ headway/2), and (when the caller provides it) the live queue at
 *  its platforms. A comfortable, frequent line with drained platforms scores high; a packed,
 *  infrequent, or visibly-queueing one scores low. Returns null with no service (no trains). */
export function lineSatisfaction(l: PerLine, queueAtStops = 0): Satisfaction | null {
  if (l.trains <= 0) return null;
  // #19 Crowding, aligned to loadPip's bands (healthy <0.6, busy 0.6–0.9, crush ≥0.9) so the chip glyph and the
  // satisfaction score don't disagree side-by-side: the penalty starts at busy (0.6) and saturates by crush (0.9).
  const crowd = l.loadFactor <= 0.6 ? 0 : Math.min(60, (l.loadFactor - 0.6) * 200);
  // Wait: about half the headway. Painless under ~4 min, then ramps.
  const waitMin = l.headwayMs / 2 / SIM_MS_PER_CLOCK_MIN; // clock minutes (frame-unified)
  const wait = waitMin <= 4 ? 0 : Math.min(40, (waitMin - 4) * 4);
  // Queues: a handful of people waiting is a working network; double digits per platform isn't.
  const queue = queueAtStops <= 4 ? 0 : Math.min(40, (queueAtStops - 4) * 2);
  const score = Math.max(0, Math.min(100, Math.round(100 - crowd - wait - queue)));
  if (score >= 70) return { score, glyph: "😀", color: "var(--ot-gauge-good,#009e73)", word: "happy" };
  if (score >= 45) return { score, glyph: "😐", color: "#e69f00", word: "ok" };
  return { score, glyph: "😟", color: "var(--ot-con-red)", word: "unhappy" };
}

/** Compact signed-money for the roster (tight columns): +$1.2M / −$340k / +$980. Same abbreviation
 *  discipline as shared.fmtMoney (1 dp under 10 of a unit, rounded above — so round values aren't the
 *  fussy "$15.0M") but ALWAYS sign-prefixed (+/−), which is the roster's "is this line up or down" read. */
export function fmtSignedMoney(d: number): string {
  if (!Number.isFinite(d)) return "+$0"; // #8 a non-finite P&L delta must not render "+$NaN"
  const sign = d < 0 ? "−" : "+";
  const a = Math.abs(d);
  const unit = (v: number, suf: string) => `${sign}$${v < 10 ? v.toFixed(1) : Math.round(v)}${suf}`;
  if (a < 1000) return `${sign}$${Math.round(a)}`;
  if (a < 1e6) return unit(a / 1e3, "k");
  if (a < 1e9) return unit(a / 1e6, "M");
  const b = a / 1e9;
  return `${sign}$${b < 10 ? b.toFixed(2) : b.toFixed(1)}B`;
}

// #27 fmtCount's canonical home is shared.ts — re-export it here so Panels (which imports it from this module)
// keeps working while there's a single source of truth (this was a duplicate "drift is a bug" trap; the bodies
// were behaviourally identical).
export { fmtCount } from "./shared";

/** Shorten an official line name for the roster + derive a 2–3 char identity badge code. This is
 *  DISPLAY-ONLY (the command-log auto-name is untouched), and lives here so the roster, hover tips,
 *  and editor read one source and can't drift. Steps: strip a transit-class prefix ("MRT"/"LRT"/…),
 *  strip a "(terminus → terminus)" tail (spatial truth belongs on the map/editor, not the scan list),
 *  strip a redundant trailing " Line". Badge code = initials of the cleaned words (mode-letter+number
 *  for a numbered bus/ferry), else `L<n>`. The FULL name is preserved verbatim for the row tooltip. */
export function shortLineName(name: string, lineId: number): { code: string; short: string } {
  const raw = (name || "").trim();
  let short = raw
    .replace(/^(MRT|LRT|Metro|Subway|Line|Bus|Ferry)\s+/i, "")
    .replace(/\s*\([^)]*[→\-–][^)]*\)\s*$/, "")
    .replace(/\s+Line$/i, "")
    .trim();
  if (!short) short = raw || `Line ${lineId + 1}`;
  const words = short.split(/[\s–-]+/).filter((w) => w && !/^(the|of|and|line)$/i.test(w));
  const num = short.match(/(\d+)/);
  let code: string;
  if (/^(bus|ferry)/i.test(raw) && num) code = raw[0].toUpperCase() + num[1];
  else if (words.length >= 2) code = (words[0][0] + words[1][0]).toUpperCase();
  else if (words.length === 1) code = words[0].slice(0, 2).toUpperCase();
  else code = `L${lineId + 1}`;
  return { code: code.slice(0, 3), short };
}

/** Legible ink (#fff or near-black) for text laid over a `u32` swatch fill, by perceptual
 *  luminance — so a code badge stays readable on both a dark navy and a bright yellow line. */
export function swatchInk(color: number): string {
  const r = ((color >> 16) & 0xff) / 255;
  const g = ((color >> 8) & 0xff) / 255;
  const b = (color & 0xff) / 255;
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum < 0.6 ? "#ffffff" : "#1c2024";
}
