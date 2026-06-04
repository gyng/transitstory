// The hub controller: owns the SimBridge, map, and overlay; holds UI state (mode, tool,
// selection, line draft); rebuilds the overlay from authoritative sim views. UI/tools only
// call Game methods which emit Commands and refresh — they never mutate sim state directly.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import type { Layer } from "@deck.gl/core";
import { CATCHMENT_M, LINE_PALETTE, SNAP_PX } from "./config";
import { lngLatToMm, metersToLngLat, mmToLngLat } from "./coords/geo";
import { cmd } from "./commands/codec";
import { colorToRgb, topoLayers, vehicleLayer, type HazardDot, type RenderView, type VehicleDot, type WaitingDot } from "./render";
import { WHOLE_LINE } from "./commands/codec";
import { BUILD, Buildability } from "./sim/buildability";
import type { SimBridge } from "./sim/SimBridge";
import type { Stats } from "./types";

const EMPTY_STATS: Stats = {
  simClockMs: 0,
  running: false,
  stationCount: 0,
  lineCount: 0,
  vehicleCount: 0,
  ridershipTotal: 0,
  waitingTotal: 0,
  leftBehind: 0,
  deniedBoardings: 0,
  avgJourneyMs: 0,
  avgWaitMs: 0,
  avgLoadFactor: 0,
  coverageScore: 0,
  simHour: 6,
  period: "AM rush",
  demandMultiplier: 1,
  buildDifficulty: 0,
  economyEnabled: true,
  balance: 0,
  capitalSpent: 0,
  fareRevenue: 0,
  perStation: [],
  perLine: [],
};

export type Mode = "build" | "run";
export type Tool = "select" | "station" | "line";

export class Game {
  mode: Mode = "build";
  tool: Tool = "station";
  /** Active transport mode for new construction (0 rail,1 bus,2 ferry,3 air). The chorded
   *  bottom bar sets this; new lines are created with it and the buildability gate follows. */
  transport = 0;
  /** Which transport modes are enabled (settings panel). Disabled modes can't be selected
   *  in the chorded bar — a frontend gate; the sim is mode-agnostic about availability. */
  enabledModes = new Set([0, 1, 2, 3]);
  /** Demand-heat map layer toggle + its source points (lng/lat + weight), set at boot. */
  showDemand = false;
  demandHeat: import("./render").DemandPoint[] = [];
  selectedStation: number | null = null;
  selectedLine: number | null = null;
  hoveredStation: number | null = null;

  /** In-progress line draft (ordered station ids) + live cursor lng/lat (T11). */
  draft: number[] = [];
  cursor: [number, number] | null = null;

  /** Listeners notified after each refresh (panels/stats bind here). */
  onChange: (() => void)[] = [];

  /** Cached topology layers (stable identity across frames; rebuilt only on refresh). */
  private below: Layer[] = [];
  private above: Layer[] = [];

  /** Latest stats snapshot (refreshed on the ~3 Hz throttle); drives waiting-pax halos. */
  lastStats: Stats = EMPTY_STATS;

  constructor(
    readonly bridge: SimBridge,
    readonly map: MlMap,
    readonly overlay: MapboxOverlay,
    readonly build: Buildability = new Buildability(),
  ) {}

  /** Set the build mode (0 Surface, 1 Elevated, 2 Tunnel) for a whole line (or one span). */
  setLineMode(line: number, mode: number, span: number = WHOLE_LINE): void {
    this.bridge.apply(cmd.setSegmentMode(line, span, mode));
    this.refresh();
  }

  // --- commands (the only write path) ---

  placeStation(lng: number, lat: number): number {
    const [x_mm, y_mm] = lngLatToMm([lng, lat]);
    const events = this.bridge.apply(cmd.placeStation(x_mm, y_mm));
    const placed = events.find((e) => "StationPlaced" in e) as
      | { StationPlaced: { id: number } }
      | undefined;
    const id = placed ? placed.StationPlaced.id : -1;
    this.selectedStation = id >= 0 ? id : this.selectedStation; // show its catchment
    this.refresh();
    return id;
  }

