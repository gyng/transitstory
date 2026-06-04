// App-wide constants. The Singapore origin anchors the local-metres frame used by
// coords/geo.ts (the one coordinate boundary). Tunables live here, not scattered.

/** Local-frame origin (also the initial map center). */
export const SG_ORIGIN = { lng: 103.8198, lat: 1.3521 } as const;
export const SG_CENTER: [number, number] = [SG_ORIGIN.lng, SG_ORIGIN.lat];
export const SG_ZOOM = 11;

/** Fixed logical sim step (20 Hz). Render interpolates to 60fps; stats DOM refresh ~1-4 Hz. */
export const TICK_MS = 50;

/** Default catchment radius (metres) and station snap radius (screen pixels). */
export const CATCHMENT_M = 500;
export const SNAP_PX = 18;

/** Waiting-queue thresholds (passengers) for the station-inspect verdict + starvation ring.
 *  busy = a watch signal; starved = the ring turns vermillion (the fix is shorter headway /
 *  more capacity). Single source for both the tooltip verdict word and the on-map ring colour. */
export const BUSY_WAITING = 4;
export const STARVED_WAITING = 12;

/** Prefix a `public/`-rooted asset path with Vite's deploy base, so committed data/title
 *  assets resolve under a project-pages base (`/transitstory/`) as well as at root (`/`).
 *  BASE_URL is "/" in dev/preview and "/transitstory/" in the GitHub Pages build. */
export function withBase(path: string): string {
  return import.meta.env.BASE_URL.replace(/\/+$/, "") + (path.startsWith("/") ? path : "/" + path);
}

/** Colour-blind-safe (Okabe-Ito) line palette; lines auto-assign the next entry on create. */
export const LINE_PALETTE: number[] = [
  0x0072b2, 0xd55e00, 0x009e73, 0xcc79a7, 0xe69f00, 0x56b4e9, 0xf0e442, 0x000000,
];
