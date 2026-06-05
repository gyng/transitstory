// The hub controller: owns the SimBridge, map, and overlay; holds UI state (mode, tool,
// selection, line draft); rebuilds the overlay from authoritative sim views. UI/tools only
// call Game methods which emit Commands and refresh — they never mutate sim state directly.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import type { Layer, PickingInfo } from "@deck.gl/core";
import { BUSY_WAITING, CATCHMENT_M, LINE_PALETTE, SNAP_PX, STARVED_WAITING } from "./config";
import { lngLatToMm, metersToLngLat, mmToLngLat } from "./coords/geo";
import { cmd } from "./commands/codec";
import { colorToRgb, topoLayers, vehicleLayer, type DemandPoint, type HazardDot, type RenderView, type VehicleDot, type WaitingDot } from "./render";
import { WHOLE_LINE } from "./commands/codec";
import { BUILD, Buildability } from "./sim/buildability";
import type { SimBridge } from "./sim/SimBridge";
import type { Event, PerLine, PerStation, Stats } from "./types";
import { lineTipHtml, MODES, modeIcon, stationTipHtml, vehicleTipHtml, type LineTip, type StationTip, type VehicleTip } from "./ui/react/shared";

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
  abandoned: 0,
  avgJourneyMs: 0,
  avgWaitMs: 0,
  avgLoadFactor: 0,
  coverageScore: 0,
  simHour: 6,
  period: "AM rush",
  demandMultiplier: 1,
  buildDifficulty: 0,
  economyEnabled: false,
  balance: 0,
  capitalSpent: 0,
  fareRevenue: 0,
  opexSpent: 0,
  perStation: [],
  perLine: [],
};

export type Mode = "build" | "run";
export type Tool = "select" | "station" | "line" | "bulldozer";

/** Shared card style for every inspector hover tooltip (station / train / line). */
const TOOLTIP_STYLE: Record<string, string> = {
  background: "rgba(255,255,255,.97)",
  color: "#1c2024",
  borderRadius: "8px",
  boxShadow: "0 2px 10px rgba(0,0,0,.18)",
  padding: "8px 10px",
};

export class Game {
  mode: Mode = "build";
  tool: Tool = "station";
  /** Active transport mode for new construction (0 rail,1 bus,2 ferry,3 air). The chorded
   *  bottom bar sets this; new lines are created with it and the buildability gate follows. */
  transport = 0;
  /** Which transport modes are enabled (settings panel). Disabled modes can't be selected
   *  in the chorded bar — a frontend gate; the sim is mode-agnostic about availability. */
  enabledModes = new Set([0, 1, 2, 3, 4]);
  /** Demand-heat map layer toggle + its source points (lng/lat + weight), set at boot. */
  showDemand = false;
  demandHeat: import("./render").DemandPoint[] = [];
  selectedStation: number | null = null;
  selectedLine: number | null = null;
  hoveredStation: number | null = null;
  /** Last rejection reason (e.g. afford-gate) for a transient toast; cleared on dismiss. */
  notice: string | null = null;

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
  /** Per-station snapshot indexed by station id (perStation is filtered, NOT index-aligned),
   *  rebuilt once per snapshot — O(1) lookups for the hover tooltip (no .find in the handler). */
  perStationById: Map<number, PerStation> = new Map();
  /** Per-line snapshot indexed by line id — O(1) lookups for the line + train hover tooltips. */
  perLineById: Map<number, PerLine> = new Map();

  constructor(
    readonly bridge: SimBridge,
    readonly map: MlMap,
    readonly overlay: MapboxOverlay,
    readonly build: Buildability = new Buildability(),
  ) {
    // Late-bind the station hover tooltip onto the overlay. setProps MERGES, so the per-frame
    // `layers` prop is untouched. pickingRadius == SNAP_PX so the tooltip's hit radius matches
    // the click/snap radius — one forgiving pick path. Content comes from game state below, not
    // from the raw pick coordinates.
    this.overlay.setProps({
      pickingRadius: SNAP_PX,
      getTooltip: (info: PickingInfo) => this.inspectTooltip(info),
    });
  }

