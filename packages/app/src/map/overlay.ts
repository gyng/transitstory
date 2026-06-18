// deck.gl overlay (overlaid mode, interleaved:false — PLAN §map-rendering). MapLibre owns
// the camera/input; deck draws the transit overlays into its own synced canvas. Layers are
// built from a RenderState and pushed via overlay.setProps with STABLE data identity;
// updateTriggers bump only on topology change — NEVER rebuild layers per frame (T15).
import { MapboxOverlay } from "@deck.gl/mapbox";
import { AmbientLight, DirectionalLight, LightingEffect } from "@deck.gl/core";
import { ScatterplotLayer } from "@deck.gl/layers";
import { metersToLngLat } from "../coords/geo";

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
export function createSunLight(): { effect: LightingEffect; sun: DirectionalLight } {
  const ambient = new AmbientLight({ color: [255, 255, 255], intensity: 1.05 });
  const sun = new DirectionalLight({
    color: [255, 247, 230], // a touch of warmth so lit faces read sunlit, not clinical
    intensity: 1.35,
    direction: [-0.6, -1, -0.85], // rakes down from the upper-right; stable (dragRotate is off)
  });
  return { effect: new LightingEffect({ ambient, sun }), sun };
}

export function createOverlay(): MapboxOverlay {
  // Attach the sun at construction so its effects array keeps a stable reference for the overlay's
  // lifetime (never rebuilt in the per-frame setProps({layers}) path).
  const { effect } = createSunLight();
  return new MapboxOverlay({ interleaved: false, effects: [effect] });
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
