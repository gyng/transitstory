// The hub controller: owns the SimBridge, map, and overlay; holds UI state (mode, tool,
// selection, line draft); rebuilds the overlay from authoritative sim views. UI/tools only
// call Game methods which emit Commands and refresh — they never mutate sim state directly.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import { CATCHMENT_M, LINE_PALETTE, SNAP_PX } from "./config";
import { lngLatToMm, mmToLngLat } from "./coords/geo";
import { cmd } from "./commands/codec";
import { buildOverlayLayers, colorToRgb, type RenderView } from "./render";
import type { SimBridge } from "./sim/SimBridge";

export type Mode = "build" | "run";
export type Tool = "select" | "station" | "line";

export class Game {
  mode: Mode = "build";
  tool: Tool = "station";
  selectedStation: number | null = null;
  selectedLine: number | null = null;
  hoveredStation: number | null = null;

  /** In-progress line draft (ordered station ids) + live cursor lng/lat (T11). */
  draft: number[] = [];
  cursor: [number, number] | null = null;

  /** Listeners notified after each refresh (panels/stats bind here). */
  onChange: (() => void)[] = [];

  constructor(
    readonly bridge: SimBridge,
    readonly map: MlMap,
    readonly overlay: MapboxOverlay,
  ) {}

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
    const ev = this.bridge.apply(cmd.createLine(this.nextLineColor()));
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
      color: colorToRgb(l.color),
      path: l.polylineMm.map(([x, y]) => mmToLngLat([x, y])),
    }));

    // Blueprint: draft station positions + live cursor (T11 populates draft).
    const blueprint: [number, number][] = this.draft.map((id) => {
      const s = stationsV[id];
      return mmToLngLat([s.xMm, s.yMm]);
    });
    if (this.cursor && blueprint.length >= 1) blueprint.push(this.cursor);

    return { stations, lines, catchments, blueprint, vehicles: [] };
  }

  refresh(): void {
    this.overlay.setProps({ layers: buildOverlayLayers(this.buildView()) });
    for (const cb of this.onChange) cb();
  }

  /** The next palette colour for a new line (deterministic by line index). */
  nextLineColor(): number {
    const n = this.bridge.linesView().length;
    return LINE_PALETTE[n % LINE_PALETTE.length];
  }
}
