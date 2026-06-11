// A real-world starting network (e.g. Singapore MRT). Stations are defined once; lines
// reference station indices, so a shared index is an interchange (enabling transfers).
// Applied via the normal Command path (Game.applyNetwork) — fully command-sourced.
export interface NetStation {
  name: string;
  lng: number;
  lat: number;
}

export interface NetLine {
  name: string;
  colorHex: string; // RRGGBB
  headwayMin: number;
  trains: number;
  loop?: boolean;
  mode?: number; // transport mode (0=rail,1=bus,2=ferry,3=air); default rail
  stations: number[]; // ordered indices into stations[]
  // Real OSM track alignment, per span: geometry[j] is the [lng,lat] vertices strictly between
  // station j and j+1 (the closing span too, for a loop). Present for real-world imports so the
  // line follows the actual layout (applied as literal waypoints); absent ⇒ straight spans.
  geometry?: [number, number][][];
}

export interface Network {
  cityId: string;
  name?: string;
  stations: NetStation[];
  lines: NetLine[];
}

import { withBase } from "../config";

export async function loadNetwork(path: string): Promise<Network> {
  const res = await fetch(withBase(path));
  if (!res.ok) throw new Error(`network fetch failed: ${path}`);
  return res.json();
}
