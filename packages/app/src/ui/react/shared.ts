// Shared UI constants + tiny formatters used across the React chrome. Kept framework-free
// (no JSX) so both presentational components and the provider can import it. Mode ids match
// crates/sim trainset::tmode (0 rail,1 bus,2 ferry,3 air,4 heavy/high-speed rail).

export interface ModeDef {
  id: number;
  key: string;
  icon: string;
  name: string;
  hint: string;
  color: string;
}

export const MODES: ModeDef[] = [
  { id: 0, key: "1", icon: "🚇", name: "Rail", color: "#0072b2",
    hint: "Place stations, then draw track. Surface routes avoid buildings — elevate or tunnel to cross built-up land and water." },
  { id: 1, key: "2", icon: "🚌", name: "Bus", color: "#d55e00",
    hint: "Runs on existing roads — cheap and quick to build, but lower capacity." },
  // Ferry tracks LINE_PALETTE[2] (Tol teal) — the old bluish-green doubled as the "healthy"
  // verdict colour, so a ferry chip read as a health badge. Mode-chip colours are toolbar
  // identity only (line colours auto-assign from LINE_PALETTE), but they shouldn't collide
  // with the semantic hues either.
  { id: 2, key: "3", icon: "⛴", name: "Ferry", color: "#44aa99",
    hint: "Terminals on the waterfront — routes cross open water with no track to build." },
  { id: 3, key: "4", icon: "✈", name: "Plane", color: "#cc79a7",
    hint: "Airports for long hops — flies over anything, at any distance." },
  { id: 4, key: "5", icon: "🚄", name: "Heavy Rail", color: "#9467bd",
    hint: "High-speed / mainline rail — very fast and high-capacity, but expensive and needs grade-separated track (elevate or tunnel through built-up land and water)." },
];

export const MODE_ICON = ["🚇", "🚌", "⛴", "✈", "🚄"];
export function modeIcon(m: number): string {
  return MODE_ICON[m] ?? "🚇";
}

/** Hand-mirror of the sim's AIR_ROSTER (trainset.rs) — index IS the `AssignTrainset.spec` id.
 *  A non-dominated capacity-vs-turnaround ladder: a bigger jet fills more per departure but sits
 *  longer at the gate (widening effective headway). Keep in lockstep with the Rust roster. */
export interface AircraftDef {
  name: string;
  capacity: number;
  turnS: number; // gate turnaround (the sim's dwell_ms / 1000)
  blurb: string;
}
export const AIR_ROSTER: AircraftDef[] = [
  { name: "Narrowbody", capacity: 250, turnS: 60, blurb: "A321/737 class — the all-round trunk jet" },
  { name: "Regional", capacity: 88, turnS: 45, blurb: "E175/CRJ class — fastest turn, keeps a thin spoke frequent" },
  { name: "Widebody", capacity: 410, turnS: 90, blurb: "777/A350 class — big ceiling for a fat pair" },
  { name: "Jumbo", capacity: 525, turnS: 120, blurb: "747/A380 class — max seats, slowest turn" },
];

/** u32 RGB → CSS hex string (#rrggbb). */
export function hex(u: number): string {
  return "#" + (u & 0xffffff).toString(16).padStart(6, "0");
}

// --- Station inspect (hover tooltip) -----------------------------------------------------
// The tooltip is deck-owned (the sanctioned getTooltip exception, not a DOM node anchored by
// lng/lat). Game assembles the StationTip from the snapshot; this renders it to HTML with the
// e2e testid contract. Verdict is null (placement-truth only) in Build / before the station has
// stats — never a confident "healthy" before any passenger has moved.

export interface StationTip {
  id: number;
  name: string;
  /** false in Build mode / before the station has a stats entry → show placement truth only. */
  hasData: boolean;
  waiting: number;
  boardings: number;
  alightings: number;
  verdict: "starved" | "busy" | "healthy" | null;
  /** Captured demand the catchment grabs, split by KIND — `homes` (origin/residential weight, where
   *  trips start) vs `jobs` (destination weight, where trips are pulled to). The split is the answer
   *  to "what's driving demand here": homes generate trips, jobs attract them (AM→PM flips which). */
  homes: number;
  jobs: number;
  /** Operational lines serving the station; 0 = orphaned (placed but not yet in service). */
  serving: number;
  /** Cumulative pressure here: full-train pass-bys (denied) + give-ups (abandoned). */
  denied: number;
  abandoned: number;
  /** `load` = this line's mean load factor (0..1), undefined in Build / before it has vehicles. */
  lines: { id: number; color: number; name: string; load?: number }[];
}

