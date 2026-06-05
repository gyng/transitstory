// Builds the deck.gl overlay layers from a plain RenderView (already in lng/lat — all mm
// conversion happened at the geo.ts boundary in Game). Layer array order IS the z-order
// (AGENTS IA): catchment < lines < blueprint < stations < vehicles < selection highlight.
import type { Layer } from "@deck.gl/core";
import { IconLayer, PathLayer, ScatterplotLayer, TextLayer } from "@deck.gl/layers";
import { BUSY_WAITING, STARVED_WAITING } from "./config";

export type Rgb = [number, number, number];

export interface StationDot {
  id: number;
  lng: number;
  lat: number;
  name: string;
  selected: boolean;
  /** Cumulative boardings — scales the dot radius so busy stations visibly grow (throughput map). */
  boardings: number;
  /** Operational lines serving this station; 0 = orphaned → muted fill until it gets service. */
  serving: number;
}
export interface LinePath {
  id: number;
  color: Rgb;
  path: [number, number][];
  mode: number; // transport mode (trainset::tmode); 4 = heavy/high-speed rail (distinct styling)
}
export interface CatchmentCircle {
  lng: number;
  lat: number;
  radiusM: number;
  /** true = transient hover peek (stroke-only, fainter); false/undefined = pinned (filled). */
  peek?: boolean;
  /** Captured-demand strength 0..1 — scales the pinned fill alpha so a station sitting on a lot
   *  of demand reads denser than one on empty land (which stations actually grab riders). */
  demand?: number;
}
export interface VehicleDot {
  lng: number;
  lat: number;
  color: Rgb;
  /** Heading in radians (0 = +x / east), from the sim's vehicleAngles buffer — drives the
   *  directional triangle so you read which way each train is travelling. */
  angle: number;
  /** Load factor (onboard / capacity), 0..~1 — drives the crowding ring colour + train size. */
  load: number;
}
export interface WaitingDot {
  lng: number;
  lat: number;
  count: number;
}
export interface HazardDot {
  lng: number;
  lat: number;
  color: Rgb; // amber = built/park, red = water
}
export interface DemandPoint {
  lng: number;
  lat: number;
  weight: number; // travel demand at this grid cell (origin+dest)
  served?: boolean; // within the catchment union of placed stations → faded; else unmet → glows
}

export interface RenderView {
  stations: StationDot[];
  lines: LinePath[];
  catchments: CatchmentCircle[];
  blueprint: [number, number][]; // in-progress line being drawn (T11)
  vehicles: VehicleDot[]; // moving trains (T15)
  waiting: WaitingDot[]; // accumulating waiting-passenger halos (T17)
  hazards: HazardDot[]; // live built/water conflict dots along the blueprint (G2)
  demand: DemandPoint[]; // travel-demand heat overlay (toggleable map layer)
  blueprintInvalid?: boolean; // in-progress route is illegal (e.g. land mode over water) → red ghost
  pinnedLabel?: { lng: number; lat: number; text: string }; // deck label for the pinned station
  selectedLine?: number | null; // drives the wide selection casing under the selected line
}

export function colorToRgb(u: number): Rgb {
  return [(u >> 16) & 0xff, (u >> 8) & 0xff, u & 0xff];
}

/** Heavy/high-speed rail mode id (crates/sim trainset::tmode::HEAVY) — gets mainline styling. */
const HEAVY_RAIL = 4;

/** Demand heat ramp. The primary channel is SERVED vs UNMET, not raw weight: unmet demand
 *  (no station in range) glows warm + solid — the gap to fill; served demand fades cool +
 *  translucent — you've got it covered. Alpha (faint↔solid) is the colour-blind-safe channel,
 *  with warm/cool hue as the secondary cue. Weight still modulates intensity. */
function demandColor(w: number, served?: boolean): [number, number, number, number] {
  const t = Math.max(0, Math.min(1, w / 5));
  if (served) return [90, 130, 170, Math.round(10 + t * 26)]; // cool + faint
  // unmet: warm + solid, intensity rising with demand weight
  const r = Math.round(120 + t * 120);
  const g = Math.round(72 + (1 - t) * 36);
  const b = Math.round(60 - t * 30);
  const a = Math.round(58 + t * 112);
  return [r, g, b, a];
}

/** Waiting-queue band: 0 = a few waiting (faint), 1 = BUSY (amber, watch), 2 = STARVED (red, fix).
 *  Single source for the ring colour/width + its updateTrigger, mirroring the loadPip language. */
