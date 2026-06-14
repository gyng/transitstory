// A real-world starting network (e.g. Singapore MRT). Stations are defined once; lines
// reference station indices, so a shared index is an interchange (enabling transfers).
// Applied via the normal Command path (Game.applyNetwork) — fully command-sourced.
export interface NetStation {
  name: string;
  lng: number;
  lat: number;
  /** Fantasy/arcadia: place this node as a BARRACKS (fields AI legions) instead of a plain station. */
  barracks?: boolean;
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
  // Branches off the trunk (P3): each diverges at trunk stop `divergeAt` and continues through
  // `stations`. Recovered from OSM route variants (e.g. the Circle Line's Marina Bay spur).
  // `geometry[j]` is the spur's own [lng,lat] vertices for its j-th span (junction→stop0 is span 0).
  branches?: { divergeAt: number; stations: number[]; geometry?: [number, number][][] }[];
}

export interface Network {
  cityId: string;
  name?: string;
  stations: NetStation[];
  lines: NetLine[];
}

import { withBase } from "../config";
import { mmToLngLat } from "../coords/geo";
import type { RawCity } from "./city";

export async function loadNetwork(path: string): Promise<Network> {
  const res = await fetch(withBase(path));
  if (!res.ok) throw new Error(`network fetch failed: ${path}`);
  return res.json();
}

/** Synthesize the starting network for a baked fantasy world from its supply graph: every resource is a
 *  SOURCE station + every town a SINK station (the baked demand grid gives them the origin/dest weight the
 *  arcadia ruleset reads to assign those roles), and the capital is a BARRACKS (fields legions). NO lines —
 *  the resource/town nodes are fixed map features; the player draws the rail connecting the chains. Applied
 *  via the normal Game.applyNetwork command path (fully command-sourced). mm → lng/lat via the one
 *  coords/geo.ts boundary; the session origin must already be set (loadCity does so before this runs). */
export function networkFromSupplyGraph(sg: NonNullable<RawCity["supplyGraph"]>): Network {
  const stations: NetStation[] = [];
  for (const t of sg.towns ?? []) {
    const [lng, lat] = mmToLngLat([t.xMm, t.yMm]);
    const name = t.kind === "capital" ? "The Capital" : t.kind === "starter" ? "Hearthhold" : "Town";
    stations.push({ name, lng, lat, barracks: t.kind === "capital" });
  }
  sg.resources.forEach((r, i) => {
    const [lng, lat] = mmToLngLat([r.xMm, r.yMm]);
    stations.push({ name: `${r.kind[0].toUpperCase()}${r.kind.slice(1)} ${i + 1}`, lng, lat });
  });
  return { cityId: "fantasy", name: "Arcadia", stations, lines: [] };
}
