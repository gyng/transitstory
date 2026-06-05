// Tiny hand-rolled SVG charts — sparklines + horizontal bars. No charting dependency (AGENTS
// minimal-deps): a transit game ledger needs trend lines and a per-line ranking, both a few lines
// of SVG. Pure presentational; data is passed in (the dashboard owns the history + snapshot reads).
import type { CSSProperties } from "react";

/** A filled sparkline over a numeric series. `zeroLine` draws a dashed baseline at 0 for series
 *  that can go negative (e.g. balance). Colours are #rrggbb (alpha is appended for the fill). */
export function Sparkline({
  values,
  width = 232,
  height = 54,
  color = "#0072b2",
  fill = true,
  zeroLine = false,
}: {
  values: number[];
  width?: number;
  height?: number;
  color?: string;
  fill?: boolean;
  zeroLine?: boolean;
}) {
  const pad = 4;
  if (values.length < 2) {
    return (
      <svg width={width} height={height} style={{ display: "block" }}>
        <text x={width / 2} y={height / 2} textAnchor="middle" fill="#b3b9c0" fontSize="11">
          gathering data…
        </text>
      </svg>
    );
  }
  let min = Math.min(...values);
  let max = Math.max(...values);
  if (zeroLine) {
    min = Math.min(min, 0);
    max = Math.max(max, 0);
  }
  if (min === max) max = min + 1;
  const xAt = (i: number) => pad + (i / (values.length - 1)) * (width - 2 * pad);
  const yAt = (v: number) => height - pad - ((v - min) / (max - min)) * (height - 2 * pad);
  const line = values.map((v, i) => `${xAt(i).toFixed(1)},${yAt(v).toFixed(1)}`).join(" ");
  const area = `${pad},${height - pad} ${line} ${width - pad},${height - pad}`;
  return (
    <svg width={width} height={height} style={{ display: "block" }}>
      {fill && <polyline points={area} fill={`${color}22`} stroke="none" />}
      {zeroLine && min < 0 && max > 0 && (
        <line x1={pad} x2={width - pad} y1={yAt(0)} y2={yAt(0)} stroke="#c9ced4" strokeDasharray="2 3" />
      )}
      <polyline points={line} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}

/** A titled chart card: title + current value on top, sparkline below. */
export function ChartCard({
  title,
  values,
  color,
  format,
  zeroLine,
  testid,
}: {
  title: string;
  values: number[];
  color: string;
  format: (v: number) => string;
  zeroLine?: boolean;
  testid?: string;
}) {
  const last = values.length ? values[values.length - 1] : 0;
  return (
    <div data-testid={testid} style={{ background: "#fff", borderRadius: 8, padding: "8px 10px", border: "1px solid #eceef1" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 2 }}>
        <span style={{ fontSize: 11, color: "#7a818a" }}>{title}</span>
        <b style={{ fontSize: 13, color }}>{format(last)}</b>
      </div>
      <Sparkline values={values} color={color} zeroLine={zeroLine} />
    </div>
  );
}

/** Horizontal bar ranking (e.g. per-line ridership). Bars normalize to the max value. */
export function BarList({
  items,
  format,
  style,
}: {
  items: { key: string | number; label: string; value: number; color: string }[];
  format: (v: number) => string;
  style?: CSSProperties;
}) {
  const max = Math.max(1, ...items.map((i) => i.value));
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 5, ...style }}>
      {items.map((it) => (
        <div key={it.key}>
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11, marginBottom: 2 }}>
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 150 }}>{it.label}</span>
            <span style={{ color: "#7a818a", flex: "0 0 auto", marginLeft: 6 }}>{format(it.value)}</span>
          </div>
          <div style={{ height: 6, background: "#eef0f3", borderRadius: 3, overflow: "hidden" }}>
            <div style={{ width: `${(it.value / max) * 100}%`, height: 6, background: it.color, borderRadius: 3 }} />
          </div>
        </div>
      ))}
    </div>
  );
}
