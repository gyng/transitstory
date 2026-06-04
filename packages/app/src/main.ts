// App entry. Boots the MapLibre map, loads the committed city (manifest + demand grid),
// constructs the SimBridge, and exposes a debug/test handle on window. Tools, overlay, and
// the game loop attach in T6/T10+.
import "./styles.css";
import { createMap } from "./map/basemap";
import { createOverlay, testMarkerLayer } from "./map/overlay";
import { loadCity } from "./sim/city";
import { SimBridge } from "./sim/SimBridge";

function mountTitle(): void {
  const el = document.createElement("div");
  el.id = "app-title";
  el.textContent = "onlytransits";
  el.style.cssText =
    "position:fixed;top:10px;left:14px;margin:0;padding:4px 10px;border-radius:8px;" +
    "background:rgba(255,255,255,.85);font:600 15px system-ui,sans-serif;color:#1c2024;" +
    "box-shadow:0 2px 10px rgba(0,0,0,.12);z-index:10";
  document.getElementById("ui")?.appendChild(el);
}

async function boot(): Promise<void> {
  mountTitle();
  const map = createMap("map");
  const overlay = createOverlay();
  map.addControl(overlay);
  overlay.setProps({ layers: [testMarkerLayer()] });

  const city = await loadCity();
  const bridge = new SimBridge(city.seed, city.coreCityJson);

  window.__ot = { map, bridge, city, overlay };
  window.__APP_READY = true;
}

void boot();
