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

export interface RenderView {
  stations: StationDot[];
  lines: LinePath[];
  catchments: CatchmentCircle[];
  blueprint: [number, number][]; // in-progress line being drawn (T11)
  vehicles: VehicleDot[]; // moving trains (T15)
}

export function colorToRgb(u: number): Rgb {
  return [(u >> 16) & 0xff, (u >> 8) & 0xff, u & 0xff];
}

export function buildOverlayLayers(view: RenderView): Layer[] {
  const layers: Layer[] = [];

  // Catchment circles (bottom) — real metres; only selected/hovered shown (capped by Game).
  layers.push(
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
  );

  // Committed lines — constant pixel width so they stay legible at every zoom.
  layers.push(
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
  );

  // In-progress blueprint (translucent grey, distinct from committed full-colour lines).
  if (view.blueprint.length > 1) {
    layers.push(
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

  // Vehicles (moving trains) — below stations so platforms stay clickable.
  layers.push(
    new ScatterplotLayer({
      id: "vehicles",
      data: view.vehicles,
      getPosition: (d: VehicleDot) => [d.lng, d.lat],
      getRadius: 5,
      radiusUnits: "pixels",
      radiusMinPixels: 4,
      getFillColor: (d: VehicleDot) => d.color,
      stroked: true,
      getLineColor: [255, 255, 255, 230],
      lineWidthMinPixels: 1.5,
    }),
  );

  // Stations (top, pickable). Selected station gets the accent colour + larger radius.
  layers.push(
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
  );

  return layers;
}
