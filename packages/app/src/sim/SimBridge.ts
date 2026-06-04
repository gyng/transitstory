// The one module that talks to the wasm Sim. UI/tools go through here: send Commands,
// read snapshots. wasm-bindgen returns Vec<f32>/Vec<u32> as fresh JS-owned typed arrays
// (already the safe copy-out — no long-lived view onto wasm memory, so no detached-buffer
// risk per PLAN §0.5). The `sim-wasm` package auto-instantiates the wasm on import (bundler
// target + vite-plugin-wasm + top-level await), so `new Sim(...)` is ready synchronously.
import { Sim } from "sim-wasm";
import { CommandLog } from "../commands/log";
import { encodeCommand } from "../commands/codec";
import type { Command, Event, LineView, StationView, Stats } from "../types";

export class SimBridge {
  private sim: Sim;
  readonly log = new CommandLog();

  constructor(seed: number, cityJson: string) {
    this.sim = new Sim(seed, cityJson);
  }

  /** The single write path. Returns the sim's events (assigned ids, auto-names, rejections). */
  apply(command: Command): Event[] {
    const events = this.sim.applyCommandJson(encodeCommand(command)) as Event[];
    this.log.push(command);
    return events;
  }

  tick(dtMs: number): void {
    this.sim.tick(dtMs);
  }

  /** Hex string (the determinism oracle); BigInt never crosses the boundary. */
  stateHash(): string {
    return this.sim.stateHash();
  }

  vehicleCount(): number {
    return this.sim.vehicleCount();
  }

  /** Interleaved `[x0,y0,...]` metres, current tick (a fresh JS-owned copy). */
  vehiclePositions(): Float32Array {
    return this.sim.vehiclePositions();
  }

  /** Interleaved `[x0,y0,...]` metres, previous tick (for alpha interpolation). */
  vehiclePrevPositions(): Float32Array {
    return this.sim.vehiclePrevPositions();
  }

  vehicleAngles(): Float32Array {
    return this.sim.vehicleAngles();
  }

  vehicleLineIds(): Uint32Array {
    return this.sim.vehicleLineIds();
  }

  stats(): Stats {
    return this.sim.stats() as Stats;
  }

  stationsView(): StationView[] {
    return this.sim.stationsView() as StationView[];
  }

  linesView(): LineView[] {
    return this.sim.linesView() as LineView[];
  }
}