  /** Commit a line through the given ordered station ids (CreateLine + AddStop*). The
   *  interactive draw gesture (T11) and the test hook both funnel here. */
  drawLineByIds(ids: number[]): number {
    if (ids.length < 2) return -1;
    const ev = this.bridge.apply(cmd.createLine(this.nextLineColor(), null, false, this.transport));
    const created = ev.find((e) => "LineCreated" in e) as
      | { LineCreated: { id: number } }
      | undefined;
    const lineId = created ? created.LineCreated.id : this.bridge.linesView().length - 1;
    for (const s of ids) this.bridge.apply(cmd.addStop(lineId, s));
    this.cancelDraft();
    this.selectedLine = lineId;
    this.selectedStation = null;
    this.refresh();
    return lineId;
  }

  /** Assign trains and auto-suggest a sensible headway (round-trip / count) on first assign. */
  assignTrainset(line: number, count: number): void {
    const hadTrains = (this.bridge.linesView()[line] && this.lineTrains(line)) || 0;
    this.bridge.apply(cmd.assignTrainset(line, 0, count));
    if (hadTrains === 0) {
      this.bridge.apply(cmd.setHeadway(line, this.suggestHeadwayMs(line, count)));
    }
    this.refresh();
  }

  /** The Headway slider is the lever: derive the train count from headway (round-trip / H)
   *  and re-assign, keeping count↔headway consistent (the dispatcher uses count). */
  setEconomy(enabled: boolean): void {
    this.bridge.apply(cmd.setEconomy(enabled));
    this.refresh();
  }

  setHeadwayMs(line: number, ms: number): void {
    this.bridge.apply(cmd.setHeadway(line, ms));
    const count = Math.max(1, Math.min(8, Math.round(this.roundTripMs(line) / ms)));
    this.bridge.apply(cmd.assignTrainset(line, 0, count));
    this.refresh();
  }

  private lineTrains(line: number): number {
    return this.bridge.stats().perLine.find((l) => l.lineId === line)?.trains ?? 0;
  }

  /** Estimated round-trip travel time (ms): out-and-back run + dwell at each stop. */
  roundTripMs(line: number): number {
    const l = this.bridge.linesView()[line];
    if (!l) return 600_000;
    let lenMm = 0;
    for (let i = 1; i < l.polylineMm.length; i++) {
      const [ax, ay] = l.polylineMm[i - 1];
      const [bx, by] = l.polylineMm[i];
      lenMm += Math.hypot(bx - ax, by - ay);
    }
    const vMaxMmS = 22_000;
    const dwellMs = 20_000;
    const oneWayMs = (lenMm / vMaxMmS) * 1000 + l.stops.length * dwellMs;
    return 2 * oneWayMs;
  }

  /** Round-trip / train count, clamped to the sim's headway bounds. */
  suggestHeadwayMs(line: number, count: number): number {
    return Math.max(30_000, Math.min(1_800_000, Math.round(this.roundTripMs(line) / Math.max(1, count))));
  }

  setMode(mode: Mode): void {
    this.mode = mode;
    this.bridge.apply(cmd.setRunning(mode === "run"));
    if (mode === "run") this.cancelDraft();
    this.refresh();
  }

  setTool(tool: Tool): void {
    if (this.tool === "line" && tool !== "line") this.cancelDraft();
    this.tool = tool;
    this.refresh();
  }

  /** Select the transport mode for new construction (chorded bottom bar). Switching mode
   *  drops any in-progress draft and arms the line tool so the next draw uses the new mode. */
  setTransport(mode: number): void {
    if (!this.enabledModes.has(mode) || mode === this.transport) return;
    this.cancelDraft();
    this.transport = mode;
    this.tool = "line";
    this.refresh();
  }