const VERDICT_COLOR = { starved: "#d62828", busy: "#e69f00", healthy: "#009e73" } as const;

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
}

function lineSwatches(tip: StationTip): string {
  if (tip.lines.length === 0) return `<span style="color:#7a818a">(no lines yet)</span>`;
  // One row per serving line: colour swatch (same hex() path as the deck PathLayer, so it never
  // drifts) + name + a load pip so an interchange shows AT A GLANCE which of its lines is the
  // crush one. The pip only appears once the line has running data (load !== undefined).
  return tip.lines
    .map((l) => {
      const pip = l.load !== undefined ? loadPip(l.load) : null;
      const tail = pip
        ? `<span style="margin-left:auto;color:${pip.color};font-weight:700">${pip.glyph} ${pip.pct}%</span>`
        : "";
      return (
        `<div data-testid="station-tip-line-${l.id}" style="display:flex;align-items:center;gap:4px;margin-top:2px">` +
        `<span style="display:inline-block;width:11px;height:11px;border-radius:3px;flex:none;background:${hex(l.color)}"></span>` +
        `<span style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:150px">${esc(l.name)}</span>${tail}</div>`
      );
    })
    .join("");
}

/** What's DRIVING demand here: the catchment's captured homes (trip origins) vs jobs (trip
 *  destinations), shown as separate channels so the player reads the two forces, not one opaque
 *  total. AM trips flow 🏠→💼, PM 💼→🏠 — seeing both is what makes "build homes→jobs" legible. */
function demandLine(tip: StationTip): string {
  const homes = Math.round(tip.homes);
  const jobs = Math.round(tip.jobs);
  if (homes + jobs <= 0) return "";
  const parts: string[] = [];
  if (homes > 0) parts.push(`🏠 ~<span data-testid="station-tip-homes">${homes}</span> homes`);
  if (jobs > 0) parts.push(`💼 ~<span data-testid="station-tip-jobs">${jobs}</span> jobs`);
  return `<div data-testid="station-tip-demand" style="color:#5a626b">${parts.join(" · ")}</div>`;
}

/** Orphaned warning (placed but no operational line serving it) — the "connect me" nudge. */
function orphanLine(tip: StationTip): string {
  if (tip.serving > 0) return "";
  return `<div data-testid="station-tip-orphan" style="color:#e69f00;font-weight:600">⚠ no service yet</div>`;
}

/** Cumulative loss AT THIS STATION: full-train pass-bys + give-ups. The per-platform failure
 *  signal — shown only when there's loss, so a healthy station stays uncluttered. */
function pressureLine(tip: StationTip): string {
  const denied = Math.round(tip.denied);
  const abandoned = Math.round(tip.abandoned);
  if (denied + abandoned <= 0) return "";
  const parts: string[] = [];
  if (denied > 0) parts.push(`<span data-testid="station-tip-denied">${denied}</span> passed by`);
  if (abandoned > 0) parts.push(`<span data-testid="station-tip-abandoned">${abandoned}</span> gave up`);
  return `<div style="color:#d62828">⊘ ${parts.join(" · ")}</div>`;
}

/** Render a StationTip to the deck tooltip HTML (carries the station-tip* testid contract). */
export function stationTipHtml(tip: StationTip): string {
  const lines = `<div data-testid="station-tip-lines" style="margin-top:5px">${lineSwatches(tip)}</div>`;
  const head = `<b data-testid="station-tip-name">◉ ${esc(tip.name)}</b>`;
  if (!tip.hasData) {
    return (
      `<div data-testid="station-tip" style="font:12px system-ui">${head}` +
      `<div style="color:#7a818a">covers ~500 m</div>${demandLine(tip)}${orphanLine(tip)}${lines}</div>`
    );
  }
  const v = tip.verdict ?? "healthy";
  return (
    `<div data-testid="station-tip" style="font:12px system-ui">${head}` +
    `<div style="margin-top:3px"><b data-testid="station-tip-waiting">${Math.round(tip.waiting)}</b> waiting ` +
    `<span data-testid="station-tip-verdict" style="color:${VERDICT_COLOR[v]};font-weight:700">${v.toUpperCase()}</span></div>` +
    `<div style="color:#5a626b">▲ <span data-testid="station-tip-boardings">${Math.round(tip.boardings)}</span> boarded · ` +
    `▼ <span data-testid="station-tip-alightings">${Math.round(tip.alightings)}</span> off</div>` +
    `${demandLine(tip)}${pressureLine(tip)}${orphanLine(tip)}${lines}</div>`
  );
}