function waitBand(count: number): 0 | 1 | 2 {
  if (count >= STARVED_WAITING) return 2;
  if (count >= BUSY_WAITING) return 1;
  return 0;
}
function waitRing(count: number): { color: [number, number, number, number]; width: number } {
  const band = waitBand(count);
  if (band === 2) return { color: [214, 40, 40, 235], width: 3.5 }; // starved — vermillion
  if (band === 1) return { color: [230, 159, 0, 225], width: 2 }; // busy — amber
  return { color: [230, 159, 0, 130], width: 1.5 }; // a few waiting — faint amber
}

/** Topology layers (rebuilt only on topology/selection change — cached by Game so they keep
 *  a stable identity across frames). Split into below/above the vehicle layer to preserve the
 *  z-order catchment<lines<blueprint<vehicles<stations while only vehicles update per frame. */
export function topoLayers(view: RenderView): { below: Layer[]; above: Layer[] } {
  const below: Layer[] = [
    // Travel-demand heat (bottom of the stack so the network draws over it). Soft blue→red
    // additive blobs sized by demand weight — a "where do people want to go" map layer.
    new ScatterplotLayer({
      id: "demand-heat",
      data: view.demand,
      getPosition: (d: DemandPoint) => [d.lng, d.lat],
      getRadius: (d: DemandPoint) => 120 + Math.sqrt(d.weight) * 120,
      radiusUnits: "meters",
      radiusMinPixels: 6,
      getFillColor: (d: DemandPoint) => demandColor(d.weight, d.served),
      stroked: false,
      // `demand` is a fresh array only when the served set is recomputed (topology/toggle), so
      // identity is stable across frames; this trigger guards the in-place served recolor.
      updateTriggers: { getFillColor: view.demand.map((d) => (d.served ? 1 : 0)).join("") },
    }),
    new ScatterplotLayer({
      id: "catchments",
      data: view.catchments,
      getPosition: (d: CatchmentCircle) => [d.lng, d.lat],
      getRadius: (d: CatchmentCircle) => d.radiusM,
      radiusUnits: "meters",
      // Pinned (selected) station = filled + solid stroke; hover peek = stroke-only, fainter, so a
      // peek reads as provisional and never greys out what's under it. The pinned fill alpha scales
      // with captured demand (28..96) so a station on heavy demand reads denser than one on empty
      // land — the "which stations actually grab riders" signal, surfaced where you're looking.
      getFillColor: (d: CatchmentCircle) =>
        d.peek ? [0, 114, 178, 0] : [0, 114, 178, Math.round(28 + Math.min(1, d.demand ?? 0) * 68)],
      stroked: true,
      getLineColor: (d: CatchmentCircle) => (d.peek ? [0, 114, 178, 110] : [0, 114, 178, 180]),
      lineWidthMinPixels: 1.5,
      updateTriggers: {
        getFillColor: view.catchments.map((c) => `${!!c.peek}:${Math.round((c.demand ?? 0) * 10)}`).join(","),
        getLineColor: view.catchments.map((c) => !!c.peek).join(","),
      },
    }),
    // Selected-line emphasis: a wide dark casing under the picked line so it pops on the muted
    // basemap regardless of hue (width + dark frame = colour-blind-safe, not a hue change). Wider
    // than the heavy-rail casing so it frames even mainline track. Bumps only on selection change.
    new PathLayer({
      id: "lines-selected-casing",
      data: view.selectedLine == null ? [] : view.lines.filter((d) => d.id === view.selectedLine),
      getPath: (d: LinePath) => d.path,
      getColor: [34, 34, 40, 220],
      getWidth: 15,
      widthUnits: "pixels",
      widthMinPixels: 11,
      capRounded: true,
      jointRounded: true,
      updateTriggers: { getColor: view.selectedLine ?? -1 },
    }),
    // Heavy / high-speed rail reads as MAINLINE track, not a flat metro stroke: a dark casing
    // under a wider colored core with a pale centre stripe (a "double-track" look). Only the
    // heavy lines are in these two extra layers; metro/bus/ferry/air stay in the flat "lines".
    new PathLayer({
      id: "lines-heavy-casing",
      data: view.lines.filter((d) => d.mode === HEAVY_RAIL),
      getPath: (d: LinePath) => d.path,
      getColor: [34, 34, 40, 255],
      getWidth: 13,
      widthUnits: "pixels",
      widthMinPixels: 9,
      capRounded: true,
      jointRounded: true,
    }),
    new PathLayer({
      id: "lines",
      data: view.lines,
      getPath: (d: LinePath) => d.path,
      getColor: (d: LinePath) => d.color,
      getWidth: (d: LinePath) => (d.mode === HEAVY_RAIL ? 8 : 6),
      widthUnits: "pixels",
      widthMinPixels: 4,
      capRounded: true,
      jointRounded: true,
      // Pickable so hovering the track raises the line inspector (under stations + trains in
      // z-order, so it only fires on bare track). The pick hit-area widens with pickingRadius.
      pickable: true,
    }),
    new PathLayer({
      id: "lines-heavy-centre",
      data: view.lines.filter((d) => d.mode === HEAVY_RAIL),
      getPath: (d: LinePath) => d.path,
      getColor: [245, 245, 250, 220],
      getWidth: 2,
      widthUnits: "pixels",
      widthMinPixels: 1,
      capRounded: true,
      jointRounded: true,
    }),
  ];

  if (view.blueprint.length > 1) {
    below.push(
      new PathLayer({
        id: "blueprint",
        data: [{ path: view.blueprint }],
        getPath: (d: { path: [number, number][] }) => d.path,
        // Provisional ghost: muted grey when valid, red when the route is illegal (NIMBY's
        // blue/red blueprint signal). updateTriggers so the colour flips with validity.
        getColor: view.blueprintInvalid ? [214, 40, 40, 220] : [120, 124, 130, 190],
        getWidth: 4,
        widthUnits: "pixels",
        widthMinPixels: 3,
        capRounded: true,
        jointRounded: true,
        updateTriggers: { getColor: view.blueprintInvalid ? 1 : 0 },
      }),
    );
  }

  const above: Layer[] = [
    new ScatterplotLayer({
      id: "stations",
      data: view.stations,
      getPosition: (d: StationDot) => [d.lng, d.lat],
      // Radius grows with cumulative boardings (sqrt, capped +6px) so the static dot field becomes
      // a usage heatmap — busy stations swell. Selected adds a bump on top.
      getRadius: (d: StationDot) => (d.selected ? 9 : 7) + Math.min(6, Math.sqrt(d.boardings) * 0.4),
      radiusUnits: "pixels",
      radiusMinPixels: 5,
      // Selected fill = selection blue (ties to its blue catchment ring). Otherwise an ORPHANED
      // station (no operational line serving it) is muted grey and a SERVED one is near-black, so
      // stations visibly "light up" as you connect + run them (place→draw→assign cause→effect).
      getFillColor: (d: StationDot) =>
        d.selected ? [0, 114, 178] : d.serving > 0 ? [28, 32, 36] : [120, 126, 134],
      stroked: true,
      getLineColor: [255, 255, 255],
      lineWidthMinPixels: 2,
      pickable: true,
      updateTriggers: {
        getFillColor: view.stations.map((s) => `${s.selected}:${s.serving > 0}`).join(","),
        getRadius: view.stations.map((s) => `${s.selected}:${Math.round(Math.sqrt(s.boardings))}`).join(","),
      },
    }),
    // Waiting-passenger halo: a ring that grows with the queue (top, so a starved station is always
    // visible). Stroked-only so it doesn't occlude the station dot. Three bands so "filling up"
    // reads BEFORE "starved": a faint thin ring under BUSY (a few people, fine), solid amber once
    // BUSY (watch this), thick vermillion once STARVED (fix the headway). updateTriggers on the
    // band membership (a string), never per frame.
    new ScatterplotLayer({
      id: "waiting",
      data: view.waiting,
      getPosition: (d: WaitingDot) => [d.lng, d.lat],
      getRadius: (d: WaitingDot) => 8 + Math.min(16, Math.sqrt(d.count) * 2.5),
      radiusUnits: "pixels",
      stroked: true,
      filled: false,
      getLineColor: (d: WaitingDot) => waitRing(d.count).color,
      getLineWidth: (d: WaitingDot) => waitRing(d.count).width,
      lineWidthUnits: "pixels",
      lineWidthMinPixels: 1.5,
      updateTriggers: {
        getRadius: view.waiting.map((w) => w.count).join(","),
        getLineColor: view.waiting.map((w) => waitBand(w.count)).join(","),
        getLineWidth: view.waiting.map((w) => waitBand(w.count)).join(","),
      },
    }),
    // Live build-conflict dots along the in-progress blueprint (amber built/park, red water).
    new ScatterplotLayer({
      id: "hazards",
      data: view.hazards,
      getPosition: (d: HazardDot) => [d.lng, d.lat],
      getRadius: 4,
      radiusUnits: "pixels",
      getFillColor: (d: HazardDot) => d.color,
      stroked: false,
    }),
    // Pinned-station label (deck geometry, NOT a DOM node anchored by lng/lat). One line at the
    // selected station; data length 0/1 so it costs nothing when nothing is pinned. characterSet
    // "auto" so names with non-ASCII glyphs render. updateTriggers on the label id/text only.
    new TextLayer<{ lng: number; lat: number; text: string }>({
      id: "station-label",
      data: view.pinnedLabel ? [view.pinnedLabel] : [],
      getPosition: (d) => [d.lng, d.lat],
      getText: (d) => d.text,
      characterSet: "auto",
      getSize: 12,
      sizeUnits: "pixels",
      getColor: [28, 32, 36, 255],
      getPixelOffset: [0, -16],
      fontWeight: 700,
      background: true,
      getBackgroundColor: [255, 255, 255, 235],
      backgroundPadding: [5, 3],
      getTextAnchor: "middle",
      getAlignmentBaseline: "bottom",
      updateTriggers: {
        getText: view.pinnedLabel?.text ?? "",
        getPosition: view.pinnedLabel ? `${view.pinnedLabel.lng},${view.pinnedLabel.lat}` : "",
      },
    }),
  ];

  return { below, above };
}

