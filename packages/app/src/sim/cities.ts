// Registry of selectable cities (the game menu reads this). Each manifest is a committed
// CityData JSON; demand grids + networks are referenced from within it.
export interface CityEntry {
  id: string;
  name: string;
  blurb: string;
  manifest: string;
}

export const CITIES: CityEntry[] = [
  { id: "singapore", name: "Singapore", blurb: "MRT — dense island metro", manifest: "/data/singapore_city.json" },
  { id: "tokyo", name: "Tokyo", blurb: "JR + subway — the big one", manifest: "/data/tokyo_city.json" },
  { id: "calgary", name: "Calgary", blurb: "C-Train — LRT + free-fare downtown", manifest: "/data/calgary_city.json" },
];

export function cityById(id: string | null): CityEntry {
  return CITIES.find((c) => c.id === id) ?? CITIES[0];
}
