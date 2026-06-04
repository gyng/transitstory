// Camera-independent test hooks (AGENTS testing): the e2e specs drive the game through
// these instead of raw pixel clicks, and placement routes through Game -> coords/geo.ts so
// the test exercises the production coordinate boundary, not a second one.
import type { Game } from "./game";

export function installTestHooks(game: Game): void {
  window.__ot_test = {
    placeStationLngLat: (lng, lat) => game.placeStation(lng, lat),
    drawLine: (ids) => game.drawLineByIds(ids),
    assignTrainset: (line, count) => game.assignTrainset(line, count),
    setHeadwayMs: (line, ms) => game.setHeadwayMs(line, ms),
    setRunning: (running) => game.setMode(running ? "run" : "build"),
    stats: () => game.bridge.stats(),
  };
}
