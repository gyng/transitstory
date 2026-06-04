// Shared UI constants + tiny formatters used across the React chrome. Kept framework-free
// (no JSX) so both presentational components and the provider can import it. Mode ids match
// crates/sim trainset::tmode (0 rail,1 bus,2 ferry,3 air).

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
  { id: 2, key: "3", icon: "⛴", name: "Ferry", color: "#009e73",
    hint: "Terminals on the waterfront — routes cross open water with no track to build." },
  { id: 3, key: "4", icon: "✈", name: "Plane", color: "#cc79a7",
    hint: "Airports for long hops — flies over anything, at any distance." },
];

export const MODE_ICON = ["🚇", "🚌", "⛴", "✈"];
export function modeIcon(m: number): string {
  return MODE_ICON[m] ?? "🚇";
}

/** u32 RGB → CSS hex string (#rrggbb). */
export function hex(u: number): string {
  return "#" + (u & 0xffffff).toString(16).padStart(6, "0");
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
