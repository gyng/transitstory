// deck.gl overlay (overlaid mode, interleaved:false — PLAN §map-rendering). MapLibre owns
// the camera/input; deck draws the transit overlays into its own synced canvas. Layers are
// built from a RenderState and pushed via overlay.setProps with STABLE data identity;
// updateTriggers bump only on topology change — NEVER rebuild layers per frame (T15).
import { MapboxOverlay } from "@deck.gl/mapbox";
import { AmbientLight, DirectionalLight, LightingEffect, PostProcessEffect, type Effect } from "@deck.gl/core";
import { ScatterplotLayer } from "@deck.gl/layers";
import { metersToLngLat } from "../coords/geo";
import { acesfx, ACESFX_PROPS } from "./postfx";

// One scene LightingEffect for the 3D meshes (trains / wagons / cargo lumps / legions / trees). With
// NO LightingEffect deck applies a flat, camera-relative DEFAULT light, so the low-poly bodies read
// shadeless and their facets swim as the camera turns. A fixed ambient + a single warm directional
// "sun" gives every mesh consistent directional FORM (sloped prows, load-lump height, banner folds)
// that stays put under pan/zoom — most visible in arcadia (tilted to pitch 45) but a quiet upgrade
// top-down too. Built ONCE and held in the overlay's effects array (STABLE identity — deck never
// tears down/rebuilds the light, per the render-hot-path rule); the per-layer `material` objects
// already feed phong, so the meshes respond with no per-layer change.
// NOTE on shadows: deck's experimental DirectionalLight `_shadow` was tried (arcadia only, where the
// ground is a deck ColumnLayer that could receive). In `interleaved:false` overlay mode it renders
// severe white shadow-map artifacts across the terrain — the documented hazard. Real cast shadows would
// require switching the overlay to interleaved mode (a large, risky z-order/blend rework), so they're
// deferred as NOT cost-effective. The directional sun below already gives the meshes their form.
/** The overlay plus the light handles the day/night cycle mutates on the ~3 Hz sim-hour slice. */
export interface OverlayHandles {
  overlay: MapboxOverlay;
  sun: DirectionalLight;
  ambient: AmbientLight;
}

export function createOverlay(arcadia = false): OverlayHandles {
  // The light objects are returned so the day/night cycle can mutate their intensity/colour/direction
  // in place (same instances, stable effects array).
  const ambient = new AmbientLight({ color: [255, 255, 255], intensity: 1.05 });
  const sun = new DirectionalLight({
    color: [255, 247, 230], // a touch of warmth so lit faces read sunlit, not clinical
    intensity: 1.35,
    direction: [-0.6, -1, -0.85], // rakes down from the upper-right; stable (dragRotate is off)
  });
  const effects: Effect[] = [new LightingEffect({ ambient, sun })];
  if (arcadia) {
    // ACES filmic tone-map + a whisper of film grain + a gentle vignette: ONE fullscreen pass over the
    // whole (deck-drawn) arcadia scene. Transit's ground is MapLibre tiles the overlay can't touch, so
    // there it'd only tint the trains/lines — scope it to arcadia where it covers everything.
    effects.push(new PostProcessEffect(acesfx, ACESFX_PROPS));
  }
  const overlay = new MapboxOverlay({ interleaved: false, effects });
  return { overlay, sun, ambient };
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const mix3 = (a: number[], b: number[], t: number): [number, number, number] => [
  lerp(a[0], b[0], t),
  lerp(a[1], b[1], t),
  lerp(a[2], b[2], t),
];

/** Drive the scene sun + ambient by sim hour (0..24): a bright warm midday → a dim cool night, with
 *  amber dawn/dusk ramps. MUTATES the light objects in place (deck reads them per render via
 *  getProjectedLight), so the same LightingEffect instance stays in the overlay's stable effects array.
 *  Returns the 0..1 NIGHT factor (0 = full day, 1 = deep night) that fades in the warm town/train glows.
 *  Ride the ~3 Hz sim-hour slice (like sky.ts) — NEVER the rAF loop. */
export function setLightingHour(sun: DirectionalLight, ambient: AmbientLight, hour: number): number {
  // daylight 0..1, dawn ramp 5→8, dusk ramp 17→20
  let day: number;
  if (hour < 5 || hour >= 20) day = 0;
  else if (hour < 8) day = (hour - 5) / 3;
  else if (hour < 17) day = 1;
  else day = 1 - (hour - 17) / 3;
  const ember = 4 * day * (1 - day); // peaks at the dawn/dusk ramp midpoints, 0 at noon + night

  const sunCol = mix3([120, 150, 220], [255, 247, 230], day); // cool night → warm day
  sun.color = mix3(sunCol, [255, 170, 110], ember * 0.5); // amber kiss at dawn/dusk
  sun.intensity = lerp(0.18, 1.4, day);
  sun.direction = [-0.6, -1, -(0.35 + 0.65 * day)]; // raking at dawn/dusk, steep at noon
  ambient.color = mix3([130, 150, 195], [255, 255, 255], day);
  ambient.intensity = lerp(0.5, 1.05, day);
  return 1 - day;
}

// T6 sanity marker at the local origin (0,0 m -> Singapore origin lng/lat). Confirms the
// overlay renders and stays anchored to the map on pan/zoom. Replaced by real layers in T10+.
export function testMarkerLayer() {
  const [lng, lat] = metersToLngLat([0, 0]);
  return new ScatterplotLayer({
    id: "test-marker",
    data: [{ position: [lng, lat] as [number, number] }],
    getPosition: (d: { position: [number, number] }) => d.position,
    getRadius: 300,
    radiusUnits: "meters",
    getFillColor: [0, 114, 178, 220],
    stroked: true,
    getLineColor: [255, 255, 255, 255],
    lineWidthMinPixels: 2,
    pickable: true,
  });
}
