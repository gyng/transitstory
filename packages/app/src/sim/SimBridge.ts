// The one module that talks to the wasm Sim. UI/tools go through here: send Commands,
// read snapshots. wasm-bindgen returns Vec<f32>/Vec<u32> as fresh JS-owned typed arrays
// (already the safe copy-out — no long-lived view onto wasm memory, so no detached-buffer
// risk per PLAN §0.5). The `sim-wasm` package auto-instantiates the wasm on import (bundler
// target + vite-plugin-wasm + top-level await), so `new Sim(...)` is ready synchronously.
import { Sim } from "sim-wasm";
import { CommandLog } from "../commands/log";
import { encodeCommand } from "../commands/codec";
import type { AccessLink, Command, Event, FollowView, JourneyView, LineView, OdLink, ShedCell, StationView, Stats } from "../types";

export class SimBridge {
  private sim: Sim;
  readonly log = new CommandLog();
  /** Fired after every committed command and after undo — boot wires autosave here. */
  onCommit: (() => void) | null = null;

  constructor(
    readonly seed: number,
    private readonly cityJson: string,
  ) {
    this.sim = new Sim(seed, cityJson);
  }

  /** The single write path. Returns the sim's events (assigned ids, auto-names, rejections). */
  apply(command: Command): Event[] {
    const events = this.sim.applyCommandJson(encodeCommand(command)) as Event[];
    this.log.push(command);
    this.onCommit?.();
    return events;
  }

  /** Reconstruct the Sim from seed + the current log (never splice state). The basis for both
   *  undo and load — replays through the same applyCommandJson path the log recorded. */
  private rebuild(): void {
    this.sim.free();
    this.sim = new Sim(this.seed, this.cityJson);
    for (const c of this.log.all()) this.sim.applyCommandJson(encodeCommand(c));
  }

  /** Undo = drop the last command and rebuild from seed + log[..-1]. Returns false if empty. */
  undo(): boolean {
    if (this.log.length === 0) return false;
    this.log.popLast();
    this.rebuild();
    this.onCommit?.();
    return true;
  }

  /** Load a saved command log and rebuild (does NOT fire onCommit — the caller wires that after). */
  loadLog(cmds: readonly Command[]): void {
    this.log.replace(cmds);
    this.rebuild();
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

  /** Interleaved `[onboard0,cap0, onboard1,cap1, ...]` per vehicle — the train inspector's load. */
  vehicleLoads(): Uint16Array {
    return this.sim.vehicleLoads();
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

  /** OD "desire lines" for a selected station — its top destinations by gravity pull (read-only,
   *  computed on demand). Empty unless the station is operational + served. */
  stationOd(id: number): OdLink[] {
    return this.sim.stationOd(id) as OdLink[];
  }

  /** Accessibility isochrone for a selected station — reachable stations + transit travel time
   *  (read-only, computed on demand). Empty unless the station is operational + served. */
  stationAccess(id: number): AccessLink[] {
    return this.sim.stationAccess(id) as AccessLink[];
  }

  /** Walk shed for a selected station — the buildability cells it reaches on foot (water severs,
   *  crossed corridors pinch), each with a decay intensity (read-only). Empty when the city has no
   *  raster, so the caller falls back to the nominal-radius ring. */
  stationWalkshed(id: number): ShedCell[] {
    return this.sim.stationWalkshed(id) as ShedCell[];
  }

  /** Inspect the `nth` waiting rider at a station — a named commuter (agent demand) or anonymous
   *  gravity trip, with route + home/work. null if no one is waiting there. */
  sampleJourney(station: number, nth: number): JourneyView | null {
    return this.sim.sampleJourney(station, nth) as JourneyView | null;
  }

  /** Live state of a followed citizen, or null if they're not currently in transit. */
  followCitizen(citizenId: number): FollowView | null {
    return this.sim.followCitizen(citizenId) as FollowView | null;
  }

  /** Authoritative construction-cost ($, track only) for a hypothetical line — the build HUD's
   *  preview figure (read-only; never mutates state). */
  previewLineCost(stationIds: number[], mode: number, loopLine: boolean): number {
    return this.sim.previewLineCost(new Uint32Array(stationIds), mode, loopLine);
  }
}
