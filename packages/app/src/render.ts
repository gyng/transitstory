// Builds the deck.gl overlay layers from a plain RenderView (already in lng/lat — all mm
// conversion happened at the geo.ts boundary in Game). Layer array order IS the z-order
// (AGENTS IA): catchment < lines < blueprint < stations < vehicles < selection highlight.
import type { Layer } from "@deck.gl/core";
import { PathLayer, ScatterplotLayer } from "@deck.gl/layers";

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
}
export interface CatchmentCircle {
  lng: number;
  lat: number;
  radiusM: number;
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

export interface RenderView {
  stations: StationDot[];
  lines: LinePath[];
  catchments: CatchmentCircle[];
  blueprint: [number, number][]; // in-progress line being drawn (T11)
  vehicles: VehicleDot[]; // moving trains (T15)
  waiting: WaitingDot[]; // accumulating waiting-passenger halos (T17)
}

export function colorToRgb(u: number): Rgb {
  return [(u >> 16) & 0xff, (u >> 8) & 0xff, u & 0xff];
}

/** Topology layers (rebuilt only on topology/selection change — cached by Game so they keep
 *  a stable identity across frames). Split into below/above the vehicle layer to preserve the
 *  z-order catchment<lines<blueprint<vehicles<stations while only vehicles update per frame. */
export function topoLayers(view: RenderView): { below: Layer[]; above: Layer[] } {
  const below: Layer[] = [
    new ScatterplotLayer({
      id: "catchments",
      data: view.catchments,
      getPosition: (d: CatchmentCircle) => [d.lng, d.lat],
      getRadius: (d: CatchmentCircle) => d.radiusM,
      radiusUnits: "meters",
      getFillColor: [0, 114, 178, 38],
      stroked: true,
      getLineColor: [0, 114, 178, 150],
      lineWidthMinPixels: 1.5,
    }),
    new PathLayer({
      id: "lines",
      data: view.lines,
      getPath: (d: LinePath) => d.path,
      getColor: (d: LinePath) => d.color,
      getWidth: 6,
      widthUnits: "pixels",
      widthMinPixels: 4,
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
        getColor: [120, 124, 130, 190],
        getWidth: 4,
        widthUnits: "pixels",
        widthMinPixels: 3,
        capRounded: true,
        jointRounded: true,
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
      getFillColor: (d: StationDot) => (d.selected ? [214, 94, 0] : [28, 32, 36]),
      stroked: true,
      getLineColor: [255, 255, 255],
      lineWidthMinPixels: 2,
      pickable: true,
      updateTriggers: {
        getFillColor: view.stations.map((s) => s.selected).join(","),
        getRadius: view.stations.map((s) => s.selected).join(","),
      },
    }),
    // Waiting-passenger halo: an amber ring that grows with the queue (top, so a starved
    // station is always visible). Stroked-only so it doesn't occlude the station dot.
    new ScatterplotLayer({
      id: "waiting",
      data: view.waiting,
      getPosition: (d: WaitingDot) => [d.lng, d.lat],
      getRadius: (d: WaitingDot) => 8 + Math.min(16, Math.sqrt(d.count) * 2.5),
      radiusUnits: "pixels",
      stroked: true,
      filled: false,
      getLineColor: [230, 159, 0, 220],
      lineWidthMinPixels: 2,
      updateTriggers: {
        getRadius: view.waiting.map((w) => w.count).join(","),
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