  /** deck getTooltip handler — the unified inspector. Dispatches on which pickable layer was hit
   *  (stations / vehicles / lines), building content from the snapshot (not the raw pick) so each
   *  readout matches what the panels show. Z-order makes the hierarchy natural: a station on top,
   *  else a train between stops, else the line's own track. */
  private inspectTooltip(info: PickingInfo): { html: string; style: Record<string, string> } | null {
    if (!info || !info.layer) return null;
    let html: string | null = null;
    if (info.layer.id === "stations") {
      const obj = info.object as { id?: number } | undefined;
      const tip = obj && typeof obj.id === "number" ? this.stationTip(obj.id) : null;
      if (tip) html = stationTipHtml(tip);
    } else if (info.layer.id === "vehicles") {
      const tip = this.vehicleTip(info.index);
      if (tip) html = vehicleTipHtml(tip);
    } else if (info.layer.id === "lines") {
      const obj = info.object as { id?: number } | undefined;
      const tip = obj && typeof obj.id === "number" ? this.lineTip(obj.id) : null;
      if (tip) html = lineTipHtml(tip);
    }
    return html === null ? null : { html, style: TOOLTIP_STYLE };
  }

  /** Inspect a moving train (by its index in the vehicle SoA) — its line + live load factor.
   *  Onboard/capacity come from the `vehicleLoads` copy-out; the line's identity from the snapshot. */
  vehicleTip(index: number): VehicleTip | null {
    const lineIds = this.bridge.vehicleLineIds();
    if (index < 0 || index >= lineIds.length) return null;
    const lineId = lineIds[index];
    const ls = this.perLineById.get(lineId);
    const loads = this.bridge.vehicleLoads();
    return {
      lineName: ls?.name || `Line ${lineId + 1}`,
      lineColor: ls?.color ?? 0x888888,
      modeIcon: modeIcon(ls?.mode ?? 0),
      onboard: loads[index * 2] ?? 0,
      capacity: loads[index * 2 + 1] ?? 0,
    };
  }

  /** Inspect a line by id — its mode, ridership, load, and service shape from the snapshot. */
  lineTip(id: number): LineTip | null {
    const lv = this.bridge.linesView()[id];
    if (!lv || lv.removed) return null;
    const ls = this.perLineById.get(id);
    return {
      name: lv.name || `Line ${id + 1}`,
      color: lv.color,
      modeIcon: modeIcon(lv.mode),
      modeName: MODES[lv.mode]?.name ?? "Line",
      ridership: ls?.ridership ?? 0,
      loadFactor: ls?.loadFactor ?? 0,
      stops: ls?.stops ?? lv.stops.length,
      trains: ls?.trains ?? 0,
      headwayMin: ls ? Math.round(ls.headwayMs / 60000) : 0,
    };
  }

  /** Assemble the station inspect readout (drives the hover tooltip + the e2e hook). Returns
   *  placement truth only (verdict = null) in Build mode or before the station has a stats
   *  entry — never a confident "healthy" before any passenger has moved. */
  stationTip(id: number): StationTip | null {
    const sv = this.bridge.stationsView()[id];
    if (!sv || sv.removed) return null;
    const ps = this.perStationById.get(id);
    const lines = this.bridge
      .linesView()
      .filter((l) => !l.removed && l.stops.includes(id))
      .map((l) => ({ id: l.id, color: l.color, name: l.name }));
    const hasData = this.mode === "run" && ps !== undefined;
    const waiting = ps?.waiting ?? 0;
    return {
      id,
      name: sv.name,
      hasData,
      waiting,
      boardings: ps?.boardings ?? 0,
      alightings: ps?.alightings ?? 0,
      verdict: hasData ? this.starvation(waiting) : null,
      lines,
    };
  }

  /** Waiting-queue verdict — the single source for the tooltip word AND the ring colour. */
  starvation(waiting: number): "starved" | "busy" | "healthy" {
    if (waiting >= STARVED_WAITING) return "starved";
    if (waiting >= BUSY_WAITING) return "busy";
    return "healthy";
  }

