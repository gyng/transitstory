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
    // TTD L5c: place/remove a block signal at a lng/lat by the SAME production path a click takes (project →
    // signalGestureAt → coords/geo.ts span/at_mm geometry). Requires a line selected in build mode.
    placeSignalLngLat: (lng, lat) => game.placeSignalLngLat(lng, lat),
    // TTD L5c testid contract: ALL placed signals' addresses as `signal-<line>-<span>-<atMm>` ids (deck
    // layers aren't DOM, so the e2e asserts on this camera-independent list + the `placed-signals` layer).
    placedSignalIds: () => {
      const raw = game.bridge.placedSignals();
      const ids: string[] = [];
      for (let i = 0; i + 5 < raw.length; i += 6) ids.push(`signal-${raw[i]}-${raw[i + 2]}-${raw[i + 3]}`);
      return ids;
    },
    placeBarracksLngLat: (lng, lat) => game.placeBarracks(lng, lat),
    postBounty: (station, amount) => game.postBounty(station, amount),
    unlockTech: (tech) => game.unlockTech(tech),
    castSpell: (kind) => game.castSpell(kind),
    setAutocast: (enabled) => game.setAutocast(enabled),
    drawLine: (ids) => game.drawLineByIds(ids),
    // TTD L5c: select a line (the signal gesture is contextual — it needs a selected line) + set its
    // whole-line track type (0=double, 1=single) — the single-track precondition for placing a signal.
    selectLine: (id) => game.selectLine(id),
    setLineTrack: (line, track) => game.setLineTrack(line, track),
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
