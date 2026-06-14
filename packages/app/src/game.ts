// The hub controller: owns the SimBridge, map, and overlay; holds UI state (mode, tool,
// selection, line draft); rebuilds the overlay from authoritative sim views. UI/tools only
// call Game methods which emit Commands and refresh — they never mutate sim state directly.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import type { Layer, PickingInfo } from "@deck.gl/core";
import { BUSY_WAITING, CATCHMENT_M, DETAIL_ZOOM, LINE_PALETTE, SNAP_PX, STARVED_WAITING, TICK_MS } from "./config";
import { lngLatToMm, metersToLngLat, metersToLngLatInto, mmToLngLat } from "./coords/geo";
import { cmd } from "./commands/codec";
import { armyLayer, colorToRgb, peepLayer, topoLayers, vehicleLayers, type DecadenceAnchor, type DemandPoint, type DesireArc, type HazardDot, type ReachDot, type RenderView, type ResourceMarker, type ShedHex, type TerrainCell, type TideCell, type TownMarker, type VehicleDot, type WaitingDot } from "./render";
import { audio } from "./fx/audio";
import { Effects } from "./fx/effects";
import { createSky, type Sky } from "./map/sky";
import { WHOLE_LINE } from "./commands/codec";
import { BUILD, Buildability } from "./sim/buildability";
import type { SimBridge } from "./sim/SimBridge";
import type { Event, PerLine, PerStation, Stats } from "./types";
import { lineTipHtml, MODE_SPECS, MODES, modeIcon, SIM_MS_PER_CLOCK_MIN, stationTipHtml, vehicleTipHtml, type LineTip, type StationTip, type VehicleTip } from "./ui/react/shared";
import { meanStopQueue } from "./ui/react/lineEconomics";

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
  simDay: 0,
  demandOriginTotal: 0,
  buildDifficulty: 0,
  economyEnabled: false,
  balance: 0,
  capitalSpent: 0,
  fareRevenue: 0,
  opexSpent: 0,
  perStation: [],
  perLine: [],
  ruleset: "transit",
  tribute: 0,
  decadence: 0,
  decadencePct: 0,
  townsCaptured: 0,
  armyCount: 0,
  realmLost: false,
  techUnlocked: 0,
};

export type Mode = "build" | "run";
export type Tool = "select" | "station" | "line" | "bulldozer" | "barracks" | "bounty";

/** Standard bounty posted per click of the bounty tool — baits AI legions toward that town. */
const BOUNTY_AMOUNT = 1000;

/** Shared card style for every inspector hover tooltip (station / train / line). */
const TOOLTIP_STYLE: Record<string, string> = {
  background: "rgba(255,255,255,.97)",
  color: "#1c2024",
  borderRadius: "8px",
  boxShadow: "0 2px 10px rgba(0,0,0,.18)",
  padding: "8px 10px",
};

const EMPTY_U32 = new Uint32Array(0);

/** Right-click context-menu state (surfaced through the UI slice; rendered by <ContextMenu>). The
 *  menu offers Inspect / Bulldoze on the resolved target — station, line, or empty-map view tools. */
export interface ContextMenuState {
  /** Screen position (≈ clientX/Y; the full-screen map sits at the viewport origin). */
  x: number;
  y: number;
  lngLat: { lng: number; lat: number };
  kind: "station" | "line" | "empty";
  /** Station/line id; -1 for an empty-map menu. */
  id: number;
}

export class Game {
  mode: Mode = "build";
  tool: Tool = "station";
  /** The loaded city's ruleset ("transit" | "arcadia"), set in boot from the manifest. Drives the
   *  mode-aware chrome (e.g. the fantasy build tools) without per-frame stats reads. */
  ruleset = "transit";
  /** Active transport mode for new construction (0 rail,1 bus,2 ferry,3 air). The chorded
   *  bottom bar sets this; new lines are created with it and the buildability gate follows. */
  transport = 0;
  /** Which transport modes are enabled (settings panel). Disabled modes can't be selected
   *  in the chorded bar — a frontend gate; the sim is mode-agnostic about availability. */
  enabledModes = new Set([0, 1, 2, 3, 4]);
  /** Demand-heat map layer toggle + its source points (lng/lat + weight), set at boot. */
  showDemand = false;
  demandHeat: import("./render").DemandPoint[] = [];
  /** Demand-grid cell pitch (m), set at boot — sizes the demand-heat hexagons so they tile it. */
  demandCellM = 400;
  /** Accessibility "Reach" overlay toggle — when on + a station selected, shades reachable
   *  stations by transit travel time from it (the isochrone). Read via a pure core query. */
  showReach = false;
  /** "Roads" overlay toggle — paints the ROAD corridors where buses run cheap + fast. Also
   *  auto-shown while drawing a Bus line (so you see where to route it). Memoized lng/lat below. */
  showRoads = false;
  private roadCells: import("./render").RoadCell[] | null = null;
  /** Baked fantasy terrain hexes (lng/lat + biome code) — the map itself. Set once at load from the
   *  city's buildability raster (fantasy only; empty for transit cities), so the array identity is
   *  stable across frames (no per-frame rebuild). `terrainCellM` = the hex circumradius in metres. */
  terrain: TerrainCell[] = [];
  terrainCellM = 0;
  /** Baked fantasy resource nodes (lng/lat + kind + yield) — the supply-chain sources. Set once at load
   *  from the manifest's supplyGraph (fantasy only; empty for transit). Stable identity across frames. */
  resources: ResourceMarker[] = [];
  /** Baked fantasy towns (sinks + conquest targets) + the far-edge decadence reservoir anchors. Set once
   *  at load from the manifest's supplyGraph (fantasy only; empty for transit). Stable identity. */
  towns: TownMarker[] = [];
  decadenceAnchors: DecadenceAnchor[] = [];
  /** Toggle the individual-rider "peep" dots (Cities:Skylines-style). On by default; only drawn
   *  while running (peeps are the in-transit passenger set). The dots are a determinism-free
   *  render-only read-out from the core — no sim state, no Command. */
  showPeeps = true;
  selectedStation: number | null = null;
  selectedLine: number | null = null;
  hoveredStation: number | null = null;
  /** Pre-commit snap candidate: the station the next click would chain (line tool) or demolish
   *  (bulldozer). Set by the pointer per mousemove; rendered as a ring BEFORE the click commits. */
  snapStation: number | null = null;
  /** Last rejection reason (e.g. afford-gate) for a transient toast; cleared on dismiss. */
  notice: string | null = null;

  /** In-progress line draft (ordered station ids) + live cursor lng/lat (T11). */
  draft: number[] = [];
  cursor: [number, number] | null = null;
  /** When set, the draft EXTENDS this committed line instead of creating a new one: draft[0] is
   *  the seed terminus (already on the line), the rest commit as AddStops (append at the tail,
   *  insert-at-0 from the head). The ghost dashes in the line's own colour. */
  extendTarget: { line: number; head: boolean } | null = null;
  /** Per-span control points (waypoints) that BEND the draft's track, in mm. `draftWaypoints[i]`
   *  shapes the span between draft stop i and i+1; client-side while drawing, then committed as a
   *  SetLineWaypoints command. Kept length-aligned to the spans (draft.length - 1). */
  draftWaypoints: [number, number][][] = [];
  /** The control point currently being dragged (span + index into `draftWaypoints[span]`). */
  draggingHandle: { span: number; index: number } | null = null;

  /** Listeners notified after each refresh (panels/stats bind here). */
  onChange: (() => void)[] = [];