// A white triangle pointing +x (east) at angle 0, baked once to a data URL so the IconLayer can
// rotate it by heading. `mask:true` makes it a tintable stencil so getColor carries line identity.
function arrowIconUrl(): string {
  const s = 64;
  const c = document.createElement("canvas");
  c.width = s;
  c.height = s;
  const g = c.getContext("2d")!;
  g.fillStyle = "#fff";
  g.beginPath();
  g.moveTo(58, 32); // tip (east)
  g.lineTo(14, 13);
  g.lineTo(26, 32);
  g.lineTo(14, 51);
  g.closePath();
  g.fill();
  return c.toDataURL();
}
const ARROW_ICON = typeof document !== "undefined" ? arrowIconUrl() : "";
const ARROW_MAPPING = { arrow: { x: 0, y: 0, width: 64, height: 64, mask: true, anchorX: 32, anchorY: 32 } };

/** Crowding band for a moving train, mirroring loadPip / the waiting-ring language so "busy" and
 *  "crush" read the same colour wherever they appear. Outline only — the body keeps line identity. */
function loadRing(load: number): { color: [number, number, number, number]; width: number } {
  if (load >= 0.9) return { color: [214, 40, 40, 240], width: 2.5 }; // crush — vermillion
  if (load >= 0.6) return { color: [230, 159, 0, 230], width: 2 }; // busy — amber
  return { color: [255, 255, 255, 230], width: 1.5 }; // healthy — white
}