  /** Enable/disable a transport mode (settings panel). Disabling the active mode falls back
   *  to the lowest still-enabled mode so construction always targets a valid mode. */
  setModeEnabled(mode: number, on: boolean): void {
    if (on) this.enabledModes.add(mode);
    else this.enabledModes.delete(mode);
    if (!this.enabledModes.has(this.transport)) {
      const next = [0, 1, 2, 3].find((m) => this.enabledModes.has(m));
      if (next !== undefined) this.transport = next;
    }
    this.refresh();
  }

  /** Toggle the travel-demand heat map layer. */
  setShowDemand(on: boolean): void {
    this.showDemand = on;
    this.refresh();
  }

  selectStation(id: number | null): void {
    this.selectedStation = id;
    this.selectedLine = null;
    this.refresh();
  }

  selectLine(id: number | null): void {
    this.selectedLine = id;
    this.selectedStation = null;
    this.refresh();
  }

  cancelDraft(): void {
    this.draft = [];
    this.cursor = null;
    this.map.dragPan.enable();
  }

  // --- interactive line drawing (snap → blueprint → commit) ---

  /** Append a snapped station to the in-progress line; disables dragPan on first point. */
  extendDraft(stationId: number): void {
    if (this.draft.length === 0) this.map.dragPan.disable();
    if (this.draft[this.draft.length - 1] !== stationId) this.draft.push(stationId);
    this.refresh();
  }

  /** Commit the blueprint as one line (re-enables dragPan). Needs >= 2 stops. */
  commitDraft(): void {
    const ids = [...this.draft];
    this.map.dragPan.enable();
    this.cursor = null;
    this.draft = [];
    if (ids.length >= 2) this.drawLineByIds(ids);
    else this.refresh();
  }

  // --- geometry helpers ---

  /** Nearest station id to a screen pixel within `maxPx`, else null (screen-space snap). */
  nearestStation(px: number, py: number, maxPx = SNAP_PX): number | null {
    let best: number | null = null;
    let bestD = maxPx;
    for (const s of this.bridge.stationsView()) {
      const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
      const p = this.map.project([lng, lat]);
      const d = Math.hypot(p.x - px, p.y - py);
      if (d <= bestD) {
        bestD = d;
        best = s.id;
      }
    }
    return best;
  }

  // --- rendering ---

  private buildView(): RenderView {
    const stationsV = this.bridge.stationsView();
    const linesV = this.bridge.linesView();
    const highlight =
      this.selectedStation ?? this.hoveredStation ?? null;

    const stations = stationsV.map((s) => {
      const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
      return { id: s.id, lng, lat, name: s.name, selected: s.id === this.selectedStation };
    });

    const catchments =
      highlight === null
        ? []
        : stationsV
            .filter((s) => s.id === highlight)
            .map((s) => {
              const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
              return { lng, lat, radiusM: CATCHMENT_M };
            });

    const lines = linesV.map((l) => ({
      id: l.id,
      // A line with surface track over water renders red until elevated/tunnelled.
      color: l.crossesWaterSurface ? ([214, 40, 40] as [number, number, number]) : colorToRgb(l.color),
      path: l.polylineMm.map(([x, y]) => mmToLngLat([x, y])),
    }));

    // Blueprint: draft station positions + live cursor (T11 populates draft).
    const blueprint: [number, number][] = this.draft.map((id) => {
      const s = stationsV[id];
      return mmToLngLat([s.xMm, s.yMm]);
    });
    if (this.cursor && blueprint.length >= 1) blueprint.push(this.cursor);

    // Live build-conflict dots along the blueprint (amber built/park, red water) — so the
    // player sees they can't just run surface rail over stuff as they draw.
    const hazards: HazardDot[] = [];
    if (blueprint.length >= 2 && this.build.loaded) {
      for (let i = 1; i < blueprint.length; i++) {
        const a = lngLatToMm(blueprint[i - 1]);
        const b = lngLatToMm(blueprint[i]);
        const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
        const steps = Math.min(60, Math.max(1, Math.round(len / this.build.cellMm)));
        for (let k = 0; k <= steps; k++) {
          const x = a[0] + ((b[0] - a[0]) * k) / steps;
          const y = a[1] + ((b[1] - a[1]) * k) / steps;
          const c = this.build.classifyMm(x, y);
          if (c === BUILD.WATER) {
            const [lng, lat] = mmToLngLat([x, y]);
            hazards.push({ lng, lat, color: [214, 40, 40] });
          } else if (c === BUILD.BUILT || c === BUILD.PARK) {
            const [lng, lat] = mmToLngLat([x, y]);
            hazards.push({ lng, lat, color: [230, 159, 0] });
          }
        }
      }
    }

    // Waiting-passenger halos from the latest stats snapshot (positioned at stations).
    const waiting: WaitingDot[] = [];
    for (const ps of this.lastStats.perStation) {
      if (ps.waiting > 0) {
        const s = stationsV[ps.stationId];
        if (s) {
          const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
          waiting.push({ lng, lat, count: ps.waiting });
        }
      }
    }

    const demand = this.showDemand ? this.demandHeat : [];
    return { stations, lines, catchments, blueprint, vehicles: [], waiting, hazards, demand };
  }

