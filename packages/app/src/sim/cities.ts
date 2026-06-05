// Registry of selectable cities (the game menu reads this). Each manifest is a committed
// CityData JSON; demand grids + networks are referenced from within it.
export interface CityEntry {
  id: string;
  name: string;
  blurb: string;
  manifest: string;
}

export const CITIES: CityEntry[] = [
  { id: "globe", name: "World ✈", blurb: "Global airline — connect cities by air", manifest: "/data/globe_city.json" },
  { id: "singapore", name: "Singapore", blurb: "MRT — dense island metro", manifest: "/data/singapore_city.json" },
  { id: "tokyo", name: "Tokyo", blurb: "JR + subway — the big one", manifest: "/data/tokyo_city.json" },
  { id: "calgary", name: "Calgary", blurb: "C-Train — LRT + free-fare downtown", manifest: "/data/calgary_city.json" },
  { id: "istanbul", name: "Istanbul", blurb: "Bosphorus — two continents, ferry country", manifest: "/data/istanbul_city.json" },
  { id: "manhattan", name: "New York", blurb: "Manhattan — a dense linear island", manifest: "/data/manhattan_city.json" },
  { id: "dublin", name: "Dublin", blurb: "Liffey city — a gentle starter", manifest: "/data/dublin_city.json" },
  { id: "chicago", name: "Chicago", blurb: "The 'L' loop + lakefront grid", manifest: "/data/chicago_city.json" },
  { id: "sf", name: "San Francisco", blurb: "Bay peninsula + transbay BART", manifest: "/data/sf_city.json" },
  { id: "brisbane", name: "Brisbane", blurb: "Subtropical river city", manifest: "/data/brisbane_city.json" },
  { id: "london", name: "London", blurb: "The Tube — radial across the Thames", manifest: "/data/london_city.json" },
  { id: "pyongyang", name: "Pyongyang", blurb: "Deep metro on the Taedong", manifest: "/data/pyongyang_city.json" },
  { id: "glasgow", name: "Glasgow", blurb: "Clockwork-Orange loop on the Clyde", manifest: "/data/glasgow_city.json" },
];

export function cityById(id: string | null): CityEntry {
  return CITIES.find((c) => c.id === id) ?? CITIES[0];
}