/** Load-factor verdict for the line-inspect roster pip + Editor "Performance" row. The SHAPE
 *  (○ healthy / ◐ busy / ● crush) is the colour-blind-safe primary channel; colour is secondary;
 *  the word makes the band read as text, not hue. */
export function loadPip(lf: number): { glyph: string; color: string; word: string; pct: number } {
  const pct = Math.round(lf * 100);
  if (lf >= 0.9) return { glyph: "●", color: "var(--ot-gauge-bad)", word: "crush", pct };
  if (lf >= 0.6) return { glyph: "◐", color: "#e69f00", word: "busy", pct };
  return { glyph: "○", color: "var(--ot-gauge-good)", word: "healthy", pct };
}

// --- Train inspect (hover tooltip) -------------------------------------------------------
// A moving vehicle's readout: which line it serves + how full it is. Same loadPip shape/colour
// channel as the line roster, so "crush/busy/healthy" reads identically wherever it appears.

export interface VehicleTip {
  lineName: string;
  lineColor: number;
  modeIcon: string;
  onboard: number;
  capacity: number;
}

export function vehicleTipHtml(t: VehicleTip): string {
  const lf = t.capacity > 0 ? t.onboard / t.capacity : 0;
  const pip = loadPip(lf);
  const swatch =
    `<span style="display:inline-block;width:11px;height:11px;border-radius:3px;` +
    `vertical-align:-1px;background:${hex(t.lineColor)}"></span>`;
  return (
    `<div data-testid="vehicle-tip" style="font:12px system-ui">` +
    `<b data-testid="vehicle-tip-line">${t.modeIcon} ${swatch} ${esc(t.lineName)}</b>` +
    `<div style="margin-top:3px">` +
    `<span data-testid="vehicle-tip-load" style="color:${pip.color};font-weight:700">${pip.glyph} ${pip.word}</span> ` +
    `<span style="color:#5a626b">${t.onboard}/${t.capacity} aboard (${pip.pct}%)</span></div></div>`
  );
}

// --- Line inspect (hover tooltip) --------------------------------------------------------
// Hovering a line's track shows its at-a-glance stats without having to select it (the Editor
// stays the place to *edit*). Built from the same Stats snapshot the panels read, so they agree.

export interface LineTip {
  name: string;
  color: number;
  modeIcon: string;
  modeName: string;
  ridership: number;
  loadFactor: number;
  stops: number;
  trains: number;
  headwayMin: number;
}

export function lineTipHtml(t: LineTip): string {
  const pip = loadPip(t.loadFactor);
  const swatch =
    `<span style="display:inline-block;width:11px;height:11px;border-radius:3px;` +
    `vertical-align:-1px;margin-right:3px;background:${hex(t.color)}"></span>`;
  return (
    `<div data-testid="line-tip" style="font:12px system-ui;min-width:150px">` +
    `<b data-testid="line-tip-name">${swatch}${esc(t.name)}</b> ` +
    `<span style="color:#7a818a">${t.modeIcon} ${esc(t.modeName)}</span>` +
    `<div style="margin-top:4px;color:#5a626b"><b data-testid="line-tip-ridership">${Math.round(t.ridership)}</b> riders · ` +
    `<span style="color:${pip.color};font-weight:700">${pip.glyph} ${pip.word} ${pip.pct}%</span></div>` +
    `<div style="color:#7a818a;margin-top:2px">${t.stops} stops · ${t.trains} trains · every ${t.headwayMin} min</div></div>`
  );
}

/** Money formatter: $1.23B / $45M / $678k. */
export function fmtMoney(d: number): string {
  const a = Math.abs(d);
  return a >= 1e9 ? `$${(d / 1e9).toFixed(2)}B` : a >= 1e6 ? `$${Math.round(d / 1e6)}M` : `$${Math.round(d / 1e3)}k`;
}

// Shared inline-style fragments (token-driven; mirror the old vanilla chrome 1:1).
export const PANEL_STYLE =
  "position:fixed;background:rgba(255,255,255,.96);border-radius:10px;" +
  "box-shadow:var(--ot-shadow);z-index:9;font:13px system-ui,sans-serif;color:#1c2024";