  /** Push a fresh stats snapshot (called on the ~3 Hz UI throttle) and re-render halos. */
  setStats(s: Stats): void {
    this.lastStats = s;
    this.refresh();
  }

  /** Rebuild cached topology layers from authoritative sim views; recompose with current
   *  (non-interpolated) vehicle positions. The GameLoop recomposes per frame with alpha. */
  refresh(): void {
    const { below, above } = topoLayers(this.buildView());
    this.below = below;
    this.above = above;
    this.composeAndSet(this.currentVehicleDots());
    for (const cb of this.onChange) cb();
  }

  /** Set the overlay layers: stable cached topo with the vehicle layer in z-order between
   *  them (catchment/lines/blueprint < vehicles < stations). Reused topo instances mean deck
   *  only re-uploads the small vehicle layer each frame. */
  composeAndSet(vehicles: VehicleDot[]): void {
    this.overlay.setProps({ layers: [...this.below, vehicleLayer(vehicles), ...this.above] });
  }

  /** Per-line colour table indexed by line id (for vehicle tint). */
  lineColors(): number[] {
    return this.bridge.linesView().map((l) => l.color);
  }

  /** Current (non-interpolated) vehicle dots in lng/lat, tinted by line. */
  currentVehicleDots(): VehicleDot[] {
    const pos = this.bridge.vehiclePositions();
    const lineIds = this.bridge.vehicleLineIds();
    const colors = this.lineColors();
    const dots: VehicleDot[] = [];
    for (let i = 0; i < pos.length; i += 2) {
      const [lng, lat] = metersToLngLat([pos[i], pos[i + 1]]);
      dots.push({ lng, lat, color: colorToRgb(colors[lineIds[i / 2]] ?? 0x444444) });
    }
    return dots;
  }

  /** Pre-seed a real-world network (stations + lines) via the Command path. Stations are
   *  placed in array order (so line indices match), interchanges are shared station indices. */
  applyNetwork(net: import("./sim/network").Network): void {
    for (const s of net.stations) {
      const [x, y] = lngLatToMm([s.lng, s.lat]);
      this.bridge.apply(cmd.placeStation(x, y, s.name));
    }
    net.lines.forEach((line, li) => {
      this.bridge.apply(cmd.createLine(parseInt(line.colorHex, 16) >>> 0, line.name, line.loop ?? false, line.mode ?? 0));
      for (const idx of line.stations) this.bridge.apply(cmd.addStop(li, idx));
      this.bridge.apply(cmd.assignTrainset(li, 0, Math.max(1, Math.min(8, line.trains))));
      this.bridge.apply(cmd.setHeadway(li, Math.round(line.headwayMin * 60_000)));
    });
    this.selectedStation = null;
    this.selectedLine = null;
    this.refresh();
  }

  /** The next palette colour for a new line (deterministic by line index). */
  nextLineColor(): number {
    const n = this.bridge.linesView().length;
    return LINE_PALETTE[n % LINE_PALETTE.length];
  }
}
