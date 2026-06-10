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