  /** Drop any pinned station/line (Esc stage 3, click on empty map). */
  clearSelection(): void {
    if (this.selectedStation === null && this.selectedLine === null) return;
    this.selectedStation = null;
    this.selectedLine = null;
    this.refresh();
  }

  /** Capture an afford-gate (or other) rejection so the UI can flash it; returns the events. */
  private noteRejections(events: Event[]): Event[] {
    const r = events.find((e) => "Rejected" in e) as { Rejected: { reason: string } } | undefined;
    if (r) this.notice = r.Rejected.reason;
    return events;
  }

  /** Dismiss the transient notice (the toast auto-calls this); cheap onChange, no re-render of deck. */
  dismissNotice(): void {
    if (this.notice === null) return;
    this.notice = null;
    for (const cb of this.onChange) cb();
  }

  /** Set the build mode (0 Surface, 1 Elevated, 2 Tunnel) for a whole line (or one span). */
  setLineMode(line: number, mode: number, span: number = WHOLE_LINE): void {
    this.noteRejections(this.bridge.apply(cmd.setSegmentMode(line, span, mode)));
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
    for (const s of ids) this.noteRejections(this.bridge.apply(cmd.addStop(lineId, s)));
    this.cancelDraft();
    this.selectedLine = lineId;
    this.selectedStation = null;
    this.refresh();
    return lineId;
  }

