// deck.gl overlay (overlaid mode, interleaved:false — PLAN §map-rendering). MapLibre owns
// the camera/input; deck draws the transit overlays into its own synced canvas. Layers are
// built from a RenderState and pushed via overlay.setProps with STABLE data identity;
// updateTriggers bump only on topology change — NEVER rebuild layers per frame (T15).
import { MapboxOverlay } from "@deck.gl/mapbox";
import { ScatterplotLayer } from "@deck.gl/layers";
import { metersToLngLat } from "../coords/geo";

export function createOverlay(): MapboxOverlay {
  return new MapboxOverlay({ interleaved: false });
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
