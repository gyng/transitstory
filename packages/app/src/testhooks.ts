// Camera-independent test hooks (AGENTS testing): the e2e specs drive the game through
// these instead of raw pixel clicks, and placement routes through Game -> coords/geo.ts so
// the test exercises the production coordinate boundary, not a second one.
import type { Game } from "./game";
import type { GameLoop } from "./sim/GameLoop";
import { cmd } from "./commands/codec";
import { TICK_MS } from "./config";

export function installTestHooks(game: Game, loop: GameLoop): void {
  window.__ot_test = {
    // Advance the sim SYNCHRONOUSLY by `ms` (deterministic, no rAF). Puts the sim in its running state
    // (so dispatch/movement happen) WITHOUT entering run MODE — GameLoop only auto-ticks when
    // `game.mode === "run"`, so leaving mode as-is means these manual steps never double with the rAF.
    // Lets sim-behaviour e2e assert outcomes without depending on wall-clock rAF (which starves + flakes
    // under parallel load). Test-only.
    tickMs: (ms: number) => {
      game.bridge.apply(cmd.setRunning(true));
      const n = Math.max(0, Math.floor(ms / TICK_MS));
      for (let i = 0; i < n; i++) game.bridge.tick(TICK_MS);
      game.refresh();
    },
    placeStationLngLat: (lng, lat) => game.placeStation(lng, lat),
    placeBarracksLngLat: (lng, lat) => game.placeBarracks(lng, lat),
    postBounty: (station, amount) => game.postBounty(station, amount),
    drawLine: (ids) => game.drawLineByIds(ids),
    assignTrainset: (line, count) => game.assignTrainset(line, count),
    setHeadwayMs: (line, ms) => game.setHeadwayMs(line, ms),
    setRunning: (running) => game.setMode(running ? "run" : "build"),
    setSpeed: (mult) => loop.setSpeed(mult),
    setLineMode: (line, mode) => game.setLineMode(line, mode),
    setTransport: (mode) => game.setTransport(mode),
    setModeEnabled: (mode, on) => game.setModeEnabled(mode, on),
    setShowDemand: (on) => game.setShowDemand(on),
    setShowReach: (on) => game.setShowReach(on),
    setShowRoads: (on) => game.setShowRoads(on),
    stationTip: (id) => game.stationTip(id),
    lineTip: (id) => game.lineTip(id),
    vehicleTip: (index) => game.vehicleTip(index),
    stats: () => game.bridge.stats(),
  };
}