  /** Cached topology layers (stable identity across frames; rebuilt only on refresh). */
  private below: Layer[] = [];
  private above: Layer[] = [];
  /** Spatial juice canvas (ripples / connect-flash / throbs). Client-side acknowledgement only —
   *  driven by the existing GameLoop rAF, never a deck rebuild or a sim tick. */
  effects!: Effects;
  /** Day/night mood wash over the basemap (driven by sim hour off the ~3 Hz stats slice). */
  readonly sky: Sky;
  /** Per-station boardings from the previous stats snapshot — to emit a board-burst on the delta. */
  private prevBoardings: Map<number, number> = new Map();
  /** Cached last peep sweep (lng/lat interleaved + paired citizen ids) for click-to-inspect. */
  private peepXY: Float32Array = new Float32Array(0);
  private peepCit: Uint32Array = EMPTY_U32;
  /** Right-click context-menu state, or null when closed (read by <ContextMenu> via the UI slice). */
  contextMenu: ContextMenuState | null = null;

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
    this.effects = new Effects(map);
    this.sky = createSky(map.getContainer());
  }

  /** Per-frame spatial-juice draw — called from the GameLoop rAF (build + run) with its timestamp.
   *  Pure client-side canvas tween; touches no sim state and rebuilds no deck layer. Guarded: juice
   *  is non-essential, so a transient canvas/projection error must NEVER bubble up and stop the rAF
   *  reschedule (which would freeze the sim). One failed frame is dropped, never the game loop. */
  drawEffects(now: number): void {
    try {
      this.effects.draw(now);
    } catch {
      /* swallow — a dropped juice frame is invisible; a stopped render loop is not */
    }
  }

  /** lng/lat of a station by id (for anchoring an effect), or null if missing/removed. */
  private stationLngLat(id: number): [number, number] | null {
    const s = this.bridge.stationsView()[id];
    if (!s || s.removed) return null;
    return mmToLngLat([s.xMm, s.yMm]);
  }

  /** deck getTooltip handler — the unified inspector. Dispatches on which pickable layer was hit
   *  (stations / vehicles / lines). The tooltip DOM is game-owned (not deck's): deck only re-runs
   *  getTooltip on pointer moves, which froze the numbers while a player watched one station —
   *  owning the element lets `setStats` re-render the open tooltip on the ~3 Hz slice, so a
   *  watched queue counts live. Always returns null to deck (suppresses its built-in tooltip). */
  private inspectTooltip(info: PickingInfo): null {
    if (!info || !info.layer) {
      this.tipTarget = null;
    } else if (info.layer.id === "stations") {
      const obj = info.object as { id?: number } | undefined;
      this.tipTarget = obj && typeof obj.id === "number" ? { kind: "station", id: obj.id, x: info.x, y: info.y } : null;
    } else if (info.layer.id === "vehicles") {
      this.tipTarget = info.index >= 0 ? { kind: "vehicle", id: info.index, x: info.x, y: info.y } : null;
    } else if (info.layer.id === "lines") {
      const obj = info.object as { id?: number } | undefined;
      this.tipTarget = obj && typeof obj.id === "number" ? { kind: "line", id: obj.id, x: info.x, y: info.y } : null;
    } else {
      this.tipTarget = null;
    }
    this.renderTip();
    return null;
  }

  /** What the inspector tooltip is currently showing (kind + id + anchor px), or null. */
  private tipTarget: { kind: "station" | "vehicle" | "line"; id: number; x: number; y: number } | null = null;
  private tipEl: HTMLDivElement | null = null;

  /** (Re)draw the inspector tooltip from current game state — called on hover changes AND from
   *  `setStats`, so an open tooltip's numbers stay live instead of freezing at hover time. */
  private renderTip(): void {
    const t = this.tipTarget;
    let html: string | null = null;
    if (t?.kind === "station") {
      const tip = this.stationTip(t.id);
      if (tip) html = stationTipHtml(tip);
    } else if (t?.kind === "vehicle") {
      const tip = this.vehicleTip(t.id);
      if (tip) html = vehicleTipHtml(tip);
    } else if (t?.kind === "line") {
      const tip = this.lineTip(t.id);
      if (tip) html = lineTipHtml(tip);
    }
    if (html === null || t === null) {
      if (this.tipEl) this.tipEl.style.display = "none";
      return;
    }
    if (!this.tipEl) {
      this.tipEl = document.createElement("div");
      Object.assign(this.tipEl.style, TOOLTIP_STYLE, {
        position: "absolute",
        pointerEvents: "none",
        zIndex: "9",
        font: "12px system-ui,sans-serif",
        whiteSpace: "nowrap",
      });
      this.map.getContainer().appendChild(this.tipEl);
    }
    this.tipEl.innerHTML = html;
    this.tipEl.style.display = "block";
    this.tipEl.style.left = `${t.x + 14}px`;
    this.tipEl.style.top = `${t.y + 14}px`;
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
      headwayMin: ls ? Math.round(ls.headwayMs / SIM_MS_PER_CLOCK_MIN) : 0,
    };
  }

  /** Mean live queue across a line's stops — the platform-pressure input to lineSatisfaction
   *  (see lineEconomics.meanStopQueue). One join point so the roster, editor, and dashboard all
   *  derive it identically. */
  lineQueue(id: number): number {
    const lv = this.bridge.linesView()[id];
    if (!lv || lv.removed) return 0;
    return meanStopQueue(lv.stops, this.perStationById);
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
      .map((l) => ({ id: l.id, color: l.color, name: l.name, load: this.perLineById.get(l.id)?.loadFactor }));
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
      homes: ps?.demandOrigin ?? 0,
      jobs: ps?.demandDest ?? 0,
      serving: ps?.serving ?? lines.length,
      denied: ps?.denied ?? 0,
      abandoned: ps?.abandoned ?? 0,
      // Garrison only for TOWN sinks (dest > origin) — a source isn't a conquest target. 0 ⇒ not shown.
      garrison: ps && ps.demandDest > ps.demandOrigin ? ps.townResistance ?? 0 : 0,
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
    if (r) {
      this.notice = r.Rejected.reason;
      audio.alert(); // a gated Command's audible echo (pairs with the toast)
    }
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

  /** Track type (0=Double,1=Single) for the whole line (P2). Single is cheaper but lower capacity
   *  (opposing trains must meet at passing places). */
  setLineTrack(line: number, track: number): void {
    this.noteRejections(this.bridge.apply(cmd.setSegmentTrack(line, WHOLE_LINE, track)));
    this.refresh();
  }

  /** Build mode (0/1/2) for a whole branch's own track. */
  setBranchMode(line: number, branch: number, mode: number): void {
    this.noteRejections(this.bridge.apply(cmd.setBranchTrack(line, branch, mode)));
    this.refresh();
  }

  /** Bulldoze a branch off a line (the trunk + other branches stay). */
  removeBranch(line: number, branch: number): void {
    this.noteRejections(this.bridge.apply(cmd.removeBranch(line, branch)));
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
    if (id >= 0) {
      this.effects.ripple(lng, lat); // selection-blue placement ring
      audio.place();
    }
    this.refresh();
    return id;
  }

  /** Place a BARRACKS (fantasy) — a node that fields AI legions. Mirrors placeStation but emits the
   *  fantasy command; the transit ruleset rejects it (no node created), so the tool is fantasy-only. */
  placeBarracks(lng: number, lat: number): number {
    const [x_mm, y_mm] = lngLatToMm([lng, lat]);
    const events = this.bridge.apply(cmd.placeBarracks(x_mm, y_mm));
    const placed = events.find((e) => "BarracksPlaced" in e) as { BarracksPlaced: { id: number } } | undefined;
    const id = placed ? placed.BarracksPlaced.id : -1;
    if (id >= 0) {
      this.selectedStation = id;
      this.effects.ripple(lng, lat);
      audio.place();
    }
    this.refresh();
    return id;
  }

  /** Post a BOUNTY on a town (fantasy, the Majesty steering lever) — baits AI legions toward it. The
   *  bounty tool resolves a click to the nearest town and calls this; the core sets `bounty[town]`. */
  postBounty(stationId: number, amount: number = BOUNTY_AMOUNT): void {
    this.bridge.apply(cmd.postBounty(stationId, amount));
    this.refresh();
  }

  /** Buy a tech upgrade (fantasy, S11) — spends tribute, sets the tech's bit. The core afford-gates +
   *  rejects a repeat/unknown/broke unlock (no mutation), so the UI can fire optimistically and resync
   *  from the next snapshot. The tech panel calls this; `tech` is an index into the tech table. */
  unlockTech(tech: number): void {
    this.bridge.apply(cmd.unlockTech(tech));
    this.refresh();
  }

  /** Commit a line through the given ordered station ids (CreateLine + AddStop*). The
   *  interactive draw gesture (T11) and the test hook both funnel here. All-or-nothing: if any
   *  AddStop is rejected (the afford-gate mid-sequence), the whole line is rolled back with a
   *  RemoveLine — a committed network never silently differs from the blueprint that was drawn.
   *  (The log stays append-only; the rollback is itself a Command.) */
  drawLineByIds(ids: number[]): number {
    if (ids.length < 2) return -1;
    const ev = this.bridge.apply(cmd.createLine(this.nextLineColor(), null, false, this.transport));
    const created = ev.find((e) => "LineCreated" in e) as
      | { LineCreated: { id: number } }
      | undefined;
    const lineId = created ? created.LineCreated.id : this.bridge.linesView().length - 1;
    let rejected = false;
    for (const s of ids) {
      const evs = this.noteRejections(this.bridge.apply(cmd.addStop(lineId, s)));
      if (evs.some((e) => "Rejected" in e)) rejected = true;
    }
    if (rejected) {
      this.bridge.apply(cmd.removeLine(lineId));
      this.cancelDraft();
      this.refresh();
      return -1;
    }
    this.cancelDraft();
    this.selectedLine = lineId;
    this.selectedStation = null;
    // Connect flash: the committed line lights up along its length (its own colour) + a chime.
    const lv = this.bridge.linesView()[lineId];
    if (lv && lv.polylineMm.length >= 2) {
      this.effects.connectFlash(
        lv.polylineMm.map(([x, y]) => mmToLngLat([x, y])),
        colorToRgb(lv.color).join(","),
      );
      audio.connect();
    }
    this.refresh();
    return lineId;
  }

  /** Assign trains and auto-suggest a sensible headway (round-trip / count) on first assign.
   *  `spec` defaults to the line's CURRENT roster entry so count changes (incl. the headway
   *  slider's re-derive) never silently reset a chosen aircraft back to the default. */
  assignTrainset(line: number, count: number, spec: number = this.lineSpec(line)): void {
    const hadTrains = (this.bridge.linesView()[line] && this.lineTrains(line)) || 0;
    this.noteRejections(this.bridge.apply(cmd.assignTrainset(line, spec, count)));
    if (hadTrains === 0) {
      this.bridge.apply(cmd.setHeadway(line, this.suggestHeadwayMs(line, count)));
    }
    this.refresh();
  }

  /** Pick a roster entry (AIR's aircraft ladder) for a line, keeping its train count. */
  setAircraft(line: number, spec: number): void {
    const count = Math.max(1, this.lineTrains(line));
    this.noteRejections(this.bridge.apply(cmd.assignTrainset(line, spec, count)));
    this.refresh();
  }

  /** The line's current roster entry (0 = mode default; survives count re-assignments). */
  private lineSpec(line: number): number {
    return this.bridge.stats().perLine.find((l) => l.lineId === line)?.trainsetSpec ?? 0;
  }

  /** The Headway slider is the lever: derive the train count from headway (round-trip / H)
   *  and re-assign, keeping count↔headway consistent (the dispatcher uses count). */
  setEconomy(enabled: boolean): void {
    this.bridge.apply(cmd.setEconomy(enabled));
    this.refresh();
  }

  /** Demand model: true = seed-derived citizen agents (home/work commuters), false = gravity flow.
   *  Command-sourced (in the save + deterministic). Tracked client-side for the Settings toggle. */
  agentDemand = false;
  setDemandMode(agents: boolean): void {
    this.agentDemand = agents;
    this.bridge.apply(cmd.setDemandMode(agents));
    this.refresh();
  }

  /** The citizen currently being followed (their live journey is shown + located), or null. */
  followedCitizen: number | null = null;
  setFollowed(citizenId: number): void {
    this.followedCitizen = citizenId;
    for (const cb of this.onChange) cb();
  }
  clearFollowed(): void {
    if (this.followedCitizen === null) return;
    this.followedCitizen = null;
    for (const cb of this.onChange) cb();
  }

  setHeadwayMs(line: number, ms: number): void {
    this.bridge.apply(cmd.setHeadway(line, ms));
    const count = Math.max(1, Math.min(8, Math.round(this.roundTripMs(line) / ms)));
    this.bridge.apply(cmd.assignTrainset(line, this.lineSpec(line), count));
    this.refresh();
  }

  private lineTrains(line: number): number {
    return this.bridge.stats().perLine.find((l) => l.lineId === line)?.trains ?? 0;
  }

  /** Estimated round-trip travel time (sim-ms): out-and-back run + dwell at each stop, using the
   *  LINE'S mode spec from the shared mirror (the old version hardcoded the rail spec for every
   *  mode, so bus/ferry/heavy headway suggestions were estimated at rail speeds). */
  roundTripMs(line: number): number {
    const l = this.bridge.linesView()[line];
    if (!l) return 20_000;
    let lenMm = 0;
    for (let i = 1; i < l.polylineMm.length; i++) {
      const [ax, ay] = l.polylineMm[i - 1];
      const [bx, by] = l.polylineMm[i];
      lenMm += Math.hypot(bx - ax, by - ay);
    }
    const spec = MODE_SPECS[l.mode] ?? MODE_SPECS[0];
    const oneWayMs = (lenMm / spec.vMaxMmS) * 1000 + l.stops.length * spec.dwellMs;
    return 2 * oneWayMs;
  }

  /** Round-trip / train count, clamped to the sim's headway bounds (mirrored: 1–60 clock-min). */
  suggestHeadwayMs(line: number, count: number): number {
    return Math.max(2_000, Math.min(120_000, Math.round(this.roundTripMs(line) / Math.max(1, count))));
  }

  setMode(mode: Mode): void {
    this.mode = mode;
    this.bridge.apply(cmd.setRunning(mode === "run"));
    if (mode === "run") this.cancelDraft();
    else this.effects.clear(); // back to Build — drop any lingering run-mode throbs/bursts
    audio.toggle(mode === "run");
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

  /** Redo the most recently undone command (forward navigation; any fresh command forks it). */
  redo(): boolean {
    if (!this.bridge.redo()) return false;
    this.cancelDraft();
    this.mode = this.bridge.stats().running ? "run" : "build";
    this.refresh();
    return true;
  }

  canRedo(): boolean {
    return this.bridge.canRedo();
  }

  setTool(tool: Tool): void {
    if (this.tool === "line" && tool !== "line") this.cancelDraft();
    if (tool !== this.tool) audio.tick();
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
    audio.tick();
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

  /** Toggle the accessibility "Reach" overlay (shades reachable stations by travel time from the
   *  selected one). Opt-in so it never piles onto the default selection overlay. */
  setShowReach(on: boolean): void {
    this.showReach = on;
    this.refresh();
  }

  /** Toggle the "Roads" overlay (the ROAD corridors where buses are cheap + fast). */
  setShowRoads(on: boolean): void {
    this.showRoads = on;
    this.refresh();
  }

  /** Toggle the individual-rider "peep" dots. */
  setShowPeeps(on: boolean): void {
    this.showPeeps = on;
    this.refresh();
  }

  /** Build the binary-attribute peep layer at interpolation `alpha`, or null when off / not running
   *  / no one in transit. The core fills positions (metres) + RGBA in one capped sweep; we convert
   *  metres→lng/lat IN PLACE (geo.ts, the one coordinate crossing) into the same fresh buffer and
   *  hand deck binary attributes — no per-object accessors, so thousands of dots cost ~nothing. */
  peepLayerAt(alpha: number): Layer | null {
    // Level-of-detail: peeps are micro-texture — hidden at the city-overview zoom (where they'd be a
    // swarm of flashing dots) and revealed only once zoomed in. Skips the buffer copy when hidden.
    if (!this.showPeeps || this.mode !== "run" || this.map.getZoom() < DETAIL_ZOOM) {
      this.peepCit = EMPTY_U32; // not visible → not pickable
      return null;
    }
    const xy = this.bridge.peepPositions(alpha, TICK_MS); // interleaved metres (fresh each frame)
    const count = xy.length >> 1;
    if (count === 0) {
      this.peepCit = EMPTY_U32;
      return null;
    }
    for (let i = 0; i < xy.length; i += 2) metersToLngLatInto(xy[i], xy[i + 1], xy, i); // metres→lng/lat in place
    // Cache the (lng/lat) positions + paired citizen ids so a click can map to the nearest peep's
    // rider (nearestPeep). Same sweep feeds the layer + the pick — they can't drift.
    this.peepXY = xy;
    this.peepCit = this.bridge.peepCitizens();
    return peepLayer(xy, this.bridge.peepColors(), count);
  }

  /** Citizen id of the nearest pickable (non-anonymous) peep within `maxPx` screen pixels of
   *  (px,py), or null. Projects the cached last-sweep peep positions — keeps the peep layer
   *  non-pickable (60fps budget); the loop only runs on a click/hover, not per frame. */
  nearestPeep(px: number, py: number, maxPx = 12): number | null {
    const xy = this.peepXY;
    const cit = this.peepCit;
    let best = -1;
    let bestD2 = maxPx * maxPx;
    for (let i = 0; i < cit.length; i++) {
      if (cit[i] === 0xffffffff) continue; // anonymous gravity rider — not followable
      const p = this.map.project([xy[i * 2], xy[i * 2 + 1]]);
      const dx = p.x - px;
      const dy = p.y - py;
      const d2 = dx * dx + dy * dy;
      if (d2 < bestD2) {
        bestD2 = d2;
        best = i;
      }
    }
    return best >= 0 ? cit[best] : null;
  }

  /** ROAD-cell centres in lng/lat for the overlay — derived once from the buildability raster
   *  (the same data the cost/speed gate uses), memoized so it never recomputes per frame. */
  private roadPoints(): import("./render").RoadCell[] {
    if (this.roadCells === null) {
      const cm = this.build.cellMm;
      this.roadCells = this.build.loaded
        ? this.build.cellsMm(BUILD.ROAD).map(([x, y]) => {
            // Local built-up density (BUILT cells in the 3×3) — mirrors the sim's congestion input.
            let density = 0;
            for (let ddx = -1; ddx <= 1; ddx++)
              for (let ddy = -1; ddy <= 1; ddy++)
                if (this.build.classifyMm(x + ddx * cm, y + ddy * cm) === BUILD.BUILT) density++;
            const [lng, lat] = mmToLngLat([x, y]);
            return { lng, lat, density };
          })
        : [];
    }
    return this.roadCells;
  }

  selectStation(id: number | null): void {
    this.selectedStation = id;
    this.selectedLine = null;
    if (id !== null) {
      const ll = this.stationLngLat(id);
      if (ll) this.effects.pulse(ll[0], ll[1]);
    }
    this.refresh();
  }

  selectLine(id: number | null): void {
    this.selectedLine = id;
    this.selectedStation = null;
    this.refresh();
  }

  cancelDraft(): void {
    this.draft = [];
    this.draftWaypoints = [];
    this.draggingHandle = null;
    this.cursor = null;
    this.extendTarget = null;
    this.map.dragPan.enable();
  }

  /** Begin extending a committed line from one of its termini (`head` = from the first stop).
   *  Seeds the draft with that terminus so the ghost rubber-bands from it; chaining and commit
   *  then reuse the whole draft pipeline. Loop lines have no termini — refused. Entered from the
   *  Editor's Extend buttons or by pressing a terminus of the SELECTED line with the line tool. */
  startExtend(lineId: number, head: boolean): boolean {
    const lv = this.bridge.linesView()[lineId];
    if (!lv || lv.removed || lv.loopLine || lv.stops.length === 0) return false;
    if (this.mode !== "build") this.setMode("build"); // drawing happens behind the Build wall
    this.cancelDraft();
    this.tool = "line";
    this.transport = lv.mode; // the ghost's legality (water rules etc.) follows the LINE's mode
    this.extendTarget = { line: lineId, head };
    this.draft = [head ? lv.stops[0] : lv.stops[lv.stops.length - 1]];
    this.draftWaypoints = [];
    this.selectedLine = lineId;
    this.selectedStation = null;
    this.map.dragPan.disable();
    this.refresh();
    return true;
  }

  // --- interactive line drawing (snap → blueprint → commit) ---

  /** Append a snapped station to the in-progress line; disables dragPan on first point. Each new
   *  stop opens a fresh (initially straight) span for control points. */
  extendDraft(stationId: number): void {
    if (this.draft.length === 0) this.map.dragPan.disable();
    if (this.draft[this.draft.length - 1] !== stationId) {
      if (this.draft.length >= 1) this.draftWaypoints.push([]); // new span: stop n-1 → n
      this.draft.push(stationId);
    }
    this.refresh();
  }

  /** Commit the blueprint as one line (re-enables dragPan). Needs >= 2 stops; any bent spans go
   *  out as a single SetLineWaypoints right after the line is created (so undo is one step).
   *  Pre-flight gates run BEFORE anything is sent: an invalid (over-water) or unaffordable draft
   *  stays on screen with a notice instead of committing — the player fixes or cancels, never
   *  discovers post-commit that the network differs from the blueprint. */
  commitDraft(): void {
    if (this.draft.length >= 2) {
      const p = this.draftPreview();
      if (p.invalid) {
        this.notice = "Route crosses water — elevate, tunnel, or use a ferry";
        audio.alert();
        this.refresh();
        return;
      }
      if (p.shortM > 0) {
        this.notice = `Not enough money — $${Math.ceil(p.shortM)}M short`;
        audio.alert();
        this.refresh();
        return;
      }
    }
    const ids = [...this.draft];
    const wps = this.draftWaypoints.map((span) => span.map(([x, y]) => [x, y] as [number, number]));
    const extend = this.extendTarget;
    this.map.dragPan.enable();
    this.cursor = null;
    this.draft = [];
    this.draftWaypoints = [];
    this.draggingHandle = null;
    this.extendTarget = null;
    if (extend && ids.length >= 2) {
      this.commitExtension(extend, ids.slice(1)); // ids[0] is the seed terminus, already a stop
      return;
    }
    if (ids.length >= 2) {
      const lineId = this.drawLineByIds(ids);
      // drawLineByIds is all-or-nothing (rolls back on any rejection), so a returned line has
      // every drafted stop and the per-span waypoints line up 1:1.
      if (lineId >= 0 && wps.some((s) => s.length > 0)) {
        this.noteRejections(this.bridge.apply(cmd.setLineWaypoints(lineId, wps)));
        this.refresh();
      }
    } else {
      this.refresh();
    }
  }

  /** Send the extension's AddStops: appended at the tail, or inserted at index 0 one by one from
   *  the head (each insert lands before the previous, so the drawn order is preserved outward).
   *  No new Command vocabulary — `AddStop{after}` covered this all along. On a mid-sequence
   *  rejection (afford-gate) we stop sending: unlike a fresh line there is no RemoveStop to roll
   *  back with, and a shorter extension is still a contiguous, valid line — the notice says why. */
  private commitExtension(extend: { line: number; head: boolean }, newStops: number[]): void {
    for (const s of newStops) {
      const evs = this.noteRejections(
        this.bridge.apply(cmd.addStop(extend.line, s, extend.head ? 0 : null)),
      );
      if (evs.some((e) => "Rejected" in e)) break;
    }
    this.selectedLine = extend.line;
    const lv = this.bridge.linesView()[extend.line];
    if (lv && lv.polylineMm.length >= 2) {
      this.effects.connectFlash(
        lv.polylineMm.map(([x, y]) => mmToLngLat([x, y])),
        colorToRgb(lv.color).join(","),
      );
      audio.connect();
    }
    this.refresh();
  }

  /** Undo the last placed stop in the in-progress route (Backspace), with its span's waypoints.
   *  While extending, the seed terminus (draft[0]) is part of the committed line — never popped;
   *  backing past the last NEW stop just leaves the extension armed at its seed. */
  popDraft(): void {
    const floor = this.extendTarget ? 1 : 0;
    if (this.draft.length <= floor) return;
    this.draft.pop();
    this.draftWaypoints.pop(); // drop the span that ended at the removed stop
    if (this.draft.length === 0) this.map.dragPan.enable();
    this.refresh();
  }

  /** Insert a station as a stop on a committed line, at the span it sits closest to (projection
   *  onto each consecutive stop pair; for a loop line the closing span counts too). One AddStop —
   *  one undo step. The dispatcher redistributes vehicles on the next tick. */
  insertStopOnLine(lineId: number, stationId: number): boolean {
    const lv = this.bridge.linesView()[lineId];
    const sv = this.bridge.stationsView();
    const st = sv[stationId];
    if (!lv || lv.removed || !st || st.removed || lv.stops.includes(stationId) || lv.stops.length < 2) return false;
    // Point-to-segment distance in mm against each span; the winner is where the stop belongs.
    const d2seg = (p: [number, number], a: [number, number], b: [number, number]): number => {
      const [px, py] = p;
      const [ax, ay] = a;
      const [bx, by] = b;
      const dx = bx - ax;
      const dy = by - ay;
      const len2 = dx * dx + dy * dy;
      const t = len2 > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len2)) : 0;
      return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
    };
    const pos = (id: number): [number, number] | null => {
      const v = sv[id];
      return v && !v.removed ? [v.xMm, v.yMm] : null;
    };
    const p: [number, number] = [st.xMm, st.yMm];
    let bestSpan = -1;
    let bestD = Infinity;
    const spanCount = lv.loopLine ? lv.stops.length : lv.stops.length - 1;
    for (let i = 0; i < spanCount; i++) {
      const a = pos(lv.stops[i]);
      const b = pos(lv.stops[(i + 1) % lv.stops.length]);
      if (!a || !b) continue;
      const d = d2seg(p, a, b);
      if (d < bestD) {
        bestD = d;
        bestSpan = i;
      }
    }
    if (bestSpan < 0) return false;
    const evs = this.noteRejections(this.bridge.apply(cmd.addStop(lineId, stationId, bestSpan + 1)));
    const ok = !evs.some((e) => "Rejected" in e);
    if (ok) {
      const at = mmToLngLat(p);
      this.effects.ripple(at[0], at[1], colorToRgb(lv.color).join(","));
      audio.connect();
      this.selectedLine = lineId;
    }
    this.refresh();
    return ok;
  }

  // --- control points (freeform waypoints that bend the draft's track) ---

  /** Screen-pixel hit radius for grabbing a control-point handle. */
  private static readonly HANDLE_PX = 11;

  /** Ordered mm points of the in-progress route threaded through its waypoints: stop0, wp…, stop1,
   *  wp…, plus the live cursor leg. The ghost, water-check and length all read this. */
  draftPointsMm(includeCursor = true): [number, number][] {
    const sv = this.bridge.stationsView();
    const pts: [number, number][] = [];
    for (let i = 0; i < this.draft.length; i++) {
      const s = sv[this.draft[i]];
      if (s) pts.push([s.xMm, s.yMm]);
      if (i + 1 < this.draft.length) for (const w of this.draftWaypoints[i] ?? []) pts.push(w);
    }
    if (includeCursor && this.cursor && pts.length >= 1) pts.push(lngLatToMm(this.cursor));
    return pts;
  }

  /** The draggable control-point handles for the current draft: a solid dot per existing waypoint,
   *  and a faint "+" at every sub-segment midpoint (drag it to bend the track there). lng/lat for
   *  the deck layer; `span`/`index` address `draftWaypoints` (for 'add', the splice index). */
  controlHandles(): { lng: number; lat: number; kind: "waypoint" | "add"; span: number; index: number }[] {
    // No bend handles while EXTENDING: the extension commits straight AddStops (no waypoint
    // vocabulary for "append these bends"), so offering handles would silently drop the bends
    // on commit — the blueprint must never differ from what commits.
    if (this.tool !== "line" || this.draft.length < 2 || this.extendTarget !== null) return [];
    const sv = this.bridge.stationsView();
    const out: { lng: number; lat: number; kind: "waypoint" | "add"; span: number; index: number }[] = [];
    for (let span = 0; span < this.draft.length - 1; span++) {
      const a = sv[this.draft[span]];
      const b = sv[this.draft[span + 1]];
      if (!a || !b) continue;
      const wps = this.draftWaypoints[span] ?? [];
      // The span's full point sequence: stopA, waypoints…, stopB.
      const seq: [number, number][] = [[a.xMm, a.yMm], ...wps, [b.xMm, b.yMm]];
      for (let i = 0; i < wps.length; i++) {
        const [lng, lat] = mmToLngLat(wps[i]);
        out.push({ lng, lat, kind: "waypoint", span, index: i });
      }
      // "+" handle at each sub-segment midpoint; dragging splices a new waypoint at index j.
      for (let j = 0; j < seq.length - 1; j++) {
        const mx = (seq[j][0] + seq[j + 1][0]) / 2;
        const my = (seq[j][1] + seq[j + 1][1]) / 2;
        const [lng, lat] = mmToLngLat([mx, my]);
        out.push({ lng, lat, kind: "add", span, index: j });
      }
    }
    return out;
  }

  /** Begin dragging a control point at a screen pixel. Grabs an existing waypoint, or (on a "+"
   *  midpoint) splices a new one there. Returns true if a handle was grabbed (drawing is paused). */
  startHandleDrag(px: number, py: number, lng: number, lat: number): boolean {
    let best: { kind: "waypoint" | "add"; span: number; index: number } | null = null;
    let bestD = Game.HANDLE_PX;
    for (const h of this.controlHandles()) {
      const p = this.map.project([h.lng, h.lat]);
      const d = Math.hypot(p.x - px, p.y - py);
      // Prefer an existing waypoint over a coincident "+" so you grab, not duplicate.
      if (d <= bestD || (d <= Game.HANDLE_PX && h.kind === "waypoint" && best?.kind === "add")) {
        bestD = Math.min(bestD, d);
        best = { kind: h.kind, span: h.span, index: h.index };
      }
    }
    if (!best) return false;
    if (best.kind === "add") {
      (this.draftWaypoints[best.span] ??= []).splice(best.index, 0, lngLatToMm([lng, lat]));
      this.draggingHandle = { span: best.span, index: best.index };
    } else {
      this.draggingHandle = { span: best.span, index: best.index };
    }
    this.map.dragPan.disable();
    this.refresh();
    return true;
  }

  /** Move the control point under drag to a new lng/lat (bends the ghost live, sub-100 ms). */
  dragHandle(lng: number, lat: number): void {
    const h = this.draggingHandle;
    if (!h) return;
    const span = this.draftWaypoints[h.span];
    if (span && span[h.index]) {
      span[h.index] = lngLatToMm([lng, lat]);
      this.refresh();
    }
  }

  /** Release the dragged control point (drawing resumes; dragPan stays off while drafting). */
  endHandleDrag(): void {
    this.draggingHandle = null;
  }

  /** Append a control point to a draft span (the camera-independent equivalent of a "+"-handle
   *  drag — used by the test hook). `span` indexes the gap between draft stop `span` and `span+1`. */
  addDraftWaypoint(span: number, lng: number, lat: number): void {
    if (span < 0 || span >= this.draft.length - 1) return;
    (this.draftWaypoints[span] ??= []).push(lngLatToMm([lng, lat]));
    this.refresh();
  }

  /** Remove the control point at a screen pixel (double-click) → straightens that bend. */
  removeHandleAt(px: number, py: number): boolean {
    for (const h of this.controlHandles()) {
      if (h.kind !== "waypoint") continue;
      const p = this.map.project([h.lng, h.lat]);
      if (Math.hypot(p.x - px, p.y - py) <= Game.HANDLE_PX) {
        this.draftWaypoints[h.span]?.splice(h.index, 1);
        this.refresh();
        return true;
      }
    }
    return false;
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
    const pts = this.draftPointsMm(); // threaded through waypoints — a bend can route over water
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
  draftPreview(): { stops: number; lengthKm: number; costM: number; invalid: boolean; shortM: number } {
    const len = (pts: [number, number][]) => {
      let mm = 0;
      for (let i = 1; i < pts.length; i++) mm += Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
      return mm;
    };
    const sv = this.bridge.stationsView();
    const stopPts: [number, number][] = this.draft.map((id) => [sv[id].xMm, sv[id].yMm]);
    if (this.cursor) stopPts.push(lngLatToMm(this.cursor));
    const straight = len(stopPts); // stops (+cursor), no waypoints
    const bent = len(this.draftPointsMm()); // threaded through the waypoints + cursor
    // Authoritative straight-stop cost from the core, scaled by the bend ratio so the live readout
    // tracks a detour (the exact bent cost is recomputed by the core on commit). 0 until 2 stops.
    let cost = this.draft.length >= 2 ? this.bridge.previewLineCost(this.draft, this.transport, false) : 0;
    if (cost > 0 && straight > 0) cost = (cost * bent) / straight;
    // Affordability pre-flight (economy on only): how far the draft overshoots the balance, in $M.
    // The core's afford-gate is still the authority at commit — this is the early warning.
    const s = this.lastStats;
    const shortM = s?.economyEnabled ? Math.max(0, (cost - s.balance) / 1e6) : 0;
    return { stops: this.draft.length, lengthKm: bent / 1_000_000, costM: cost / 1e6, invalid: this.draftInvalid(), shortM };
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

  /** Demolition echo — bulldoze was the one Command with NO on-map acknowledgement (AGENTS: every
   *  Command gets an immediate visual echo). A red ripple where the thing stood + a toast naming
   *  what was lost; with the economy on, the toast carries the written-off build cost (capital is
   *  sunk — undo restores it, demolish doesn't refund it). */
  private demolishEcho(name: string, at: [number, number] | null, capitalCost = 0): void {
    if (at) this.effects.ripple(at[0], at[1], "214,40,40");
    const sunk = this.lastStats.economyEnabled && capitalCost > 0 ? ` — $${Math.round(capitalCost / 1e6)}M build cost written off` : "";
    this.notice = `Demolished ${name}${sunk}`;
  }

  /** Bulldoze under a screen pixel: remove the nearest station within snap radius, else the
   *  nearest line. One undoable Command each (undo = rebuild from seed + log[..-1]). */
  bulldozeAt(px: number, py: number): void {
    const st = this.nearestStation(px, py);
    if (st !== null) {
      this.removeStationById(st);
      return;
    }
    const ln = this.nearestLine(px, py);
    if (ln !== null) this.removeLineById(ln);
  }

  // --- right-click context menu (run/select only — build keeps its two-stage stop) ---

  /** Open the context menu at (px,py), resolving the target precedence station → line → empty. */
  openContextMenu(px: number, py: number, lngLat: { lng: number; lat: number }): void {
    const st = this.nearestStation(px, py);
    if (st !== null) this.contextMenu = { x: px, y: py, lngLat, kind: "station", id: st };
    else {
      const ln = this.nearestLine(px, py);
      this.contextMenu = ln !== null ? { x: px, y: py, lngLat, kind: "line", id: ln } : { x: px, y: py, lngLat, kind: "empty", id: -1 };
    }
    for (const cb of this.onChange) cb();
  }

  closeContextMenu(): void {
    if (this.contextMenu === null) return;
    this.contextMenu = null;
    for (const cb of this.onChange) cb();
  }

  /** Bulldoze a station by id (bulldozer tool + right-click → Bulldoze): one undoable
   *  RemoveStation Command, with the demolition echo. Undo = rebuild from seed + log. */
  removeStationById(id: number): void {
    const sv = this.bridge.stationsView()[id];
    const at = sv && !sv.removed ? mmToLngLat([sv.xMm, sv.yMm]) : null;
    const name = sv?.name || `Station ${id + 1}`;
    const evs = this.noteRejections(this.bridge.apply(cmd.removeStation(id)));
    if (!evs.some((e) => "Rejected" in e)) this.demolishEcho(name, at);
    if (this.selectedStation === id) this.selectedStation = null;
    this.refresh();
  }

  /** Bulldoze a line by id (its vehicles despawn) — one undoable RemoveLine Command, with the
   *  demolition echo (incl. the written-off capital when the economy is on). */
  removeLineById(id: number): void {
    const lv = this.bridge.linesView()[id];
    const name = lv?.name || `Line ${id + 1}`;
    const mid = lv && !lv.removed && lv.polylineMm.length > 0 ? mmToLngLat(lv.polylineMm[Math.floor(lv.polylineMm.length / 2)]) : null;
    const capital = this.perLineById.get(id)?.capitalCost ?? 0;
    const evs = this.noteRejections(this.bridge.apply(cmd.removeLine(id)));
    if (!evs.some((e) => "Rejected" in e)) this.demolishEcho(name, mid, capital);
    if (this.selectedLine === id) this.selectedLine = null;
    this.refresh();
  }

  /** Follow the nearest pickable rider to screen-centre (the empty-menu "watch a random rider").
   *  Returns false if none is eligible (e.g. gravity demand has only anonymous trips). */
  followRandomPeep(): boolean {
    const c = this.map.getContainer();
    const cid = this.nearestPeep(c.clientWidth / 2, c.clientHeight / 2, 1e9);
    if (cid === null) return false;
    this.setFollowed(cid);
    return true;
  }

  /** Try to inspect (follow) the rider under (px,py). Returns true if a real citizen was picked;
   *  on an anonymous-only hit, raises a one-shot nudge toward agent demand and returns false. */
  inspectPeepAt(px: number, py: number): boolean {
    const cid = this.nearestPeep(px, py);
    if (cid !== null) {
      this.setFollowed(cid);
      return true;
    }
    // Nothing followable here. If peeps ARE present (anonymous gravity trips), nudge toward agents.
    if (this.peepCit.length > 0 && !this.agentDemand) {
      this.notice = "Anonymous trip — switch to Citizen (agent) demand in Settings to follow riders";
      for (const cb of this.onChange) cb();
    }
    return false;
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
        const ps = this.perStationById.get(s.id);
        return {
          id: s.id,
          lng,
          lat,
          name: s.name,
          selected: s.id === this.selectedStation,
          boardings: ps?.boardings ?? 0, // throughput → dot radius
          serving: ps?.serving ?? 0, // 0 = orphaned → muted fill
          bounty: s.bounty, // fantasy: >0 → a ⚑ marker (the steering lever's visual feedback)
        };
      });

    // Captured demand, self-calibrated against the busiest station so the catchment-fill density
    // reads comparatively regardless of a city's absolute demand weights.
    const stationDemand = (id: number): number => {
      const ps = this.perStationById.get(id);
      return ps ? ps.demandOrigin + ps.demandDest : 0;
    };
    let maxDemand = 0;
    for (const ps of this.lastStats.perStation) maxDemand = Math.max(maxDemand, ps.demandOrigin + ps.demandDest);

    // A pinned (selected) station gets the filled catchment (alpha ∝ its captured demand); a mere
    // hover peek gets the stroke-only fainter one (render.ts splits on `peek`).
    const peeking = this.selectedStation === null;
    const catchments =
      highlight === null
        ? []
        : stationsV
            .filter((s) => s.id === highlight)
            .map((s) => {
              const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
              return { lng, lat, radiusM: CATCHMENT_M, peek: peeking, demand: maxDemand > 0 ? stationDemand(s.id) / maxDemand : 0 };
            });

    // The lopsided walk shed of the highlighted station (hexagons over its reachable cells). The
    // catchment ring above drops to a nominal-reach outline whenever this is non-empty.
    const shed = highlight === null ? [] : this.shedFor(highlight, stationsV[highlight]);

    const lines = linesV
      .filter((l) => !l.removed)
      .flatMap((l) => {
        // A line with surface track over water renders red until elevated/tunnelled.
        const color = l.crossesWaterSurface ? ([214, 40, 40] as [number, number, number]) : colorToRgb(l.color);
        const mode = l.mode; // heavy/high-speed rail (4) gets distinct mainline styling
        // The trunk, plus one path per branch (P3) — all the same id/colour so a Y-shaped line
        // (e.g. the Circle Line's Marina Bay spur) draws as one coloured service.
        const paths = [l.polylineMm, ...(l.branchPolylinesMm ?? [])];
        return paths
          .filter((p) => p.length >= 2)
          .map((p) => ({ id: l.id, color, path: p.map(([x, y]) => mmToLngLat([x, y])), mode }));
      });

    // Blueprint: the draft threaded through its control points (so bends render live) + cursor leg.
    const blueprint: [number, number][] = this.draftPointsMm().map((p) => mmToLngLat(p));

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

    // OD desire lines from the selected station (on-demand → no mud): where its riders are drawn.
    // Empty unless a served station is pinned (the core query returns [] for orphaned/unserved).
    // Suppressed while the Reach overlay is on — desire vs reach are alternative lenses on the
    // pinned station (where demand pulls / how fast you get there), shown one at a time, not stacked.
    let desire: DesireArc[] = [];
    if (this.selectedStation !== null && !this.showReach) {
      const sv0 = stationsV[this.selectedStation];
      if (sv0 && !sv0.removed) {
        const from = mmToLngLat([sv0.xMm, sv0.yMm]);
        desire = this.bridge.stationOd(this.selectedStation).map((o) => ({
          from,
          to: mmToLngLat([o.xMm, o.yMm]),
          weight: o.weight,
        }));
      }
    }

    // Accessibility isochrone (opt-in "Reach" toggle): shade stations reachable from the selected
    // one by transit travel time. Off by default so it never piles onto the selection overlay.
    let reach: ReachDot[] = [];
    if (this.showReach && this.selectedStation !== null) {
      reach = this.bridge.stationAccess(this.selectedStation).map((a) => {
        const [lng, lat] = mmToLngLat([a.xMm, a.yMm]);
        return { lng, lat, ms: a.ms };
      });
    }

    // ROAD corridors: shown when the toggle is on, OR auto-revealed while drawing a Bus line
    // (transport 1) so you can see where to route it cheap + fast. Empty otherwise (no overlay).
    const showRoads = this.showRoads || (this.mode === "build" && this.tool === "line" && this.transport === 1);
    const roads = showRoads ? this.roadPoints() : [];
    const roadHour = Math.floor(this.lastStats.simHour); // drives the live congestion recolour

    return {
      stations,
      lines,
      catchments,
      shed,
      blueprint,
      roads,
      roadHour,
      demandCellM: this.demandCellM,
      roadCellM: this.build.cellMm / 1000, // mm → m (the buildability grid pitch)
      terrain: this.terrain, // baked fantasy terrain hexes (the map itself); empty for transit cities
      terrainCellM: this.terrainCellM, // fantasy hex size (m) → the hexagon circumradius
      tideCells: this.decadenceTideAt(), // fantasy S10c: the cold decadence creep (read on each refresh)
      resources: this.resources, // baked fantasy supply-chain source nodes; empty for transit cities
      towns: this.towns, // baked fantasy towns (sinks + conquest targets); empty for transit cities
      decadenceAnchors: this.decadenceAnchors, // baked far-edge reservoir anchors; empty for transit cities
      vehicles: [],
      waiting,
      hazards,
      demand,
      desire,
      reach,
      blueprintInvalid: this.draftInvalid(),
      blueprintColor: this.extendTarget
        ? colorToRgb(this.bridge.linesView()[this.extendTarget.line]?.color ?? 0x787e86)
        : null,
      controlHandles: this.controlHandles(),
      pinnedLabel,
      selectedLine: this.selectedLine,
      snapRing: this.snapRingView(),
    };
  }

  /** The pre-commit snap highlight datum (or null): the station the next click would chain
   *  (line tool) or demolish (bulldozer), set by the pointer per mousemove. */
  private snapRingView(): { lng: number; lat: number; demolish: boolean } | null {
    if (this.snapStation === null || this.mode !== "build") return null;
    if (this.tool !== "line" && this.tool !== "bulldozer") return null;
    const s = this.bridge.stationsView()[this.snapStation];
    if (!s || s.removed) return null;
    const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
    return { lng, lat, demolish: this.tool === "bulldozer" };
  }

  private shedKey: string | null = null;
  private shedView: ShedHex[] = [];
  /** Walk-shed hexagons for the highlighted station — the lopsided set of reachable cells from the
   *  core (water severs, motorways pinch). Pure geography (a static raster), so it's memoized and
   *  recomputed only when the highlight changes — never per frame. Keyed on id + POSITION (not id
   *  alone): an undo/load rebuilds the World and can rebind an id to a new station, so the position
   *  in the key forces a recompute then. Empty for cities with no buildability raster (the catchment
   *  ring then renders its classic filled disc). */
  private shedFor(id: number, sv: { xMm: number; yMm: number; removed?: boolean } | undefined): ShedHex[] {
    if (!sv || sv.removed) return [];
    const key = `${id}:${sv.xMm},${sv.yMm}`;
    if (key === this.shedKey) return this.shedView;
    this.shedKey = key;
    this.shedView = this.bridge.stationWalkshed(id).map((c) => {
      const [lng, lat] = mmToLngLat([c.xMm, c.yMm]);
      return { lng, lat, intensity: c.intensity };
    });
    return this.shedView;
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
    if (s.running) this.emitStatsJuice(s);
    this.renderTip(); // an open inspector tooltip re-reads the fresh snapshot (no frozen numbers)
    this.refresh();
  }

  /** Spatial juice driven off the ~3 Hz stats snapshot (NOT per sim tick): throb the worst-starved
   *  platforms, and spark a soft burst where boardings jumped since the last snapshot. */
  private emitStatsJuice(s: Stats): void {
    // Cheap scans first — no projection / no stationsView() until we know there's work to draw.
    // Worst-starved platforms (capped so it reads as "fix these few", not a sea of rings).
    const starvedPs = s.perStation
      .filter((ps) => ps.waiting >= STARVED_WAITING)
      .sort((a, b) => b.waiting - a.waiting)
      .slice(0, 24);
    // Board-bursts: stations with the biggest boarding gain since the last snapshot.
    const deltas: { id: number; d: number }[] = [];
    for (const ps of s.perStation) {
      const prev = this.prevBoardings.get(ps.stationId) ?? ps.boardings; // first snapshot → no burst
      const d = ps.boardings - prev;
      if (d >= 3) deltas.push({ id: ps.stationId, d });
      this.prevBoardings.set(ps.stationId, ps.boardings);
    }
    if (starvedPs.length === 0 && deltas.length === 0) {
      this.effects.setThrobs([]); // healthy network — clear stale throbs, skip all projection work
      return;
    }
    const sv = this.bridge.stationsView();
    const at = (id: number): { lng: number; lat: number } | null => {
      const v = sv[id];
      if (!v || v.removed) return null;
      const [lng, lat] = mmToLngLat([v.xMm, v.yMm]);
      return { lng, lat };
    };
    this.effects.setThrobs(
      starvedPs.map((ps) => at(ps.stationId)).filter((p): p is { lng: number; lat: number } => p !== null),
    );
    deltas.sort((a, b) => b.d - a.d);
    for (const { id } of deltas.slice(0, 6)) {
      const p = at(id);
      if (p) this.effects.burst(p.lng, p.lat);
    }
  }

  /** Rebuild cached topology layers from authoritative sim views; recompose with current
   *  (non-interpolated) vehicle positions. The GameLoop recomposes per frame with alpha. */
  refresh(): void {
    const { below, above } = topoLayers(this.buildView());
    this.below = below;
    this.above = above;
    this.composeAndSet(this.currentVehicleDots(), this.peepLayerAt(1));
    for (const cb of this.onChange) cb();
  }

  /** Set the overlay layers: stable cached topo with the vehicle layer + peep layer spliced into
   *  z-order (catchment/lines/blueprint < vehicles < peeps < stations < waiting). Reused topo
   *  instances mean deck only re-uploads the small per-frame vehicle + peep layers. */
  /** Marching-legion dots (fantasy). Read each compose like the vehicle layer; metres→lng/lat in place.
   *  Null when there are no legions (transit always; arcadia until the first launch). */
  armyLayerAt(): Layer | null {
    const xy = this.bridge.armyPositions();
    const count = xy.length >> 1;
    if (count === 0) return null;
    for (let i = 0; i < xy.length; i += 2) metersToLngLatInto(xy[i], xy[i + 1], xy, i);
    return armyLayer(xy, count);
  }

  /** The decadence tide's corrupted cells (fantasy S10c), read on each ~3 Hz refresh (the tide creeps
   *  slowly, never per frame). `[x_m,y_m,v,...]` metres → lng/lat. Empty for transit / before it starts. */
  private decadenceTideAt(): TideCell[] {
    const t = this.bridge.decadenceTide();
    const out: TideCell[] = [];
    for (let i = 0; i + 2 < t.length; i += 3) {
      const [lng, lat] = metersToLngLat([t[i], t[i + 1]]);
      out.push({ lng, lat, v: t[i + 2] });
    }
    return out;
  }

  composeAndSet(vehicles: VehicleDot[], peeps: Layer | null): void {
    const peep = peeps ? [peeps] : [];
    const armies = this.armyLayerAt();
    const army = armies ? [armies] : []; // legions above carts, below peeps/labels (z-order)
    // Level-of-detail (runs per frame on the live zoom): below DETAIL_ZOOM the city-overview shows
    // only the network — drop the per-station waiting halos, the pinned label, and the vehicle
    // direction arrows (micro-detail that turns to a flashing swarm at overview). Peeps are gated
    // separately in peepLayerAt. Cheap: a filter over ~17 already-built layers, no rebuild.
    const detail = this.map.getZoom() >= DETAIL_ZOOM;
    const vlayers = detail ? vehicleLayers(vehicles) : vehicleLayers(vehicles).filter((l) => l.id !== "vehicle-dir");
    // Exactly one waiting layer shows per frame: the full per-station halos when zoomed in, the
    // starved-only subset at overview (a starved platform must be findable at ANY zoom).
    const above = detail
      ? this.above.filter((l) => l.id !== "waiting-overview")
      : this.above.filter((l) => l.id !== "waiting" && l.id !== "station-label");
    this.overlay.setProps({ layers: [...this.below, ...vlayers, ...army, ...peep, ...above] });
  }

  /** Per-line colour table indexed by line id (for vehicle tint). */
  lineColors(): number[] {
    return this.bridge.linesView().map((l) => l.color);
  }

  /** Build vehicle dots interpolated at `alpha` (0 = previous tick, →1 = current), each carrying
   *  line tint + heading (directional triangle) + load factor (crowding ring). The single source
   *  for both the per-frame GameLoop render and the on-refresh recompose, so they never drift. */
  vehicleDotsAt(alpha: number): VehicleDot[] {
    const cur = this.bridge.vehiclePositions();
    if (cur.length === 0) return [];
    const prev = this.bridge.vehiclePrevPositions();
    const lineIds = this.bridge.vehicleLineIds();
    const angles = this.bridge.vehicleAngles();
    const loads = this.bridge.vehicleLoads(); // interleaved [onboard, capacity] per vehicle
    const colors = this.lineColors();
    const dots: VehicleDot[] = [];
    for (let i = 0; i < cur.length; i += 2) {
      const vi = i / 2;
      const x = prev[i] + (cur[i] - prev[i]) * alpha;
      const y = prev[i + 1] + (cur[i + 1] - prev[i + 1]) * alpha;
      const [lng, lat] = metersToLngLat([x, y]);
      const cap = loads[vi * 2 + 1] ?? 0;
      const load = cap > 0 ? (loads[vi * 2] ?? 0) / cap : 0;
      dots.push({ lng, lat, color: colorToRgb(colors[lineIds[vi]] ?? 0x444444), angle: angles[vi] ?? 0, load });
    }
    return dots;
  }

  /** Current (non-interpolated) vehicle dots — the on-refresh recompose before the loop runs. */
  currentVehicleDots(): VehicleDot[] {
    return this.vehicleDotsAt(1);
  }

  /** Pre-seed a real-world network (stations + lines) via the Command path. Stations are
   *  placed in array order (so line indices match), interchanges are shared station indices. */
  applyNetwork(net: import("./sim/network").Network): void {
    for (const s of net.stations) {
      const [x, y] = lngLatToMm([s.lng, s.lat]);
      // Fantasy: a flagged node is a BARRACKS (fields legions); otherwise a plain station. Both
      // create a node at this index, so line references stay aligned either way.
      this.bridge.apply(s.barracks ? cmd.placeBarracks(x, y, s.name) : cmd.placeStation(x, y, s.name));
    }
    net.lines.forEach((line, li) => {
      // Imported lines with real OSM geometry are LITERAL — they follow the supplied track
      // alignment directly (no synthesised Catmull-Rom curve).
      const literal = !!(line.geometry && line.geometry.length);
      this.bridge.apply(cmd.createLine(parseInt(line.colorHex, 16) >>> 0, line.name, line.loop ?? false, line.mode ?? 0, literal));
      for (const idx of line.stations) this.bridge.apply(cmd.addStop(li, idx));
      if (literal) {
        // Per-span [lng,lat] vertices → mm waypoints (the one coordinate crossing, coords/geo.ts).
        const wps = line.geometry!.map((span) => span.map(([lng, lat]) => lngLatToMm([lng, lat])));
        this.bridge.apply(cmd.setLineWaypoints(li, wps));
      }
      // Branches (P3): build each via AddBranchStop — branch index bi creates on its first stop
      // (bi == current branch count) then extends. Recovers e.g. the Circle Line's Marina Bay spur.
      (line.branches ?? []).forEach((br, bi) => {
        for (const st of br.stations) this.bridge.apply(cmd.addBranchStop(li, bi, br.divergeAt, st));
        if (literal && br.geometry && br.geometry.length) {
          const wps = br.geometry.map((span) => span.map(([lng, lat]) => lngLatToMm([lng, lat])));
          this.bridge.apply(cmd.setBranchWaypoints(li, bi, wps));
        }
      });
      this.bridge.apply(cmd.assignTrainset(li, 0, Math.max(1, Math.min(8, line.trains))));
      // headwayMin in the network JSON means real minutes of SERVICE — clock-frame sim-ms now.
      this.bridge.apply(cmd.setHeadway(li, Math.round(line.headwayMin * SIM_MS_PER_CLOCK_MIN)));
    });
    this.legalizeWaterCrossings(net);
    this.selectedStation = null;
    this.selectedLine = null;
    this.refresh();
  }

  /** A loaded real metro is EXISTING grade-separated infrastructure — where a land line crosses
   *  water it does so on a tunnel/viaduct, not illegal surface track. Without this, those spans load
   *  as surface-over-water and render as the red "fix me" warning instead of the line's true colour
   *  (and the editor scolds pre-built reality). So tunnel each span that crosses water; ferry/air
   *  (modes 2/3) cross water freely and are skipped. This also matters mechanically now: the core
   *  PARKS a surface-over-water line (no dispatch) until it's grade-separated, so legalizing here
   *  is what lets a loaded network run at all. Deterministic (commands on the same log). At boot
   *  the economy is off, so the afford-gate never rejects it. A whole-line fallback covers any
   *  curve that dips into water where the straight span didn't, so a loaded land line is never
   *  left painted red (or parked). */
  private legalizeWaterCrossings(net: import("./sim/network").Network): void {
    if (!this.build.loaded) return;
    const TUNNEL = 2;
    const crossesWater = (a: number, b: number): boolean => {
      const sa = net.stations[a];
      const sb = net.stations[b];
      if (!sa || !sb) return false;
      const [ax, ay] = lngLatToMm([sa.lng, sa.lat]);
      const [bx, by] = lngLatToMm([sb.lng, sb.lat]);
      const len = Math.hypot(bx - ax, by - ay);
      const steps = Math.min(80, Math.max(1, Math.round(len / this.build.cellMm)));
      for (let k = 0; k <= steps; k++) {
        const x = ax + ((bx - ax) * k) / steps;
        const y = ay + ((by - ay) * k) / steps;
        if (this.build.classifyMm(x, y) === BUILD.WATER) return true;
      }
      return false;
    };
    net.lines.forEach((line, li) => {
      const mode = line.mode ?? 0;
      if (mode === 2 || mode === 3) return; // ferry/air cross water freely — leave on surface
      const ids = line.stations;
      for (let j = 0; j + 1 < ids.length; j++) {
        if (crossesWater(ids[j], ids[j + 1])) this.bridge.apply(cmd.setSegmentMode(li, j, TUNNEL));
      }
      if (line.loop && ids.length > 2 && crossesWater(ids[ids.length - 1], ids[0])) {
        this.bridge.apply(cmd.setSegmentMode(li, ids.length - 1, TUNNEL));
      }
    });
    // Safety net: any line the core STILL flags (a curve crossing water the straight span missed)
    // gets a whole-line tunnel, so a loaded land line never renders as the surface-over-water red.
    for (const lv of this.bridge.linesView()) {
      if (lv.crossesWaterSurface) this.bridge.apply(cmd.setSegmentMode(lv.id, WHOLE_LINE, TUNNEL));
    }
  }

  /** The next palette colour for a new line (deterministic by line index). */
  nextLineColor(): number {
    const n = this.bridge.linesView().length;
    return LINE_PALETTE[n % LINE_PALETTE.length];
  }
}
