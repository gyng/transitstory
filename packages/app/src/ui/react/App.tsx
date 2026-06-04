// React root. Phase machine: menu → (boot) → playing. `boot()` builds the imperative world
// (map, deck overlay, sim, Game, GameLoop) exactly as the old main.ts did — React only owns
// the floating chrome inside #ui. The map/deck overlay live in the separate #map div and the
// rAF loop runs entirely outside React (AGENTS render-hot-path / two-clocks rules).
import { useCallback, useEffect, useRef, useState } from "react";
import { createMap } from "../../map/basemap";
import { createOverlay } from "../../map/overlay";
import { loadCity } from "../../sim/city";
import { loadNetwork } from "../../sim/network";
import { cityById, type CityEntry } from "../../sim/cities";
import { SimBridge } from "../../sim/SimBridge";
import { Buildability } from "../../sim/buildability";
import { Game } from "../../game";
import { GameLoop } from "../../sim/GameLoop";
import { attachPointer } from "../../tools/pointer";
import { installTestHooks } from "../../testhooks";
import { GameProvider } from "./GameContext";
import { Menu } from "./Menu";
import { StatsBar } from "./StatsBar";
import { Panels } from "./Panels";
import { Toolbar } from "./Toolbar";

interface BootedWorld {
  game: Game;
  loop: GameLoop;
  cityName: string;
}

/** Build the imperative world for one city. Mirrors the old main.ts boot() — sans the UI
 *  mounts (React owns those) and the 3 Hz stats interval (the GameProvider runs it). */
async function boot(manifestPath: string, withNetwork: boolean): Promise<BootedWorld> {
  const city = await loadCity(manifestPath); // sets the session coordinate origin

  const map = createMap("map", city.raw.center, city.raw.zoom);
  const overlay = createOverlay();
  map.addControl(overlay);

  const bridge = new SimBridge(city.seed, city.coreCityJson);
  const game = new Game(bridge, map, overlay, new Buildability(city.buildability));
  game.demandHeat = city.demandHeat; // travel-demand heat overlay source
  const loop = new GameLoop(game);
  attachPointer(game);
  installTestHooks(game, loop);

  if (withNetwork && city.raw.networkPath) {
    try {
      game.applyNetwork(await loadNetwork(city.raw.networkPath));
    } catch (e) {
      console.warn("network load failed; starting empty", e);
    }
  }

  map.once("load", () => game.refresh());
  game.refresh();

  window.__ot = { map, bridge, city, overlay, game };
  window.__APP_READY = true;
  return { game, loop, cityName: city.raw.name };
}

function Title({ name }: { name: string }) {
  return (
    <div
      id="app-title"
      style={{
        position: "fixed",
        top: 10,
        left: 14,
        margin: 0,
        padding: "4px 10px",
        borderRadius: 8,
        background: "rgba(255,255,255,.85)",
        font: "600 14px system-ui,sans-serif",
        color: "#1c2024",
        boxShadow: "0 2px 10px rgba(0,0,0,.12)",
        zIndex: 10,
      }}
    >
      onlytransits · {name}
    </div>
  );
}

export function App() {
  const [world, setWorld] = useState<BootedWorld | null>(null);
  const [booting, setBooting] = useState(false);
  const started = useRef(false);

  const startBoot = useCallback((manifestPath: string, withNetwork: boolean) => {
    setBooting(true);
    void boot(manifestPath, withNetwork).then(setWorld);
  }, []);

  // Deep-link / e2e: `?city=<id>&network=0|1` skips the menu.
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const params = new URLSearchParams(location.search);
    const cityParam = params.get("city");
    if (cityParam) {
      startBoot(cityById(cityParam).manifest, params.get("network") === "1");
    }
  }, [startBoot]);

  if (!world) {
    if (booting) return null; // map is being built; chrome appears once the world is ready
    return <Menu onStart={(c: CityEntry, withNet: boolean) => startBoot(c.manifest, withNet)} />;
  }

  return (
    <GameProvider game={world.game} loop={world.loop}>
      <Title name={world.cityName} />
      <StatsBar />
      <Panels />
      <Toolbar />
    </GameProvider>
  );
}
