// App entry. Shows the start menu (pick city + mode), then boots that city: map, sim, tools,
// UI, optional real network. Deep-link / e2e: `?city=<id>&network=0|1` skips the menu.
import "./styles.css";
import { createMap } from "./map/basemap";
import { createOverlay } from "./map/overlay";
import { loadCity } from "./sim/city";
import { loadNetwork } from "./sim/network";
import { cityById, type CityEntry } from "./sim/cities";
import { SimBridge } from "./sim/SimBridge";
import { Buildability } from "./sim/buildability";
import { Game } from "./game";
import { GameLoop } from "./sim/GameLoop";
import { attachPointer } from "./tools/pointer";
import { mountToolbar } from "./ui/toolbar";
import { mountPanels } from "./ui/panels";
import { mountStatsBar } from "./ui/statsbar";
import { showMenu } from "./ui/menu";
import { installTestHooks } from "./testhooks";

function mountTitle(name: string): void {
  const el = document.createElement("div");
  el.id = "app-title";
  el.textContent = `onlytransits · ${name}`;
  el.style.cssText =
    "position:fixed;top:10px;left:14px;margin:0;padding:4px 10px;border-radius:8px;" +
    "background:rgba(255,255,255,.85);font:600 14px system-ui,sans-serif;color:#1c2024;" +
    "box-shadow:0 2px 10px rgba(0,0,0,.12);z-index:10";
  document.getElementById("ui")?.appendChild(el);
}

async function boot(manifestPath: string, withNetwork: boolean): Promise<void> {
  const city = await loadCity(manifestPath); // sets the session coordinate origin
  mountTitle(city.raw.name);

  const map = createMap("map", city.raw.center, city.raw.zoom);
  const overlay = createOverlay();
  map.addControl(overlay);

  const bridge = new SimBridge(city.seed, city.coreCityJson);
  const game = new Game(bridge, map, overlay, new Buildability(city.buildability));
  game.demandHeat = city.demandHeat; // travel-demand heat overlay source
  const loop = new GameLoop(game);
  attachPointer(game);
  installTestHooks(game, loop);

  const ui = document.getElementById("ui")!;
  mountToolbar(ui, game, loop);
  mountPanels(ui, game);
  const statsBar = mountStatsBar(ui);

  // Optionally pre-seed the real-world network (e.g. the MRT) via the Command path.
  if (withNetwork && city.raw.networkPath) {
    try {
      game.applyNetwork(await loadNetwork(city.raw.networkPath));
    } catch (e) {
      console.warn("network load failed; starting empty", e);
    }
  }

  map.once("load", () => game.refresh());
  game.refresh();
  loop.start();

  setInterval(() => {
    const s = bridge.stats();
    statsBar.update(s);
    game.setStats(s);
  }, 333);

  window.__ot = { map, bridge, city, overlay, game };
  window.__APP_READY = true;
}

function startApp(): void {
  const params = new URLSearchParams(location.search);
  const cityParam = params.get("city");
  if (cityParam) {
    // Deep-link / e2e: skip the menu.
    const entry = cityById(cityParam);
    void boot(entry.manifest, params.get("network") === "1");
  } else {
    showMenu((c: CityEntry, withNetwork: boolean) => void boot(c.manifest, withNetwork));
  }
}

startApp();
