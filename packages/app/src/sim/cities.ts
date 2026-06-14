// Registry of selectable cities (the game menu reads this). Each manifest is a committed
// CityData JSON; demand grids + networks are referenced from within it.
export interface CityEntry {
  id: string;
  name: string;
  blurb: string;
  manifest: string;
  /** Which ruleset/world the engine constructs — the frontend mirror of `CityData.ruleset`
   *  (fantasy-fork.md). Absent ⇒ `"transit"` (every shipped city). `"arcadia"` selects the hex
   *  4X-logistics fantasy campaign once its world is baked (S6). The menu groups by this so the
   *  fantasy campaign reads as a distinct mode, not just another city in the grid. */
  kind?: "transit" | "arcadia";
  /** The coverage score (0–100) the city's REAL network earns at load — the score-chase anchor
   *  ("beat the Tube"). Measured in-browser against the committed network + demand data (load
   *  `?city=<id>&network=1`, read `stats().coverageScore`); re-measure if the coverage formula or
   *  a city's data changes. Last measured 2026-06-11 (post clock-unification: the friendlier headway-quality span lifted most anchors by 1-3; the globe's loaded air headways moved it to 64). */
  realScore: number;
}

// Singapore is first, so it is the default selection (the menu + cityById both seed from CITIES[0]).
// The globe air board is last — it's the odd-one-out mode, not the default city.
export const CITIES: CityEntry[] = [
  { id: "singapore", name: "Singapore", blurb: "MRT — dense island metro", manifest: "/data/singapore_city.json", realScore: 41 },
  { id: "tokyo", name: "Tokyo", blurb: "JR + subway — the big one", manifest: "/data/tokyo_city.json", realScore: 39 },
  { id: "calgary", name: "Calgary", blurb: "C-Train — LRT + free-fare downtown", manifest: "/data/calgary_city.json", realScore: 14 },
  { id: "istanbul", name: "Istanbul", blurb: "Bosphorus — two continents, ferry country", manifest: "/data/istanbul_city.json", realScore: 47 },
  { id: "manhattan", name: "New York", blurb: "Manhattan — a dense linear island", manifest: "/data/manhattan_city.json", realScore: 24 },
  { id: "dublin", name: "Dublin", blurb: "Liffey city — a gentle starter", manifest: "/data/dublin_city.json", realScore: 41 },
  { id: "chicago", name: "Chicago", blurb: "The 'L' loop + lakefront grid", manifest: "/data/chicago_city.json", realScore: 32 },
  { id: "sf", name: "San Francisco", blurb: "Bay peninsula + transbay BART", manifest: "/data/sf_city.json", realScore: 48 },
  { id: "brisbane", name: "Brisbane", blurb: "Subtropical river city", manifest: "/data/brisbane_city.json", realScore: 32 },
  { id: "london", name: "London", blurb: "The Tube — radial across the Thames", manifest: "/data/london_city.json", realScore: 47 },
  { id: "pyongyang", name: "Pyongyang", blurb: "Deep metro on the Taedong", manifest: "/data/pyongyang_city.json", realScore: 38 },
  { id: "glasgow", name: "Glasgow", blurb: "Clockwork-Orange loop on the Clyde", manifest: "/data/glasgow_city.json", realScore: 41 },
  { id: "globe", name: "World ✈", blurb: "Global airline — connect cities by air", manifest: "/data/globe_city.json", realScore: 64 },
  // The fantasy 4X-logistics campaign (the fork). Hex lattice + supply chain + conquest + decadence.
  { id: "arcadia", name: "Arcadia ⚔", blurb: "Against the Dark — supply, conquer, hold the rot", manifest: "/data/arcadia_world.json", realScore: 0, kind: "arcadia" },
  // The procedurally-baked fantasy continent (scripts/build_world.py, seed 7). Terrain only for now —
  // S2/S3 add resources + towns; S6 the solvability validator. The hand-authored Arcadia above stays
  // the playable demo until the baked world grows its supply graph.
  { id: "fantasy", name: "Arcadia ⚔ (baked)", blurb: "A procedurally-forged, certified-winnable continent", manifest: "/data/fantasy_world.json", realScore: 0, kind: "arcadia" },
];

export function cityById(id: string | null): CityEntry {
  return CITIES.find((c) => c.id === id) ?? CITIES[0];
}

/** Personal best (max coverage in a from-scratch run) per city — localStorage-backed. Runs that
 *  load the real network don't count (starting at the anchor isn't beating it). */
const BEST_KEY = (cityId: string) => `ot-best:${cityId}`;

export function personalBest(cityId: string): number | null {
  try {
    const v = localStorage.getItem(BEST_KEY(cityId));
    return v === null ? null : Number(v) || 0;
  } catch {
    return null; // storage unavailable (private mode etc.) — bests just don't persist
  }
}

/** Record a new best if `score` beats the stored one; returns true when it did. */
export function recordBest(cityId: string, score: number): boolean {
  try {
    const cur = personalBest(cityId) ?? -1;
    if (score > cur) {
      localStorage.setItem(BEST_KEY(cityId), String(Math.round(score)));
      return true;
    }
  } catch {
    /* storage unavailable — silently skip */
  }
  return false;
}
