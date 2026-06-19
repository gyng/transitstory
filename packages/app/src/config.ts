// App-wide constants. The Singapore origin anchors the local-metres frame used by
// coords/geo.ts (the one coordinate boundary). Tunables live here, not scattered.

/** Local-frame origin (also the initial map center). */
export const SG_ORIGIN = { lng: 103.8198, lat: 1.3521 } as const;
export const SG_CENTER: [number, number] = [SG_ORIGIN.lng, SG_ORIGIN.lat];
export const SG_ZOOM = 11;

/** Map level-of-detail threshold. At/above this zoom the overview reveals the "micro" overlays —
 *  individual rider peeps, per-station waiting halos, vehicle direction arrows, pinned labels. Below
 *  it (the city-overview default, SG_ZOOM=11) only the NETWORK shows — lines + vehicles + stations —
 *  so the map reads cleanly instead of as a swarm of flashing dots. Tune to taste. */
export const DETAIL_ZOOM = 12.5;

/** Fixed logical sim step (20 Hz). Render interpolates to 60fps; stats DOM refresh ~1-4 Hz. */
export const TICK_MS = 50;

/** Default catchment radius (metres) and station snap radius (screen pixels). */
export const CATCHMENT_M = 500;
export const SNAP_PX = 18;

/** Waiting-queue thresholds (passengers) for the station-inspect verdict + starvation ring.
 *  busy = a watch signal; starved = the ring turns vermillion (the fix is shorter headway /
 *  more capacity). Single source for both the tooltip verdict word and the on-map ring colour. */
/** CLOCK-FRAME RETUNE: with ~7-seat vehicles sweeping queues every 1-60 clock-minutes, queues
 *  peak far lower than the pre-unification 200-seat/3-clock-hour world — STARVED now means
 *  "more than a full trainload left on the platform", the same felt pressure. */
export const BUSY_WAITING = 3;
export const STARVED_WAITING = 7;

/** Prefix a `public/`-rooted asset path with Vite's deploy base, so committed data/title
 *  assets resolve under a project-pages base (`/transitstory/`) as well as at root (`/`).
 *  BASE_URL is "/" in dev/preview and "/transitstory/" in the GitHub Pages build. */
export function withBase(path: string): string {
  return import.meta.env.BASE_URL.replace(/\/+$/, "") + (path.startsWith("/") ? path : "/" + path);
}

/** Colour-blind-safe line palette (Okabe-Ito base); lines auto-assign the next entry on create.
 *  Two slots deliberately deviate from straight Okabe-Ito to keep LINE identity clear of the
 *  SEMANTIC alert hues: bluish-green 0x009e73 (= the "healthy" verdict) → Tol teal 0x44aa99, and
 *  amber 0xe69f00 (= the "busy" alert) → Tol wine 0x882255 — so a line's colour never reads as a
 *  health verdict at a glance. */
export const LINE_PALETTE: number[] = [
  0x0072b2, 0xd55e00, 0x44aa99, 0xcc79a7, 0x882255, 0x56b4e9, 0xf0e442, 0x000000,
];

/** ARCADIA line palette — the empire is the ONLY warm thing on the ash world, so every line stays in the
 *  warm half (slot 0 = warm, never the cold selection-blue). CB strategy for an all-warm ramp (the hardest
 *  case): 6 tones ordered by INTERLEAVED lightness (copper↔rust-brown↔orange↔rust↔ochre↔gold) so index-adjacent
 *  lines contrast in value AND hue-temperature, not red/green. Kept clear of capital-gold (0xebaf2d) so
 *  dominion-gold never reads as a player line. CRITICAL: also kept OUT of the rival THREAT-crimson band
 *  (rival rail [230,60,48], holds [228,52,44], hosts [190,55,55]) — the old oxblood/rust/brick slots
 *  (7a2e1f/9c3b1f/a83232) read as an enemy host at a glance; every slot now has G well above the crimson
 *  floor so a player line is never confusable with the enemy. Used by `nextLineColor`, arcadia ruleset only. */
export const ARCADIA_LINE_PALETTE: number[] = [
  0xd98c3a, 0x8a5a2b, 0xe07b2e, 0xb5651d, 0xc96a2c, 0xd9a441,
];
