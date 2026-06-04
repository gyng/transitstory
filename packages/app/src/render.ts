// Builds the deck.gl overlay layers from a plain RenderView (already in lng/lat — all mm
// conversion happened at the geo.ts boundary in Game). Layer array order IS the z-order
// (AGENTS IA): catchment < lines < blueprint < stations < vehicles < selection highlight.
import type { Layer } from "@deck.gl/core";
import { PathLayer, ScatterplotLayer, TextLayer } from "@deck.gl/layers";
import { STARVED_WAITING } from "./config";

export type Rgb = [number, number, number];

export interface StationDot {
  id: number;
  lng: number;
  lat: number;
  name: string;
  selected: boolean;
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
}
export interface VehicleDot {
  lng: number;
  lat: number;
  color: Rgb;
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
      // Pinned (selected) station = filled + solid stroke; hover peek = stroke-only, fainter,
      // so a peek reads as provisional and never greys out what's under it.
      getFillColor: (d: CatchmentCircle) => (d.peek ? [0, 114, 178, 0] : [0, 114, 178, 38]),
      stroked: true,
      getLineColor: (d: CatchmentCircle) => (d.peek ? [0, 114, 178, 110] : [0, 114, 178, 170]),
      lineWidthMinPixels: 1.5,
      updateTriggers: {
        getFillColor: view.catchments.map((c) => !!c.peek).join(","),
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
      getRadius: (d: StationDot) => (d.selected ? 9 : 7),
      radiusUnits: "pixels",
      radiusMinPixels: 5,
      // Selected fill = selection blue (ties to its blue catchment ring); deliberately NOT the
      // old [214,94,0] orange, which collided with the Bus identity colour + gauge-bad. The
      // radius bump + white stroke are the colour-blind-safe channels.
      getFillColor: (d: StationDot) => (d.selected ? [0, 114, 178] : [28, 32, 36]),
      stroked: true,
      getLineColor: [255, 255, 255],
      lineWidthMinPixels: 2,
      pickable: true,
      updateTriggers: {
        getFillColor: view.stations.map((s) => s.selected).join(","),
        getRadius: view.stations.map((s) => s.selected).join(","),
      },
    }),
    // Waiting-passenger halo: a ring that grows with the queue (top, so a starved station is
    // always visible). Stroked-only so it doesn't occlude the station dot. Amber while merely
    // busy; flips to thick vermillion once the queue is STARVED — pointing at the headway fix.
    // updateTriggers on the starved-id SET (a membership string), never per frame.
    new ScatterplotLayer({
      id: "waiting",
      data: view.waiting,
      getPosition: (d: WaitingDot) => [d.lng, d.lat],
      getRadius: (d: WaitingDot) => 8 + Math.min(16, Math.sqrt(d.count) * 2.5),
      radiusUnits: "pixels",
      stroked: true,
      filled: false,
      getLineColor: (d: WaitingDot) =>
        d.count >= STARVED_WAITING ? [214, 40, 40, 235] : [230, 159, 0, 220],
      getLineWidth: (d: WaitingDot) => (d.count >= STARVED_WAITING ? 3.5 : 2),
      lineWidthUnits: "pixels",
      lineWidthMinPixels: 2,
      updateTriggers: {
        getRadius: view.waiting.map((w) => w.count).join(","),
        getLineColor: view.waiting.map((w) => w.count >= STARVED_WAITING).join(","),
        getLineWidth: view.waiting.map((w) => w.count >= STARVED_WAITING).join(","),
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

/** The per-frame vehicle layer (moving trains). Below stations so platforms stay clickable. */
export function vehicleLayer(dots: VehicleDot[]): Layer {
  return new ScatterplotLayer({
    id: "vehicles",
    data: dots,
    getPosition: (d: VehicleDot) => [d.lng, d.lat],
    getRadius: 5,
    radiusUnits: "pixels",
    radiusMinPixels: 4,
    getFillColor: (d: VehicleDot) => d.color,
    stroked: true,
    getLineColor: [255, 255, 255, 230],
    lineWidthMinPixels: 1.5,
  });
}