  /** Assign trains and auto-suggest a sensible headway (round-trip / count) on first assign. */
  assignTrainset(line: number, count: number): void {
    const hadTrains = (this.bridge.linesView()[line] && this.lineTrains(line)) || 0;
    this.noteRejections(this.bridge.apply(cmd.assignTrainset(line, 0, count)));
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

  /** Reversible-by-construction undo: rebuild World from seed + log[..-1] (the frontend never
   *  splices sim state). Clears the now-possibly-stale selection and resyncs Build/Run. */
  undo(): boolean {
    if (!this.bridge.undo()) return false;
    this.cancelDraft();
    this.selectedStation = null;
    this.selectedLine = null;
    this.mode = this.bridge.stats().running ? "run" : "build";
    this.refresh();
    return true;
  }

  canUndo(): boolean {
    return this.bridge.log.length > 0;
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
      const next = [0, 1, 2, 3, 4].find((m) => this.enabledModes.has(m));
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

  /** Undo the last placed waypoint in the in-progress route (Backspace). */
  popDraft(): void {
    if (this.draft.length === 0) return;
    this.draft.pop();
    if (this.draft.length === 0) this.map.dragPan.enable();
    this.refresh();
  }

  /** Esc / right-click: a two-stage "stop" — first abandon the in-progress route, then (if
   *  nothing is pending) leave the build tool back to Select. Mirrors the CAD/Leaflet/NIMBY
   *  convention (Esc cancels the rubber-band, Esc again exits the tool). */
  stopBuilding(): void {
    if (this.draft.length > 0) {
      this.cancelDraft();
      this.refresh();
      return;
    }
    if (this.tool !== "select") {
      this.tool = "select";
      this.refresh();
      return;
    }
    // Stage 3: nothing building and already in Select → drop a pinned station/line.
    this.clearSelection();
  }

  /** True if the in-progress route is illegal for the active mode — a land mode (rail/bus)
   *  crossing water. Ferry/air cross water freely. Drives the red ghost + the build readout. */
  draftInvalid(): boolean {
    if (this.transport >= 2 || !this.build.loaded || this.draft.length < 1) return false;
    const sv = this.bridge.stationsView();
    const pts: [number, number][] = this.draft.map((id) => [sv[id].xMm, sv[id].yMm]);
    if (this.cursor) pts.push(lngLatToMm(this.cursor));
    for (let i = 1; i < pts.length; i++) {
      const [ax, ay] = pts[i - 1];
      const [bx, by] = pts[i];
      const len = Math.hypot(bx - ax, by - ay);
      const steps = Math.min(60, Math.max(1, Math.round(len / this.build.cellMm)));
      for (let k = 0; k <= steps; k++) {
        const x = ax + ((bx - ax) * k) / steps;
        const y = ay + ((by - ay) * k) / steps;
        if (this.build.classifyMm(x, y) === BUILD.WATER) return true;
      }
    }
    return false;
  }

  /** Live preview of the in-progress route for the build HUD (client-side geometry; the $ cost
   *  is filled in by the sim cost-preview query). Length ≈ straight-segment sum through the
   *  drafted stations plus the live cursor leg (the committed line is curve-smoothed). */
  draftPreview(): { stops: number; lengthKm: number; costM: number; invalid: boolean } {
    const sv = this.bridge.stationsView();
    const pts: [number, number][] = this.draft.map((id) => [sv[id].xMm, sv[id].yMm]);
    if (this.cursor) pts.push(lngLatToMm(this.cursor));
    let mm = 0;
    for (let i = 1; i < pts.length; i++) mm += Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
    // Authoritative construction cost (track only) from the core, in $millions; 0 until 2 stops.
    const cost = this.draft.length >= 2 ? this.bridge.previewLineCost(this.draft, this.transport, false) : 0;
    return { stops: this.draft.length, lengthKm: mm / 1_000_000, costM: cost / 1e6, invalid: this.draftInvalid() };
  }

  // --- geometry helpers ---

  /** Nearest station id to a screen pixel within `maxPx`, else null (screen-space snap). */
  nearestStation(px: number, py: number, maxPx = SNAP_PX): number | null {
    let best: number | null = null;
    let bestD = maxPx;
    for (const s of this.bridge.stationsView()) {
      if (s.removed) continue;
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

  /** Nearest non-removed line to a screen pixel (min distance from the pixel to the line's
   *  projected polyline), within `maxPx`. Used by the bulldozer to target a whole line. */
  nearestLine(px: number, py: number, maxPx = SNAP_PX + 6): number | null {
    let best: number | null = null;
    let bestD = maxPx;
    const segDist = (a: [number, number], b: [number, number]): number => {
      const dx = b[0] - a[0];
      const dy = b[1] - a[1];
      const l2 = dx * dx + dy * dy;
      let t = l2 > 0 ? ((px - a[0]) * dx + (py - a[1]) * dy) / l2 : 0;
      t = Math.max(0, Math.min(1, t));
      return Math.hypot(px - (a[0] + t * dx), py - (a[1] + t * dy));
    };
    for (const l of this.bridge.linesView()) {
      if (l.removed || l.polylineMm.length < 2) continue;
      const pts = l.polylineMm.map(([x, y]) => {
        const [lng, lat] = mmToLngLat([x, y]);
        const p = this.map.project([lng, lat]);
        return [p.x, p.y] as [number, number];
      });
      for (let i = 1; i < pts.length; i++) {
        const d = segDist(pts[i - 1], pts[i]);
        if (d <= bestD) {
          bestD = d;
          best = l.id;
        }
      }
    }
    return best;
  }

  /** Bulldoze under a screen pixel: remove the nearest station within snap radius, else the
   *  nearest line. One undoable Command each (undo = rebuild from seed + log[..-1]). */
  bulldozeAt(px: number, py: number): void {
    const st = this.nearestStation(px, py);
    if (st !== null) {
      this.noteRejections(this.bridge.apply(cmd.removeStation(st)));
      if (this.selectedStation === st) this.selectedStation = null;
      this.refresh();
      return;
    }
    const ln = this.nearestLine(px, py);
    if (ln !== null) {
      this.noteRejections(this.bridge.apply(cmd.removeLine(ln)));
      if (this.selectedLine === ln) this.selectedLine = null;
      this.refresh();
    }
  }

  // --- rendering ---

  private buildView(): RenderView {
    const stationsV = this.bridge.stationsView();
    const linesV = this.bridge.linesView();
    const highlight =
      this.selectedStation ?? this.hoveredStation ?? null;

    const stations = stationsV
      .filter((s) => !s.removed)
      .map((s) => {
        const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
        return { id: s.id, lng, lat, name: s.name, selected: s.id === this.selectedStation };
      });

    // A pinned (selected) station gets the filled catchment; a mere hover peek gets the
    // stroke-only fainter one (render.ts splits on `peek`).
    const peeking = this.selectedStation === null;
    const catchments =
      highlight === null
        ? []
        : stationsV
            .filter((s) => s.id === highlight)
            .map((s) => {
              const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
              return { lng, lat, radiusM: CATCHMENT_M, peek: peeking };
            });

    const lines = linesV
      .filter((l) => !l.removed)
      .map((l) => ({
        id: l.id,
        // A line with surface track over water renders red until elevated/tunnelled.
        color: l.crossesWaterSurface ? ([214, 40, 40] as [number, number, number]) : colorToRgb(l.color),
        path: l.polylineMm.map(([x, y]) => mmToLngLat([x, y])),
        mode: l.mode, // heavy/high-speed rail (4) gets distinct mainline styling
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

    // Pinned-station label (deck TextLayer): name, plus the live queue once it has data.
    let pinnedLabel: RenderView["pinnedLabel"];
    if (this.selectedStation !== null) {
      const s = stationsV[this.selectedStation];
      if (s && !s.removed) {
        const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
        const tip = this.stationTip(this.selectedStation);
        const text = tip && tip.hasData ? `${s.name} · ${Math.round(tip.waiting)} waiting` : s.name;
        pinnedLabel = { lng, lat, text };
      }
    }

    const demand = this.demandPoints();
    return {
      stations,
      lines,
      catchments,
      blueprint,
      vehicles: [],
      waiting,
      hazards,
      demand,
      blueprintInvalid: this.draftInvalid(),
      pinnedLabel,
      selectedLine: this.selectedLine,
    };
  }

  /** Travel-demand heat points with a SERVED flag (inside the catchment union of placed
   *  stations). Memoized on the station id-set + the toggle — recomputed only when topology
   *  changes or the layer turns on, NEVER per frame / per pointer move. Unmet cells glow warm
   *  (the gap to fill), served cells fade cool (you've got it). Approximate client-side union —
   *  the sim-true per-cell served fraction is the demand-model pass's concern. */
  private demandSig = "";
  private demandView: DemandPoint[] = [];
  private demandCellsMm: [number, number][] | null = null;
  private demandPoints(): DemandPoint[] {
    if (!this.showDemand) return [];
    const stationsV = this.bridge.stationsView().filter((s) => !s.removed);
    const sig = stationsV.map((s) => s.id).join(",");
    if (sig === this.demandSig && this.demandView.length === this.demandHeat.length) return this.demandView;
    this.demandSig = sig;
    if (!this.demandCellsMm) this.demandCellsMm = this.demandHeat.map((c) => lngLatToMm([c.lng, c.lat]));
    const cells = this.demandCellsMm;
    const stMm = stationsV.map((s) => [s.xMm, s.yMm] as [number, number]);
    const rMm = CATCHMENT_M * 1000;
    const r2 = rMm * rMm;
    this.demandView = this.demandHeat.map((c, i) => {
      const [cx, cy] = cells[i];
      let served = false;
      for (const [sx, sy] of stMm) {
        const dx = sx - cx;
        const dy = sy - cy;
        if (dx * dx + dy * dy <= r2) {
          served = true;
          break;
        }
      }
      return { lng: c.lng, lat: c.lat, weight: c.weight, served };
    });
    return this.demandView;
  }

  /** Push a fresh stats snapshot (called on the ~3 Hz UI throttle) and re-render halos. */
  setStats(s: Stats): void {
    this.lastStats = s;
    this.perStationById = new Map(s.perStation.map((ps) => [ps.stationId, ps]));
    this.perLineById = new Map(s.perLine.map((l) => [l.lineId, l]));
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
