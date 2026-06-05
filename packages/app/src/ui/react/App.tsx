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
import { writeSave, type SaveBlob } from "../../sim/save";
import { Game } from "../../game";
import { GameLoop } from "../../sim/GameLoop";
import { attachPointer } from "../../tools/pointer";
import { installTestHooks } from "../../testhooks";
import { GameProvider, useGame, useGameUI } from "./GameContext";
import { Menu } from "./Menu";
import { StatsBar } from "./StatsBar";
import { Panels } from "./Panels";
import { Toolbar } from "./Toolbar";
import { OnboardingCoach } from "./Onboarding";
import { ObjectivePanel } from "./Objectives";
import { ServiceReport } from "./ServiceReport";
import { DraftControls } from "./DraftControls";
import { CommuterCard } from "./CommuterCard";
import { FollowCard } from "./FollowCard";
import { getScenario } from "../../objectives";

interface BootedWorld {
  game: Game;
  loop: GameLoop;
  cityName: string;
}

/** Build the imperative world for one city. Mirrors the old main.ts boot() — sans the UI
 *  mounts (React owns those) and the 3 Hz stats interval (the GameProvider runs it). When
 *  `resume` is given, the saved command log is replayed instead of the real network. */
async function boot(manifestPath: string, withNetwork: boolean, resume?: SaveBlob): Promise<BootedWorld> {
  const city = await loadCity(manifestPath); // sets the session coordinate origin

  const map = createMap("map", city.raw.center, city.raw.zoom);
  const overlay = createOverlay();
  map.addControl(overlay);

  const bridge = new SimBridge(city.seed, city.coreCityJson);
  const game = new Game(bridge, map, overlay, new Buildability(city.buildability));
  game.demandHeat = city.demandHeat; // travel-demand heat overlay source
  game.demandCellM = city.demandCellM; // sizes the demand-heat hexagons to the grid pitch
  const loop = new GameLoop(game);
  attachPointer(game);
  installTestHooks(game, loop);

  if (resume) {
    bridge.loadLog(resume.log); // replays seed + saved log through the same Command path
    game.mode = bridge.stats().running ? "run" : "build";
  } else if (withNetwork && city.raw.networkPath) {
    try {
      game.applyNetwork(await loadNetwork(city.raw.networkPath));
    } catch (e) {
      console.warn("network load failed; starting empty", e);
    }
  }

  // Autosave from the first PLAYER action onward — wiring onCommit after the pre-seeded network
  // / resume replay means we don't re-save the baseline; the log always holds the full state.
  bridge.onCommit = () =>
    writeSave({ v: 1, cityId: city.raw.id, cityName: city.raw.name, seed: city.seed, log: [...bridge.log.all()] });

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
      Transit Story · {name}
    </div>
  );
}

export function App() {
  const [world, setWorld] = useState<BootedWorld | null>(null);
  const [booting, setBooting] = useState(false);
  const [scenarioId, setScenarioId] = useState<string | null>(null);
  const started = useRef(false);

  const startBoot = useCallback((manifestPath: string, withNetwork: boolean, scenario: string | null = null) => {
    setScenarioId(scenario);
    setBooting(true);
    void boot(manifestPath, withNetwork).then(setWorld);
  }, []);

  const startResume = useCallback((save: SaveBlob) => {
    setScenarioId(null);
    setBooting(true);
    void boot(cityById(save.cityId).manifest, false, save).then(setWorld);
  }, []);

  // Deep-link / e2e: `?city=<id>&network=0|1&scenario=<id>` skips the menu.
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const params = new URLSearchParams(location.search);
    const cityParam = params.get("city");
    if (cityParam) {
      const entry = cityById(cityParam);
      // The globe's "network" is its cities — always load it (the board is empty without them).
      startBoot(entry.manifest, entry.id === "globe" || params.get("network") === "1", params.get("scenario"));
    }
  }, [startBoot]);

  // Ctrl/Cmd-Z = undo the last committed command (rebuild from seed + log[..-1]).
  useEffect(() => {
    if (!world) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "z") {
        e.preventDefault();
        world.game.undo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [world]);

  if (!world) {
    if (booting) return null; // map is being built; chrome appears once the world is ready
    return (
      <Menu
        onStart={(c: CityEntry, withNet: boolean, scenario: string | null) =>
          startBoot(c.manifest, c.id === "globe" || withNet, scenario)}
        onResume={startResume}
      />
    );
  }

  const scenario = getScenario(scenarioId);

  return (
    <GameProvider game={world.game} loop={world.loop}>
      <Title name={world.cityName} />
      <UndoControl />
      <Toast />
      <OnboardingCoach />
      <StatsBar />
      {scenario && <ObjectivePanel scenario={scenario} />}
      <Panels />
      <ServiceReport />
      <DraftControls />
      <CommuterCard />
      <FollowCard />
      <Toolbar />
    </GameProvider>
  );
}

/** Transient notice (e.g. an afford-gate rejection) so a gated Command has a visible echo
 *  (AGENTS: every Command needs an on-map/HUD echo). Auto-dismisses after a few seconds. */
function Toast() {
  const game = useGame();
  const { notice } = useGameUI();
  useEffect(() => {
    if (!notice) return;
    const id = window.setTimeout(() => game.dismissNotice(), 3000);
    return () => window.clearTimeout(id);
  }, [notice, game]);
  if (!notice) return null;
  return (
    <div
      data-testid="notice"
      onClick={() => game.dismissNotice()}
      style={{
        position: "fixed",
        top: 56,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 20,
        padding: "8px 16px",
        borderRadius: 8,
        background: "var(--ot-gauge-bad, #d62828)",
        color: "#fff",
        font: "600 13px system-ui,sans-serif",
        boxShadow: "0 4px 16px rgba(0,0,0,.3)",
        cursor: "pointer",
      }}
    >
      {notice}
    </div>
  );
}

/** A small undo affordance next to the title (AGENTS UX "reversible by construction"). Ctrl-Z
 *  is the primary path; this makes it discoverable. Re-renders on the UI slice so its disabled
 *  state tracks the 0↔1 command boundary. */
function UndoControl() {
  const game = useGame();
  useGameUI(); // subscribe: re-render when selection/mode change (covers the empty↔non-empty edge)
  const enabled = game.canUndo();
  return (
    <button
      data-testid="undo"
      onClick={() => game.undo()}
      disabled={!enabled}
      title="Undo last action (Ctrl-Z)"
      style={{
        position: "fixed",
        top: 10,
        left: 200,
        zIndex: 10,
        padding: "4px 10px",
        borderRadius: 8,
        border: "0",
        background: enabled ? "rgba(255,255,255,.85)" : "rgba(255,255,255,.4)",
        color: enabled ? "#1c2024" : "#9aa3ad",
        font: "600 13px system-ui,sans-serif",
        cursor: enabled ? "pointer" : "default",
        boxShadow: "0 2px 10px rgba(0,0,0,.12)",
      }}
    >
      ↶ Undo
    </button>
  );
}