/** The per-frame vehicle layers (moving trains): a line-coloured body whose radius grows and
 *  whose outline shifts white→amber→red with load (identity + crowding, always visible against
 *  the same-coloured track via its contrasting stroke), with a small WHITE triangle on top
 *  rotated to the heading so you read which way each train is travelling. Both below stations so
 *  platforms stay clickable. Returned as an array spliced into the z-order between topo below/above. */
export function vehicleLayers(dots: VehicleDot[]): Layer[] {
  return [
    // Body: the crowding-aware dot. Line-colour fill = identity; radius + outline colour/width
    // track load (white healthy → amber busy → red crush, the loadPip/waiting-ring language).
    // Pickable, id "vehicles" so the train inspector (getTooltip dispatch on layer.id) still fires.
    new ScatterplotLayer({
      id: "vehicles",
      data: dots,
      getPosition: (d: VehicleDot) => [d.lng, d.lat],
      getRadius: (d: VehicleDot) => 7 + d.load * 3,
      radiusUnits: "pixels",
      radiusMinPixels: 6,
      getFillColor: (d: VehicleDot) => d.color,
      stroked: true,
      getLineColor: (d: VehicleDot) => loadRing(d.load).color,
      getLineWidth: (d: VehicleDot) => loadRing(d.load).width,
      lineWidthUnits: "pixels",
      lineWidthMinPixels: 1.5,
      pickable: true,
      updateTriggers: {
        getRadius: dots.map((d) => Math.round(d.load * 10)).join(","),
        getLineColor: dots.map((d) => loadRing(d.load).width).join(","),
        getLineWidth: dots.map((d) => loadRing(d.load).width).join(","),
      },
    }),
    // Direction: a small WHITE triangle rotated to the train's heading (deck getAngle is CCW
    // degrees; our heading is CCW radians from +x → straight conversion). White so it reads on the
    // line-coloured body; smaller than the dot so the identity colour still rings it. Not pickable.
    new IconLayer({
      id: "vehicle-dir",
      data: dots,
      getPosition: (d: VehicleDot) => [d.lng, d.lat],
      getIcon: () => "arrow",
      iconAtlas: ARROW_ICON,
      iconMapping: ARROW_MAPPING,
      getColor: [255, 255, 255, 235],
      getAngle: (d: VehicleDot) => (d.angle * 180) / Math.PI,
      getSize: (d: VehicleDot) => 9 + d.load * 3,
      sizeUnits: "pixels",
      sizeMinPixels: 7,
    }),
  ];
}
