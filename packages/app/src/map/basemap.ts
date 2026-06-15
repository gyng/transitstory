// MapLibre basemap. A MUTED basemap (CARTO Positron — low-contrast, OSM-derived) so the
// transit overlays own the visual energy (AGENTS IA: figure-ground). OSM attribution is
// mounted from the first map commit (release gate, not polish). PMTiles self-hosting is the
// deferred T7 upgrade; the hosted style keeps the basemap off the critical path.
import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { SG_CENTER, SG_ZOOM } from "../config";

/** CARTO Positron (free, no API key): muted greyscale, © OpenStreetMap contributors. */
export const POSITRON_STYLE =
  "https://basemaps.cartocdn.com/gl/positron-gl-style/style.json";
/** Fully self-contained fallback if the hosted style is unreachable. */
export const MAPLIBRE_DEMO_STYLE = "https://demotiles.maplibre.org/style.json";

export function createMap(
  container: string | HTMLElement,
  center: [number, number] = SG_CENTER,
  zoom: number = SG_ZOOM,
): maplibregl.Map {
  const map = new maplibregl.Map({
    container,
    style: POSITRON_STYLE,
    center,
    zoom,
    attributionControl: false,
    dragRotate: false,
    pitchWithRotate: false,
  });

  // The hosted CARTO style already declares "© CARTO, © OpenStreetMap contributors" on its
  // sources — adding a customAttribution on top rendered the same credit twice. The control
  // (the ODbL release gate) stays mounted; the text comes from the style. The demo fallback
  // style carries its own attribution too.
  map.addControl(new maplibregl.AttributionControl({ compact: false }), "bottom-right");
  map.addControl(new maplibregl.NavigationControl({ showCompass: false }), "top-right");

  // Deterministic readiness signal for e2e (waits on this, not a sleep).
  map.on("idle", () => {
    window.__MAP_READY = true;
  });

  return map;
}

/** Repaint the (real-world) Positron basemap into a DEAD ASH-GREY void for the fantasy/arcadia continent —
 *  so the baked terrain island is the only inhabited ground and figure-ground holds (the locked vision: "a
 *  dead grey world ... muted ash-grey hex vellum"). Off-continent the world must read DEADER than the empire,
 *  not brighter (the default near-white Positron inverts that). Overrides the loaded style's paint in place
 *  (no style-swap → no network risk on the critical path): a dead-vellum background (always-settable
 *  fallback), dead water/land fills, hidden labels, near-invisible road/boundary lines. Hosted CARTO layer
 *  ids can drift, so every override is guarded; the background fallback alone guarantees no white sheet.
 *  Idempotent + style-reload safe via the `styledata` retry. Call once in boot for arcadia cities only. */
const DEAD_VELLUM = "#2b2d31"; // darker than terrain PLAIN [128,128,124] and WATER [34,40,52]
export function applyArcadiaBasemap(map: maplibregl.Map): void {
  let done = false;
  const apply = () => {
    if (done) return;
    try {
      map.setPaintProperty("background", "background-color", DEAD_VELLUM);
    } catch {
      /* style may have no 'background' layer — the fills below still dead the canvas */
    }
    const layers = map.getStyle()?.layers;
    if (!layers) return; // style not loaded yet — a later styledata fires this again
    for (const layer of layers) {
      const id = layer.id;
      try {
        if (layer.type === "symbol") {
          map.setLayoutProperty(id, "visibility", "none"); // basemap place labels off (deck TextLayer owns text)
        } else if (layer.type === "fill" && /water|ocean|sea|river|lake|bay/i.test(id)) {
          map.setPaintProperty(id, "fill-color", "#26282c");
        } else if (layer.type === "fill") {
          map.setPaintProperty(id, "fill-color", "#2f3135");
          map.setPaintProperty(id, "fill-opacity", 1);
        } else if (layer.type === "line") {
          map.setPaintProperty(id, "line-color", "#303236");
          map.setPaintProperty(id, "line-opacity", 0.25); // roads/borders barely there — the dead world stays dead
        }
      } catch {
        /* hosted-style layer id drifted — skip this one, the background fallback already holds */
      }
    }
    done = true;
    map.off("styledata", apply);
  };
  map.on("styledata", apply);
  apply(); // apply now if the style is already loaded; otherwise the styledata retry catches it
}
