// Camera-independent test hooks (AGENTS testing): the e2e specs drive the game through
// these instead of raw pixel clicks, and placement routes through Game -> coords/geo.ts so
// the test exercises the production coordinate boundary, not a second one.
import type { Game } from "./game";
import type { GameLoop } from "./sim/GameLoop";

export function installTestHooks(game: Game, loop: GameLoop): void {
  window.__ot_test = {
    placeStationLngLat: (lng, lat) => game.placeStation(lng, lat),
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
