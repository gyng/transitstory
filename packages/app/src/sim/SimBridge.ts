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

  /** Commands undone but not yet superseded — redo re-applies them through the normal `apply`
   *  path (the log stays append-only; redo is just "send that command again"). Any FRESH command
   *  forks history and clears it, like every editor's redo. */
  private redoStack: Command[] = [];
  private replayingRedo = false;

  // Render-side cache for the TOPOLOGY snapshots (lines/stations). Both `LineView` (geometry,
  // stops, colour, modes, span/track types) and `StationView` (position, name, bounty,
  // platform count) change ONLY on a Command — every field is written exclusively in an `apply`
  // handler, never during `tick()` (verified: bounty/platform_count are command-only writes).
  // So a fresh wasm decode per call is pure waste: the roster used to decode ALL lines once PER
  // ROW (O(lines²), the measured 3 Hz freeze) and the vehicle path decoded `linesView()` twice
  // per rAF frame. Cache here, invalidate on every write. READ-ONLY for callers (they already are).
  private _linesView: LineView[] | null = null;
  private _stationsView: StationView[] | null = null;
  private invalidateViews(): void {
    this._linesView = null;
    this._stationsView = null;
  }

  /** The single write path. Returns the sim's events (assigned ids, auto-names, rejections). */
  apply(command: Command): Event[] {
    if (!this.replayingRedo) this.redoStack.length = 0; // a fresh command forks history
    const events = this.sim.applyCommandJson(encodeCommand(command)) as Event[];
    this.invalidateViews(); // topology may have changed
    this.log.push(command);
    this.onCommit?.();
    return events;
  }

  /** Reconstruct the Sim from seed + the current log (never splice state). The basis for both
   *  undo and load — replays through the same applyCommandJson path the log recorded. */
  private rebuild(): void {
    this.invalidateViews(); // topology rebuilt from scratch
    this.sim.free();
    this.sim = new Sim(this.seed, this.cityJson);
    for (const c of this.log.all()) this.sim.applyCommandJson(encodeCommand(c));
  }

  /** Undo = drop the last command and rebuild from seed + log[..-1]. Returns false if empty. */
  undo(): boolean {
    if (this.log.length === 0) return false;
    const popped = this.log.popLast();
    if (popped) this.redoStack.push(popped);
    this.rebuild();
    this.onCommit?.();
    return true;
  }

  /** Redo = re-apply the most recently undone command. Returns false with nothing to redo. */
  redo(): boolean {
    const cmd = this.redoStack.pop();
    if (!cmd) return false;
    this.replayingRedo = true;
    try {
      this.apply(cmd);
    } finally {
      this.replayingRedo = false;
    }
    return true;
  }

  canRedo(): boolean {
    return this.redoStack.length > 0;
  }

  /** Load a saved command log and rebuild (does NOT fire onCommit — the caller wires that after). */
  loadLog(cmds: readonly Command[]): void {
    this.log.replace(cmds);
    this.redoStack.length = 0; // a loaded save is a fresh history
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

  /** Interleaved marching-legion positions `[x0,y0,...]` in metres (fantasy/arcadia). Empty otherwise. */
  armyPositions(): Float32Array {
    return this.sim.armyPositions();
  }

  /** Interleaved legion TARGET positions `[x0,y0,...]` in metres, aligned with `armyPositions` (fantasy/
   *  arcadia, S11 — the AI intent). A marching legion's entry is its target town; others collapse to their
   *  own spot. Empty otherwise. */
  armyTargets(): Float32Array {
    return this.sim.armyTargets();
  }

  /** Interleaved raider positions `[x0,y0,...]` in metres (fantasy/arcadia, S11 — the rival). Empty otherwise. */
  raiderPositions(): Float32Array {
    return this.sim.raiderPositions();
  }

  /** Interleaved raider TARGET positions `[tx0,ty0,...]` in metres (#war — the rival's intent), aligned with
   *  raiderPositions. Where each raider is heading (capital / supply seam / captured town). Empty for transit. */
  raiderTargets(): Float32Array {
    return this.sim.raiderTargets();
  }

  /** Raider ROLE per raider `[role0,...]` (#war), aligned with raiderPositions: 0 breacher/1 saboteur/2 reclaimer. */
  raiderRoles(): Float32Array {
    return this.sim.raiderRoles();
  }

  /** #13 P1d — interleaved RIVAL HOST positions `[x0,y0,...]` in metres (the symmetric AI's mustered legions).
   *  Empty without a rival realm. */
  rivalHostPositions(): Float32Array {
    return this.sim.rivalHostPositions();
  }

  /** #13 — interleaved RIVAL HOST TARGET positions `[tx0,ty0,...]` in metres (the rival's intent, the telegraph),
   *  aligned with rivalHostPositions: the captured town each host marches to re-contest. */
  rivalHostTargets(): Float32Array {
    return this.sim.rivalHostTargets();
  }

  /** Legion STATE per legion `[state0,...]` (#war), aligned with armyPositions: 0 marching/1 besieging/2 done. */
  armyStates(): Float32Array {
    return this.sim.armyStates();
  }

  /** Per-vehicle CARGO commodity `[k0,...]` aligned with vehiclePositions (0 ore/1 grain/2 aether/3 fuel/
   *  4-7 processed; 255 = empty/transit) — colours the in-world 3D cargo block by the goods it hauls. */
  vehicleCargo(): Float32Array {
    return this.sim.vehicleCargo();
  }

  /** Trailing CARGO CARS pulled by rail trains (#multi-car), flat across all vehicles — 6 f32 per car
   *  `[x,y,angle,commodity,load,lineId]` (metres). A string of cars curving behind each loco. Empty for
   *  bus/ferry/air (single body). Pair with `vehicleCarsPrev()` for alpha interpolation. */
  vehicleCars(): Float32Array {
    return this.sim.vehicleCars();
  }

  /** Previous-tick positions of the trailing cargo cars `[x0,y0,...]` (metres), aligned 1:1 per car with
   *  `vehicleCars()` — the alpha-interpolation companion. */
  vehicleCarsPrev(): Float32Array {
    return this.sim.vehicleCarsPrev();
  }

  /** Interleaved spell flashes `[x,y,kind,alpha,...]` in metres (fantasy/arcadia, S11 — the spell arm). */
  spellFlashes(): Float32Array {
    return this.sim.spellFlashes();
  }

  /** Interleaved TTD signal markers `[x,y,status,...]` in metres; status 0=clear/green 1=occupied/red
   *  2=waiting/amber. Fresh array each call (no stale view). Empty off single-track. */
  signalMarkers(): Float32Array {
    return this.sim.signalMarkers();
  }

  /** PLAYER-PLACED block signals (TTD L5c) — the authoritative store as a flat Float64Array, 6 per
   *  signal `[line, path, span, at_mm, x_m, y_m]` (positions in metres; ids + at_mm are exact integers,
   *  lossless for a RemoveSignal round-trip). Distinct from `signalMarkers` (the per-tick occupancy
   *  readout). Read on the ~3 Hz / on-change cadence — these are the posts the player dropped. */
  placedSignals(): Float64Array {
    return this.sim.placedSignals();
  }

  /** Interleaved decadence-tide cells `[x0,y0,v0,...]` (metres + 0..1 strength) — the cold-tide overlay. */
  decadenceTide(): Float32Array {
    return this.sim.decadenceTide();
  }

  /** Interleaved `[onboard0,cap0, onboard1,cap1, ...]` per vehicle — the train inspector's load. */
  vehicleLoads(): Uint16Array {
    return this.sim.vehicleLoads();
  }

  /** Render-only "peep" dots at interpolation `alpha` + render `tickMs` — interleaved `[x,y,...]`
   *  metres (a fresh JS copy). Caches the paired colours, so call `peepColors()` immediately after.
   *  Determinism-free (the core reads only un-hashed passenger state). Capped in the core. */
  peepPositions(alpha: number, tickMs: number): Float32Array {
    return this.sim.peepPositions(alpha, tickMs);
  }

  /** RGBA bytes (4 per peep) paired with the LAST `peepPositions()` sweep — call order matters. */
  peepColors(): Uint8Array {
    return this.sim.peepColors();
  }

  /** Citizen id per peep (index-aligned with the LAST `peepPositions()` sweep), `0xffffffff` for an
   *  anonymous gravity rider — so a clicked peep maps back to a rider to inspect/follow. */
  peepCitizens(): Uint32Array {
    return this.sim.peepCitizens();
  }

  stats(): Stats {
    return this.sim.stats() as Stats;
  }

  /** Topology snapshot of all stations — cached until the next Command (apply/rebuild). The
   *  returned array is READ-ONLY (callers .find/.filter/.map/index it; none mutate it). */
  stationsView(): StationView[] {
    return (this._stationsView ??= this.sim.stationsView() as StationView[]);
  }

  /** Topology snapshot of all lines — cached until the next Command (apply/rebuild). READ-ONLY
   *  (see `stationsView`). Collapses the roster's former O(lines²) per-row decode to O(lines). */
  linesView(): LineView[] {
    return (this._linesView ??= this.sim.linesView() as LineView[]);
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
