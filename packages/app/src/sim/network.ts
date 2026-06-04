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
  stations: number[]; // ordered indices into stations[]
}

export interface Network {
  cityId: string;
  name?: string;
  stations: NetStation[];
  lines: NetLine[];
}

export async function loadNetwork(path: string): Promise<Network> {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`network fetch failed: ${path}`);
  return res.json();
}
