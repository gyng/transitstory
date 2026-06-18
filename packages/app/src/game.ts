// The hub controller: owns the SimBridge, map, and overlay; holds UI state (mode, tool,
// selection, line draft); rebuilds the overlay from authoritative sim views. UI/tools only
// call Game methods which emit Commands and refresh — they never mutate sim state directly.
import type { Map as MlMap } from "maplibre-gl";
import type { MapboxOverlay } from "@deck.gl/mapbox";
import type { Layer, PickingInfo } from "@deck.gl/core";
import { ARCADIA_LINE_PALETTE, BUSY_WAITING, CATCHMENT_M, DETAIL_ZOOM, LINE_PALETTE, SNAP_PX, STARVED_WAITING, TICK_MS } from "./config";
import { lngLatToMm, metersToLngLat, metersToLngLatInto, mmToLngLat } from "./coords/geo";
import { cmd } from "./commands/codec";
import { signalLayer, placedSignalLayers, ambientCargoLayer, ambientTraderLayer, armyIntentLayer, legionLayer, legionCampfireLayer, legionNameLayer, entityBadgeLayer, raiderIntentLayer, raiderLayer, spellFlashLayer, colorToRgb, nightGlowLayers, peepLayer, topoLayers, vehicleLayers, vehicleNightGlow, type AmbientTrader, type BufferPip, type CargoCar, type DecadenceAnchor, type DemandPoint, type DesireArc, type BarracksBadge, type FrontierNode, type HazardDot, type IntentArc, type LegionDot, type PlacedSignalMarker, type RaidLabel, type SignalGhost, type SiegeRing, type ReachDot, type RenderView, type ResourceMarker, type RiverSeg, type ShedHex, type TerrainCell, type TideCell, type TownMarker, type TreeInstance, type VehicleDot, type WaitingDot } from "./render";
import { audio } from "./fx/audio";
import { Effects, type Flow, type NightLight } from "./fx/effects";
import { createSky, type Sky } from "./map/sky";
import { WHOLE_LINE } from "./commands/codec";
import { BUILD, Buildability } from "./sim/buildability";
import { axialOf, centerOf, lineCosted, type Axial } from "./sim/hexgrid";
import type { SimBridge } from "./sim/SimBridge";
import type { Event, PerLine, PerStation, Stats } from "./types";
import { fmtMoney, lineTipHtml, MODE_SPECS, MODES, modeIcon, SIM_MS_PER_CLOCK_MIN, stationTipHtml, vehicleTipHtml, type LineTip, type StationTip, type VehicleTip } from "./ui/react/shared";
import { meanStopQueue } from "./ui/react/lineEconomics";

/** Compact number for floating juice text — "1.2M" / "45k" / "678" (the caller adds any $/⬢/sign). */
function fmtShort(n: number): string {
  const a = Math.abs(n);
  return a >= 1e6 ? `${Math.round(n / 1e5) / 10}M` : a >= 1e4 ? `${Math.round(n / 1e3)}k` : `${Math.round(n)}`;
}

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
  mana: 0,
  manpower: 0,
  decadence: 0,
  decadencePct: 0,
  townsCaptured: 0,
  armyCount: 0,
  raiderCount: 0,
  realmLost: false,
  techUnlocked: 0,
  spellsCast: 0,
  autocast: false,
  buildGoldDivisor: 0,
  goldUpkeepDaily: 0,
};

export type Mode = "build" | "run";
// TTD L6 (track + services), the OWNER-CHOSEN OpenTTD model. Two draw gestures share the SAME pipeline
// (extendDraft → commitDraft → drawLineByIds), branching only at commit:
//  • `line` is "Track" — lays a stockless corridor (bare grey rail) you route services over.
//  • `service` is "Service" — the same draw, but commit AUTO-ASSIGNS stock so it lands as a live coloured
//    line. Draw several services over the same stations to share one corridor (emergent via co-located
//    cells; the L1 cross-line mutex + TrackGraph already fuse them).
// Snap-restricting Service to on-track stations + the L3 `BindLineToTrack{line,segments}` Command (a
// first-class TrackSegment reference instead of binding-by-co-located-cells) remain the clean next seams.
export type Tool = "select" | "station" | "line" | "service" | "bulldozer" | "barracks" | "bounty";

/** The two TTD L6 draw tools that share the chain-stations gesture (Track lays bare rail, Service routes a
 *  stocked line). Everything in the draw pipeline (pointer dispatch, control handles, snap ring, blueprint
 *  cursor) treats them identically; only `commitDraft` branches to auto-assign stock for a service. */
export function isDrawTool(tool: Tool): boolean {
  return tool === "line" || tool === "service";
}

/** Default fleet a freshly-drawn SERVICE lands with (the Service tool auto-assigns this so it's a live
 *  coloured line at once; the player tunes count/headway/model in the editor). Bare track (the Track tool)
 *  gets nothing — it stays grey until stocked. */
const DEFAULT_SERVICE_TRAINS = 2;

/** Map-lens (#5) → the deck layer ids HIDDEN in that view mode (terrain + the player's network/vehicles are
 *  never hidden). "supply" dims the war + the rot; "military" dims the supply detail; "decadence" dims both
 *  supply detail + the legions, leaving the tide + raiders. */
const LENS_HIDE: Record<"supply" | "military" | "decadence", Set<string>> = {
  supply: new Set(["decadence-tide", "tide-front", "decadence-anchors", "army-intent", "raider-intent", "armies", "legion-names", "raiders", "raider-badges-0", "raider-badges-1", "raider-badges-2", "spells"]),
  military: new Set(["rivers", "resources", "resource-icons", "demand", "ambient-traders"]),
  decadence: new Set(["rivers", "resources", "resource-icons", "demand", "army-intent", "armies", "legion-names", "ambient-traders"]),
};

/** Standard bounty posted per click of the bounty tool — baits AI legions toward that town. */
const BOUNTY_AMOUNT = 1000;

// Legion HOST names (#legion-3d nameplates) — a fixed CB-flavour roster cycled by legion slot index
// (deterministic + render-only; a recycled slot reusing a name is fine, it's cosmetic identity).
const LEGION_NAMES = [
  "Iron Host", "Ash Legion", "Dawn Cohort", "Stone Vanguard", "Ember Host", "Grey Lances",
  "Thornguard", "Oathsworn", "Wolf Cohort", "Hearth Legion", "Bronze Host", "Stormcall",
];
/** Manpower a legion costs to field (mirrors the core's `army::LAUNCH_COST`) — for the barracks ready/starved
 *  tint + the "−manpower → ⚔" launch hint. The core is authoritative; this is a display approximation. */
const LAUNCH_COST_MANPOWER = 8;

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
  kind: "station" | "line" | "vehicle" | "peep" | "town" | "resource" | "empty";
  /** Station/line id · vehicle index · peep citizen id · town/resource array index; -1 for empty map. */
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
  /** Map LENS / view mode (fantasy #5): emphasise one reading of the busy arcadia map by dimming the
   *  others. "realm" = everything; "supply" = sources/towns/rivers/icons; "military" = legions/raiders/
   *  intent/towns; "decadence" = the tide/front/reservoir. Read in composeAndSet (render-only). */
  lens: "realm" | "supply" | "military" | "decadence" = "realm";
  private roadCells: import("./render").RoadCell[] | null = null;
  /** Baked fantasy terrain hexes (lng/lat + biome code) — the map itself. Set once at load from the
   *  city's buildability raster (fantasy only; empty for transit cities), so the array identity is
   *  stable across frames (no per-frame rebuild). `terrainCellM` = the hex circumradius in metres. */
  terrain: TerrainCell[] = [];
  terrainCellM = 0;
  /** Fantasy 3D diorama (#3d-trees): lowpoly pines instanced on the forest hexes. Built once at load from
   *  the terrain (arcadia only; empty for transit). Stable identity (no per-frame rebuild). */
  trees: TreeInstance[] = [];
  /** Baked fantasy resource nodes (lng/lat + kind + yield) — the supply-chain sources. Set once at load
   *  from the manifest's supplyGraph (fantasy only; empty for transit). Stable identity across frames. */
  resources: ResourceMarker[] = [];
  /** Baked fantasy towns (sinks + conquest targets) + the far-edge decadence reservoir anchors. Set once
   *  at load from the manifest's supplyGraph (fantasy only; empty for transit). Stable identity. */
  towns: TownMarker[] = [];
  decadenceAnchors: DecadenceAnchor[] = [];
  /** Baked flow-accumulation rivers (lng/lat segments) — render-only cold water. Set once at load from the
   *  manifest's additive `rivers` field (fantasy only; empty for transit). Stable identity across frames. */
  rivers: RiverSeg[] = [];
  /** Fantasy (arcadia) #infrastructure: >0 ARMS the connected-rail gate — rail extends only from a
   *  station already wired to the capital network (or a captured town). Baked into the manifest; mirrors
   *  `CityData.influence_hops` (the value no longer sets a radius, just on/off). 0 ⇒ no gate (transit +
   *  un-gated cities). Drives the rail-frontier halos; the authoritative gate lives in the core
   *  (`World::connected_can_add`), and the per-station `reachable` flag in the snapshot feeds the overlay. */
  influenceHops = 0;
  /** Living-world (#living): ambient ox-cart trade routes between the baked nodes (capital↔towns,
   *  town↔town, resource→town) + the carts trundling them — purely DECORATIVE (wall-clock animated,
   *  never sim state), the texture that makes the continent feel inhabited. Built once at load (arcadia
   *  only). A route your rail now serves dims (the freight "industrialised" onto the railway). */
  /** Each route is a terrain-FOLLOWING polyline (A* around water + inside the map), so carts trundle the
   *  land like real ox-trains instead of cutting straight across sea/off-map. `cum`/`len` are arc-lengths
   *  (lng/lat units) so motion is constant ground-speed regardless of route length; `glyph`/`tint` are the
   *  cargo it hauls (so you can see what's being transported). */
  private ambientRoutes: { pts: [number, number][]; cum: number[]; len: number; served: boolean; glyph: string; tint: [number, number, number] }[] = [];
  private ambientCarts: { route: number; off: number }[] = [];
  /** Cached terrain bounds (mm) so ambient pathfinding rejects off-map cells. Built lazily from `terrain`. */
  private terrainBboxMm: [number, number, number, number] | null = null;
  showAmbient = true;
  /** Toggle the individual-rider "peep" dots (Cities:Skylines-style). On by default; only drawn
   *  while running (peeps are the in-transit passenger set). The dots are a determinism-free
   *  render-only read-out from the core — no sim state, no Command. */
  showPeeps = true;
  /** TTD signals overlay (single-track block state) — OFF by default (opt-in "signal view", so the clean
   *  map isn't cluttered with a dot per block). On ⇒ the render reads `signalMarkers` each frame. */
  showSignals = false;
  selectedStation: number | null = null;
  selectedLine: number | null = null;
  hoveredStation: number | null = null;
  /** Pre-commit snap candidate: the station the next click would chain (line tool) or demolish
   *  (bulldozer). Set by the pointer per mousemove; rendered as a ring BEFORE the click commits. */
  snapStation: number | null = null;
  /** TTD L5c pre-commit signal candidate: what a click would do near the SELECTED line's track in build
   *  mode. `remove` carries an existing placed signal's address (the post the click would delete);
   *  `place` carries the spot + derived `(line, path, span, atMm)` the click would drop a new signal at.
   *  Set by the pointer per mousemove → drawn as a highlight/ghost BEFORE the click commits (AGENTS UX). */
  signalSnap:
    | { kind: "remove"; lng: number; lat: number; line: number; path: number; span: number; atMm: number }
    | { kind: "place"; lng: number; lat: number; line: number; path: number; span: number; atMm: number }
    | null = null;
  /** Last rejection reason (e.g. afford-gate) for a transient toast; cleared on dismiss. */
  notice: string | null = null;

  /** Pending (un-confirmed) station: a ghost at the snapped hex cell the player clicked, awaiting the
   *  confirm bar's ✓/✗ (fantasy "confirm build"). Client-side only — no Command until confirmed. */
  pendingStation: { lng: number; lat: number; xMm: number; yMm: number } | null = null;
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
  /** Cached TTD L5c placed-signal layers (the selected line's posts + the place ghost). Rebuilt on refresh
   *  (place/remove/select/snap change), NOT per rAF — they don't move with the trains (AGENTS render hot path). */
  private placedSignalLayersCache: Layer[] = [];
  /** Spatial juice canvas (ripples / connect-flash / throbs). Client-side acknowledgement only —
   *  driven by the existing GameLoop rAF, never a deck rebuild or a sim tick. */
  effects!: Effects;
  /** Day/night mood wash over the basemap (driven by sim hour off the ~3 Hz stats slice). */
  readonly sky: Sky;
  /** Per-station boardings from the previous stats snapshot — to emit a board-burst on the delta. */
  private prevBoardings: Map<number, number> = new Map();
  /** Last-snapshot per-node buffer fill (0..1) — so a RISE between 3 Hz snapshots reads as "this forge is
   *  working" (a source stockpiling) or "supply just landed" (a sink receiving). See emitWorldJuice. */
  private prevBufferFill: Map<number, number> = new Map();
  /** Per-station ALIGHTINGS from the previous snapshot — fantasy income floats at the delivery (earning)
   *  spots: tribute lands where cargo alights at a sink, so the gold floats where it was actually earned. */
  private prevAlightings: Map<number, number> = new Map();
  /** Cumulative economy/combat readings from the previous stats snapshot — diffed to emit floating
   *  "+gold"/"+$fare"/"−$upkeep"/"⚔ Conquered!" juice. Seeded on the first snapshot (no spurious floats). */
  private prevJuice: { tribute: number; fare: number; opex: number; towns: number; day: number; seeded: boolean } = {
    tribute: 0,
    fare: 0,
    opex: 0,
    towns: 0,
    day: 0,
    seeded: false,
  };
  /** Round-robin cursor so each train trails ONE steam puff every few stats ticks (a trail, not a fog). */
  private puffCursor = 0;
  /** Town ids already celebrated with a conquest boom — so a fallen town fires its "⚔ Conquered!" once. */
  private celebratedTowns = new Set<number>();
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
    } else if (info.layer.id === "lines" || info.layer.id === "track-rails") {
      // TTD L6: bare track (the grey `track-rails` layer) inspects like any line — so the player can hover
      // unserved infrastructure to read it and click to select it for stock assignment.
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

  /** Hex-cell size (mm) for fantasy snapping — 0 for transit (continuous geometry, no snap). */
  private get cellMm(): number {
    return this.terrainCellM > 0 ? this.terrainCellM * 1000 : 0;
  }

  /** Snap a mm position to its hex-cell CENTRE (fantasy) + the cell's axial id; identity for transit. */
  private snapToCell(xMm: number, yMm: number): { xMm: number; yMm: number; cell: Axial | null } {
    const s = this.cellMm;
    if (s <= 0) return { xMm, yMm, cell: null };
    const cell = axialOf(xMm, yMm, s);
    const [sx, sy] = centerOf(cell[0], cell[1], s);
    return { xMm: sx, yMm: sy, cell };
  }

  /** Is a non-removed station already on this hex cell? (the one-station-per-cell rule). */
  private cellOccupied(cell: Axial): boolean {
    const s = this.cellMm;
    if (s <= 0) return false;
    for (const st of this.bridge.stationsView()) {
      if (st.removed) continue;
      const a = axialOf(st.xMm, st.yMm, s);
      if (a[0] === cell[0] && a[1] === cell[1]) return true;
    }
    return false;
  }

  placeStation(lng: number, lat: number): number {
    const [rawX, rawY] = lngLatToMm([lng, lat]);
    // Fantasy: snap to the hex cell + enforce one station per cell (also blocks duplicating a baked
    // town/resource station). Transit: identity, no cell limit. The Command carries the SNAPPED mm, so
    // replay is exact.
    const { xMm, yMm, cell } = this.snapToCell(rawX, rawY);
    if (cell && this.cellOccupied(cell)) {
      this.notice = "One station per cell — this hex already has a station";
      for (const cb of this.onChange) cb();
      return -1;
    }
    const before = this.resBefore();
    const events = this.bridge.apply(cmd.placeStation(xMm, yMm));
    const placed = events.find((e) => "StationPlaced" in e) as
      | { StationPlaced: { id: number } }
      | undefined;
    const id = placed ? placed.StationPlaced.id : -1;
    this.selectedStation = id >= 0 ? id : this.selectedStation; // show its catchment
    if (id >= 0) {
      const [slng, slat] = mmToLngLat([xMm, yMm]);
      this.effects.ripple(slng, slat); // selection-blue placement ring (at the snapped cell)
      audio.place();
      this.floatSpend(slng, slat, before); // float any gold the station cost
    }
    this.refresh();
    return id;
  }

  /** Station tool click: preview a GHOST at the snapped cell (one-per-cell checked up front) for the
   *  player to CONFIRM in the UI — not an instant commit. Re-clicking another cell moves the ghost. */
  ghostStation(lng: number, lat: number): void {
    const [rawX, rawY] = lngLatToMm([lng, lat]);
    const { xMm, yMm, cell } = this.snapToCell(rawX, rawY);
    if (cell && this.cellOccupied(cell)) {
      this.notice = "One station per cell — this hex already has a station";
      this.pendingStation = null;
      this.refresh();
      for (const cb of this.onChange) cb();
      return;
    }
    const [slng, slat] = mmToLngLat([xMm, yMm]);
    this.pendingStation = { lng: slng, lat: slat, xMm, yMm };
    this.refresh();
    for (const cb of this.onChange) cb();
  }

  /** Commit the pending ghost station (the confirm bar's ✓ / Enter): one PlaceStation Command. */
  confirmPendingStation(): number {
    const p = this.pendingStation;
    if (!p) return -1;
    this.pendingStation = null;
    const id = this.placeStation(p.lng, p.lat); // snap + dedup re-checked inside
    for (const cb of this.onChange) cb();
    return id;
  }

  /** Discard the pending ghost (the confirm bar's ✗ / Esc / tool change). */
  cancelPendingStation(): void {
    if (!this.pendingStation) return;
    this.pendingStation = null;
    this.refresh();
    for (const cb of this.onChange) cb();
  }

  /** Place a BARRACKS (fantasy) — a node that fields AI legions. Mirrors placeStation but emits the
   *  fantasy command; the transit ruleset rejects it (no node created), so the tool is fantasy-only. */
  placeBarracks(lng: number, lat: number): number {
    const [x_mm, y_mm] = lngLatToMm([lng, lat]);
    const before = this.resBefore();
    const events = this.bridge.apply(cmd.placeBarracks(x_mm, y_mm));
    const placed = events.find((e) => "BarracksPlaced" in e) as { BarracksPlaced: { id: number } } | undefined;
    const id = placed ? placed.BarracksPlaced.id : -1;
    if (id >= 0) {
      this.selectedStation = id;
      this.effects.ripple(lng, lat);
      audio.place();
      this.floatSpend(lng, lat, before);
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

  /** TTD L2: set a station's platform berth count (clamped [1, MAX_PLATFORMS] in the core — the UI reads
   *  the committed value back). K berths let K consists dwell in parallel. One undoable Command. */
  buildPlatforms(stationId: number, k: number): void {
    this.noteRejections(this.bridge.apply(cmd.buildPlatforms(stationId, k)));
    this.refresh();
  }

  /** Current platform-berth count for a station (read from the snapshot; default 1). */
  stationPlatforms(id: number): number {
    return this.bridge.stationsView()[id]?.platformCount ?? 1;
  }

  /** Display name for a station (snapshot read) — the station panel title. */
  stationName(id: number): string {
    return this.bridge.stationsView()[id]?.name || `Station ${id + 1}`;
  }

  /** Buy a tech upgrade (fantasy, S11) — spends tribute, sets the tech's bit. The core afford-gates +
   *  rejects a repeat/unknown/broke unlock (no mutation), so the UI can fire optimistically and resync
   *  from the next snapshot. The tech panel calls this; `tech` is an index into the tech table. */
  unlockTech(tech: number): void {
    this.bridge.apply(cmd.unlockTech(tech));
    this.refresh();
  }

  /** Cast a spell (fantasy, S11) — auto-targeted, spends mana. The core gates on the SPELLCRAFT tech +
   *  afford + a valid target (rejects with no mutation otherwise), so the UI fires optimistically and
   *  resyncs from the next snapshot. The spell bar calls this; `kind` is an index into the spell table. */
  castSpell(kind: number): void {
    // Surface a no-op cast (not enough mana / no valid target) as the transient toast + alert chime, so a
    // press that does nothing still has a visible echo (AGENTS: every Command needs feedback).
    this.noteRejections(this.bridge.apply(cmd.castSpell(kind)));
    this.refresh();
  }

  /** Toggle autocast (fantasy, S11) — on = the AI auto-fires spells at the biggest threat each tick;
   *  off (default) = spells fire only on `castSpell`. Command-sourced (lives in the save/replay). */
  setAutocast(enabled: boolean): void {
    this.bridge.apply(cmd.setAutocast(enabled));
    this.refresh();
  }

  /** Commit a line through the given ordered station ids (CreateLine + AddStop*). The
   *  interactive draw gesture (T11) and the test hook both funnel here. All-or-nothing: if any
   *  AddStop is rejected (the afford-gate mid-sequence), the whole line is rolled back with a
   *  RemoveLine — a committed network never silently differs from the blueprint that was drawn.
   *  (The log stays append-only; the rollback is itself a Command.) */
  /** Snapshot the three spendable resources (gold / mana / manpower) before a build, so the spend can
   *  be floated afterwards. */
  private resBefore(): [number, number, number] {
    const s = this.bridge.stats();
    return [s.tribute, s.mana, s.manpower];
  }

  /** Float the resources a just-applied build SPENT (−X⬢ gold · −X✦ mana · −X⚔ manpower) at `lng,lat` —
   *  the immediate cost feedback, matching the income/upkeep floats. Inert when nothing was spent. */
  private floatSpend(lng: number, lat: number, before: [number, number, number]): void {
    const s = this.bridge.stats();
    const drops: [number, string, string][] = [
      [Math.round(before[0] - s.tribute), "⬢", "224,96,84"],
      [Math.round(before[1] - s.mana), "✦", "150,120,224"],
      [Math.round(before[2] - s.manpower), "⚔", "206,158,96"],
    ];
    for (const [d, glyph, color] of drops) {
      if (d > 0) this.effects.floatText(lng, lat, `−${d}${glyph}`, color, { rise: 30, size: 16, ttl: 1700 });
    }
  }

  drawLineByIds(ids: number[]): number {
    if (ids.length < 2) return -1;
    const before = this.resBefore(); // capture gold before the build, to float the spend on success
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
      // Float the gold this line cost, at its midpoint (the resource-change feedback).
      const [mlng, mlat] = mmToLngLat(lv.polylineMm[Math.floor(lv.polylineMm.length / 2)]);
      this.floatSpend(mlng, mlat, before);
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

  /** Pick a roster entry — the rolling-stock MODEL — for a line, keeping its train count. The roster is
   *  AIR's aircraft ladder or RAIL's train-model catalog (Standard/Heavy/Express); `spec` is the index. */
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
    // Re-baseline the economy/combat juice so the next run diffs from the current totals (no stale floats
    // after an undo/load reset the cumulative counters). celebratedTowns rebuilds from the live `captured`.
    this.prevJuice.seeded = false;
    this.celebratedTowns.clear();
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
    // Switching tools while drawing drops the in-progress draft — for EITHER draw tool (Track/Service),
    // since they share the draft pipeline (so a Track↔Service switch starts fresh, not mid-chain).
    if (isDrawTool(this.tool) && tool !== this.tool) this.cancelDraft();
    if (this.tool === "station" && tool !== "station") this.pendingStation = null; // drop the ghost on tool change
    if (tool !== this.tool) audio.tick();
    this.tool = tool;
    this.refresh();
  }

  /** Select the transport mode for new construction (chorded bottom bar). Switching mode
   *  drops any in-progress draft and arms a DRAW tool so the next draw uses the new mode —
   *  PRESERVING the Service tool if it's the one armed (else default to Track), so picking a
   *  mode mid-Service doesn't silently land a stockless line. */
  setTransport(mode: number): void {
    if (!this.enabledModes.has(mode) || mode === this.transport) return;
    this.cancelDraft();
    this.transport = mode;
    if (!isDrawTool(this.tool)) this.tool = "line";
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

  /** Set the map LENS / view mode (fantasy #5) — emphasises one reading of the map by dimming the rest. */
  setLens(lens: Game["lens"]): void {
    this.lens = lens;
    this.refresh();
  }

  /** Does the ACTIVE map lens already hide `layerId`? Drives disabling a layer-toggle whose layer the lens
   *  overrides — so the player never sees a toggle reading ON while its layer is invisible. */
  lensHides(layerId: string): boolean {
    if (this.ruleset !== "arcadia" || this.lens === "realm") return false;
    return LENS_HIDE[this.lens]?.has(layerId) ?? false;
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

  /** Toggle the TTD signals overlay (single-track block state: green/red/amber). */
  setShowSignals(on: boolean): void {
    this.showSignals = on;
    this.refresh();
  }

  /** TTD signal markers (single-track block state) as lng/lat + aspect, or [] when the lens is off. The
   *  core fills `[x_m, y_m, status, ...]`; we convert metres→lng/lat through coords/geo.ts (the one
   *  coordinate crossing). Read-only — no sim state, no Command. */
  private signalMarkers(): import("./render").SignalMarker[] {
    if (!this.showSignals) return [];
    const raw = this.bridge.signalMarkers();
    const out: import("./render").SignalMarker[] = [];
    for (let i = 0; i + 2 < raw.length; i += 3) {
      const [lng, lat] = metersToLngLat([raw[i], raw[i + 1]]);
      out.push({ lng, lat, aspect: raw[i + 2] });
    }
    return out;
  }

  // --- TTD L5c: player-placed block signals (a CONTEXTUAL per-line map interaction, no new tool) ---

  /** Screen-pixel hit radius for grabbing/placing a signal on a single-track span (Fitts: a generous
   *  screen-pixel constant, not metres, so it stays tappable when zoomed out). */
  private static readonly SIGNAL_PX = SNAP_PX;

  /** The PLAYER-PLACED block signals (TTD L5c) for the SELECTED line, as lng/lat + their `(line,path,span,
   *  atMm)` address, with the snap candidate flagged. The core fills the authoritative store as
   *  `[line, path, span, at_mm, x_m, y_m, ...]`; we convert metres→lng/lat through coords/geo.ts (the one
   *  coordinate crossing). Shown only when their line is the selected line in build mode (so the clean map
   *  isn't peppered with posts). Read-only — no sim state, no Command. */
  placedSignals(): PlacedSignalMarker[] {
    if (this.mode !== "build" || this.selectedLine === null) return [];
    const raw = this.bridge.placedSignals();
    const out: PlacedSignalMarker[] = [];
    const snap = this.signalSnap?.kind === "remove" ? this.signalSnap : null;
    for (let i = 0; i + 5 < raw.length; i += 6) {
      const line = raw[i];
      if (line !== this.selectedLine) continue; // only the selected line's posts
      const path = raw[i + 1];
      const span = raw[i + 2];
      const atMm = raw[i + 3];
      const [lng, lat] = metersToLngLat([raw[i + 4], raw[i + 5]]);
      const isSnap = snap !== null && snap.line === line && snap.path === path && snap.span === span && snap.atMm === atMm;
      out.push({ lng, lat, line, path, span, atMm, snap: isSnap });
    }
    return out;
  }

  /** The pre-commit PLACE ghost (TTD L5c): the translucent post the next click would drop, or null. */
  signalGhost(): SignalGhost | null {
    return this.signalSnap?.kind === "place" ? { lng: this.signalSnap.lng, lat: this.signalSnap.lat } : null;
  }

  /** Resolve what a signal gesture at a screen pixel would DO for the selected line (no mutation): remove an
   *  existing placed signal under the cursor (priority), else place a new one on the nearest SINGLE-track span
   *  of the selected line — projecting the click onto that span's polyline and mapping the in-span fraction
   *  into the SIM-frame arc-length (`at_mm`). Returns null if neither is in reach (or no line is selected /
   *  not in build mode). The single geometry path shared by the live highlight (`mousemove`) and the commit
   *  (`click`), so the ghost can never disagree with what commits. Routes lng/lat→mm via coords/geo.ts. */
  signalCandidateAt(px: number, py: number): NonNullable<Game["signalSnap"]> | null {
    if (this.mode !== "build" || this.selectedLine === null) return null;
    const lineId = this.selectedLine;
    const lv = this.bridge.linesView()[lineId];
    if (!lv || lv.removed || lv.polylineMm.length < 2) return null;

    // 1) REMOVE: an existing placed signal of this line within the snap radius (screen space) wins.
    {
      let best: { line: number; path: number; span: number; atMm: number; lng: number; lat: number } | null = null;
      let bestD = Game.SIGNAL_PX;
      const raw = this.bridge.placedSignals();
      for (let i = 0; i + 5 < raw.length; i += 6) {
        if (raw[i] !== lineId) continue;
        const [lng, lat] = metersToLngLat([raw[i + 4], raw[i + 5]]);
        const p = this.map.project([lng, lat]);
        const d = Math.hypot(p.x - px, p.y - py);
        if (d <= bestD) {
          bestD = d;
          best = { line: raw[i], path: raw[i + 1], span: raw[i + 2], atMm: raw[i + 3], lng, lat };
        }
      }
      if (best) return { kind: "remove", ...best };
    }

    // 2) PLACE: project the click onto the nearest SINGLE-track span of the trunk and derive `at_mm`. The
    //    trunk polyline is the render-smoothed geometry, but STOPS PIN the span boundaries (same vertex in
    //    both frames), so the span index + the in-span fraction map exactly onto the SIM arc-length table
    //    (`stopArclenMm`) that `Signal.at_mm` lives in — keeping `at_mm` authoritative without the smoothing.
    const poly = lv.polylineMm; // mm
    const stopArc = lv.stopArclenMm ?? [];
    if (stopArc.length < 2) return null;
    // Project each polyline vertex to screen once; walk segments, tracking which SPAN we're in by matching
    // a vertex to its stop arc-length (stops are exact polyline vertices). Find the closest point on a
    // single-track span.
    const screen = poly.map(([x, y]) => {
      const [lng, lat] = mmToLngLat([x, y]);
      const p = this.map.project([lng, lat]);
      return [p.x, p.y] as [number, number];
    });
    // Per-vertex SIM arc-length: vertex i sits at smoothed arc-fraction (cumulative mm)/total, mapped onto
    // the SIM total. Stops pin, so we anchor at each stop vertex and interpolate by smoothed length between.
    const stopPosMm = this.stopPositionsMm(lv.stops);
    const simArc = this.vertexSimArclen(poly, stopArc, stopPosMm);
    let best: { span: number; atMm: number; lng: number; lat: number } | null = null;
    let bestD = Game.SIGNAL_PX;
    for (let i = 1; i < screen.length; i++) {
      const a = screen[i - 1];
      const b = screen[i];
      const dx = b[0] - a[0];
      const dy = b[1] - a[1];
      const l2 = dx * dx + dy * dy;
      let t = l2 > 0 ? ((px - a[0]) * dx + (py - a[1]) * dy) / l2 : 0;
      t = Math.max(0, Math.min(1, t));
      const d = Math.hypot(px - (a[0] + t * dx), py - (a[1] + t * dy));
      if (d > bestD) continue;
      // SIM arc-length at the projected point along this segment.
      const segSim = simArc[i - 1] + (simArc[i] - simArc[i - 1]) * t;
      // Which span does this arc-length fall in? (strictly inside, not on a stop gate)
      const span = this.spanOfArclen(stopArc, segSim);
      if (span < 0) continue;
      if ((lv.trackTypes[span] ?? 0) !== 1) continue; // SINGLE-track spans only (1 = Single)
      const lo = stopArc[span];
      const hi = stopArc[span + 1];
      // Clamp strictly inside the span (the core rejects on-gate signals); keep a 1mm margin off each stop.
      const atMm = Math.round(Math.min(hi - 1, Math.max(lo + 1, segSim)));
      if (atMm <= lo || atMm >= hi) continue;
      // The committed post self-positions via the SIM point_at(at_mm); echo that here so the ghost sits
      // exactly where the post will land (route mm→lng/lat through geo.ts).
      const [glng, glat] = this.simPointAtLngLat(poly, simArc, atMm);
      bestD = d;
      best = { span, atMm, lng: glng, lat: glat };
    }
    if (best) return { kind: "place", line: lineId, path: 0, ...best };
    return null;
  }

  /** Per-vertex SIM arc-length for the (render-smoothed) trunk polyline: stops are exact polyline vertices
   *  (pinned across smoothing — they survive the Chaikin/Catmull pass), so we ANCHOR each stop vertex to its
   *  SIM `stopArclenMm` and distribute the SIM span length across the interior vertices in proportion to their
   *  SMOOTHED chord lengths. The stop vertices are found by matching each station's mm position to the nearest
   *  polyline vertex (in order) — robust to a variable per-span vertex count (grid smoothing isn't uniform).
   *  Pure geometry over mm arrays — never reads the camera, so it's frame-stable. */
  private vertexSimArclen(poly: [number, number][], stopArc: number[], stopPosMm: ([number, number] | null)[]): number[] {
    const n = poly.length;
    const out = new Array<number>(n).fill(0);
    // Smoothed cumulative chord length per vertex.
    const cum = new Array<number>(n).fill(0);
    for (let i = 1; i < n; i++) cum[i] = cum[i - 1] + Math.hypot(poly[i][0] - poly[i - 1][0], poly[i][1] - poly[i - 1][1]);
    const stopVtx = this.stopVertexIndices(poly, stopArc.length, stopPosMm);
    for (let k = 0; k + 1 < stopVtx.length; k++) {
      const v0 = stopVtx[k];
      const v1 = stopVtx[k + 1];
      const simLo = stopArc[k];
      const simHi = stopArc[k + 1];
      const smLo = cum[v0];
      const smSpan = cum[v1] - smLo || 1;
      out[v0] = simLo;
      for (let v = v0 + 1; v <= v1; v++) {
        const frac = (cum[v] - smLo) / smSpan;
        out[v] = simLo + frac * (simHi - simLo);
      }
    }
    // Any tail vertices past the last stop (loops aside, there shouldn't be) hold the last stop's arc-length.
    for (let v = stopVtx[stopVtx.length - 1] + 1; v < n; v++) out[v] = stopArc[stopArc.length - 1];
    return out;
  }

  /** The polyline vertex index at each STOP, found by the vertex NEAREST that stop's station position (in
   *  order, monotonic). Stops pin across smoothing, so each station sits exactly on a polyline vertex; this
   *  recovers which one without assuming a uniform per-span vertex count. Falls back to a proportional split
   *  for any stop whose position is unknown (removed station). */
  private stopVertexIndices(poly: [number, number][], count: number, stopPosMm: ([number, number] | null)[]): number[] {
    const n = poly.length;
    if (count <= 1) return [0];
    const out: number[] = [0]; // stop 0 is always vertex 0
    let from = 1;
    for (let k = 1; k < count; k++) {
      const pos = stopPosMm[k];
      if (k === count - 1) { out.push(n - 1); break; } // last stop is the last vertex
      if (!pos) { out.push(Math.round((k * (n - 1)) / (count - 1))); from = out[out.length - 1] + 1; continue; }
      let bestV = from;
      let bestD = Infinity;
      for (let v = from; v < n - 1; v++) {
        const d = (poly[v][0] - pos[0]) ** 2 + (poly[v][1] - pos[1]) ** 2;
        if (d < bestD) { bestD = d; bestV = v; }
      }
      out.push(bestV);
      from = bestV + 1;
    }
    return out;
  }

  /** mm positions of a line's stop station ids (null for a removed/unknown station). */
  private stopPositionsMm(stops: number[]): ([number, number] | null)[] {
    const sv = this.bridge.stationsView();
    return stops.map((id) => {
      const s = sv[id];
      return s && !s.removed ? ([s.xMm, s.yMm] as [number, number]) : null;
    });
  }

  /** The span index a SIM arc-length falls STRICTLY inside (between two stop boundaries), or -1 if on a stop
   *  gate / out of range. Mirrors the core's `Path::strictly_inside` (a signal can't sit on a station). */
  private spanOfArclen(stopArc: number[], s: number): number {
    for (let sp = 0; sp + 1 < stopArc.length; sp++) {
      if (s > stopArc[sp] && s < stopArc[sp + 1]) return sp;
    }
    return -1;
  }

  /** lng/lat of a SIM arc-length `atMm` along the (smoothed) trunk polyline — the render echo of the core's
   *  `Path::point_at`, so the ghost/highlight sits exactly where the committed post self-positions. Walks the
   *  per-vertex SIM arc-lengths, lerps the bracketing vertices, then crosses to lng/lat via coords/geo.ts. */
  private simPointAtLngLat(poly: [number, number][], simArc: number[], atMm: number): [number, number] {
    for (let i = 1; i < poly.length; i++) {
      if (atMm <= simArc[i]) {
        const seg = simArc[i] - simArc[i - 1] || 1;
        const t = (atMm - simArc[i - 1]) / seg;
        const x = poly[i - 1][0] + (poly[i][0] - poly[i - 1][0]) * t;
        const y = poly[i - 1][1] + (poly[i][1] - poly[i - 1][1]) * t;
        return mmToLngLat([x, y]);
      }
    }
    const last = poly[poly.length - 1];
    return mmToLngLat(last);
  }

  /** Commit the signal gesture at a screen pixel (a contextual click on the selected line in build mode):
   *  REMOVE the placed signal under the cursor, else PLACE a new one on the nearest single-track span. One
   *  undoable Command each (PlaceSignal / RemoveSignal). Returns true if it acted (so the pointer doesn't
   *  fall through to deselect). The Build/Run wall holds — `signalCandidateAt` is null outside build mode. */
  signalGestureAt(px: number, py: number): boolean {
    const c = this.signalCandidateAt(px, py);
    if (!c) return false;
    if (c.kind === "remove") {
      this.bridge.apply(cmd.removeSignal(c.line, c.path, c.span, c.atMm));
    } else {
      this.bridge.apply(cmd.placeSignal(c.line, c.path, c.span, c.atMm));
      audio.place();
      this.effects.ripple(c.lng, c.lat); // selection-blue placement echo (sub-100 ms acknowledgement)
    }
    this.signalSnap = null;
    this.refresh();
    return true;
  }

  /** Camera-independent test hook: place a signal at a lng/lat by the SAME production path a click takes —
   *  project to the screen pixel, then run the gesture (so the e2e exercises the geo.ts coordinate boundary
   *  + the real span/at_mm geometry, not a second one). Returns true if a signal was placed/removed. */
  placeSignalLngLat(lng: number, lat: number): boolean {
    const p = this.map.project([lng, lat]);
    return this.signalGestureAt(p.x, p.y);
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

  /** Fly the IMPERATIVE map camera to an alert's anchor station and pin it, so the badge + the map
   *  always agree (AGENTS render-hot-path: this drives the existing MapLibre flyTo seam directly —
   *  NOT a React rAF). Called from the top-centre AlertCluster when a ping is clicked. `tool` (when
   *  given) arms the build tool that fixes the pressure (e.g. Track to extend a starved station's
   *  service), so the click both shows AND prepares the remedy. The station id comes straight from a
   *  `stats.perStation[]` index, so there's no parallel heuristic to drift. No-op for a stale id. */
  flyToAlert(stationId: number, tool?: Tool): void {
    const ll = this.stationLngLat(stationId);
    if (!ll) return;
    // Pin the station (mounts the inspector + draws its catchment) and pan to it. easeTo keeps the
    // camera move imperative + interruptible; the reduced-motion preference jumps instead.
    this.selectStation(stationId);
    if (tool) {
      if (this.mode === "run") this.setMode("build"); // tools only arm behind the Build wall
      this.setTool(tool);
    }
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches;
    const z = Math.max(this.map.getZoom(), 12);
    if (reduce) this.map.jumpTo({ center: ll, zoom: z });
    else this.map.easeTo({ center: ll, zoom: z, duration: 700, essential: true });
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
      if (p.short > 0) {
        this.notice = `Not enough money — ${fmtMoney(p.short)} short`;
        audio.alert();
        this.refresh();
        return;
      }
      if (p.goldShort > 0) {
        this.notice = `Not enough gold — ${p.goldShort}⬢ short (deliver supply or build a shorter route)`;
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
      const service = this.tool === "service"; // capture before drawLineByIds (which doesn't touch the tool)
      const lineId = this.drawLineByIds(ids);
      // drawLineByIds is all-or-nothing (rolls back on any rejection), so a returned line has
      // every drafted stop and the per-span waypoints line up 1:1.
      if (lineId >= 0 && wps.some((s) => s.length > 0)) {
        this.noteRejections(this.bridge.apply(cmd.setLineWaypoints(lineId, wps)));
        this.refresh();
      }
      // TTD L6: the SERVICE tool lands a live coloured line — auto-assign a default fleet (+ auto-headway)
      // so it runs at once, distinguishing it from the Track tool's bare grey corridor. The Track tool
      // leaves it stockless. The player tunes count/model/headway in the editor (which the commit opened).
      if (service && lineId >= 0) this.assignTrainset(lineId, DEFAULT_SERVICE_TRAINS);
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
    // Fantasy grid: expand each consecutive pair into the cost-aware one-bend lattice route, so the
    // blueprint previews EXACTLY the track that commits (and the preview length/cost read true). The
    // routing mirrors the core (hexgrid line_costed) cell-for-cell, so ghost == committed geometry.
    if (this.cellMm > 0 && pts.length >= 2) return this.gridRoutePoints(pts);
    return pts;
  }

  /** Walk a point sequence into the dense one-bend lattice polyline (mm) — the frontend mirror of the
   *  core's grid_walk: each consecutive pair becomes the cheaper one-bend hex run, scored by the SAME
   *  terrain costs the core uses (so the ghost matches the commit and routes around water/mountains). */
  private gridRoutePoints(pts: [number, number][]): [number, number][] {
    const s = this.cellMm;
    const cost = (c: Axial): number => {
      const [x, y] = centerOf(c[0], c[1], s);
      switch (this.build.classifyMm(x, y)) {
        case 4: return 800; // WATER
        case 6: return 320; // MOUNTAIN
        case 7: return 190; // HILL
        case 8: return 140; // FOREST
        case 9: return 130; // LEY
        default: return 100; // plain / open
      }
    };
    const out: [number, number][] = [];
    for (let i = 0; i < pts.length - 1; i++) {
      const a = axialOf(pts[i][0], pts[i][1], s);
      const b = axialOf(pts[i + 1][0], pts[i + 1][1], s);
      const cells = lineCosted(a, b, cost);
      for (let j = 0; j < cells.length; j++) {
        if (i > 0 && j === 0) continue; // skip the shared joint (prev segment's tail == this head)
        out.push(centerOf(cells[j][0], cells[j][1], s));
      }
    }
    return out;
  }

  /** The draggable control-point handles for the current draft: a solid dot per existing waypoint,
   *  and a faint "+" at every sub-segment midpoint (drag it to bend the track there). lng/lat for
   *  the deck layer; `span`/`index` address `draftWaypoints` (for 'add', the splice index). */
  controlHandles(): { lng: number; lat: number; kind: "waypoint" | "add"; span: number; index: number }[] {
    // No bend handles while EXTENDING: the extension commits straight AddStops (no waypoint
    // vocabulary for "append these bends"), so offering handles would silently drop the bends
    // on commit — the blueprint must never differ from what commits.
    if (!isDrawTool(this.tool) || this.draft.length < 2 || this.extendTarget !== null) return [];
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
  draftPreview(): { stops: number; lengthKm: number; cost: number; invalid: boolean; short: number; goldCost: number; goldShort: number } {
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
    const short = s?.economyEnabled ? Math.max(0, cost - s.balance) : 0; // raw $ overshoot (formatted by the HUD)
    // Fantasy gold build economy (#economy): price the draft in gold + how far it overshoots the treasury.
    // The core afford-gate is still the authority at commit; this is the early warning the player draws against.
    const div = s?.buildGoldDivisor ?? 0;
    const goldCost = div > 0 ? Math.round(cost / div) : 0;
    const goldShort = div > 0 ? Math.max(0, goldCost - (s?.tribute ?? 0)) : 0;
    return { stops: this.draft.length, lengthKm: bent / 1_000_000, cost, invalid: this.draftInvalid(), short, goldCost, goldShort };
  }

  /** Bill-of-materials for the in-progress draft (fantasy grid): how many lattice cells of each terrain
   *  the track crosses + each terrain's SHARE of `total` cost (attributed by relative build weight), so
   *  the player sees WHERE the cost comes from — "2× water" is why a line is dear. Empty off-grid. */
  draftBom(total: number): { kind: string; count: number; cost: number; tint: string }[] {
    if (this.cellMm <= 0 || this.draft.length < 1) return [];
    const pts = this.draftPointsMm(); // the dense one-bend route (cell centres)
    if (pts.length < 2) return [];
    // [label, relative build weight, chip tint] keyed by biome class (mirrors the routing cost order).
    const W: Record<number, [string, number, string]> = {
      10: ["plains", 1.0, "150,165,150"],
      4: ["water", 8.0, "90,130,200"],
      6: ["mountain", 3.2, "120,110,110"],
      7: ["hill", 1.9, "150,140,120"],
      8: ["forest", 1.4, "110,150,110"],
      9: ["ley", 1.3, "170,130,210"],
    };
    const groups = new Map<number, { count: number; weight: number }>();
    for (const [x, y] of pts) {
      const cls = this.build.classifyMm(x, y);
      const key = W[cls] ? cls : 10; // unknown / open → plains
      const g = groups.get(key) ?? { count: 0, weight: 0 };
      g.count++;
      g.weight += W[key][1];
      groups.set(key, g);
    }
    let wsum = 0;
    for (const g of groups.values()) wsum += g.weight;
    const out = [...groups.entries()].map(([cls, g]) => ({
      kind: W[cls][0],
      count: g.count,
      cost: wsum > 0 ? Math.round(total * (g.weight / wsum)) : 0,
      tint: W[cls][2],
    }));
    out.sort((a, b) => b.cost - a.cost || b.count - a.count);
    return out;
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
    const sunk = this.lastStats.economyEnabled && capitalCost > 0 ? ` — ${fmtMoney(capitalCost)} build cost written off` : "";
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

  /** Nearest player vehicle (cart) to a screen pixel, within `maxPx` — returns its index or null. Reads
   *  the same interpolated dots the layer draws, so the pick can't drift from what's on screen. */
  nearestVehicle(px: number, py: number, maxPx = SNAP_PX): number | null {
    let best: number | null = null;
    let bestD = maxPx;
    const dots = this.vehicleDotsAt(1);
    for (let i = 0; i < dots.length; i++) {
      const p = this.map.project([dots[i].lng, dots[i].lat]);
      const d = Math.hypot(p.x - px, p.y - py);
      if (d <= bestD) { bestD = d; best = i; }
    }
    return best;
  }

  /** Nearest baked TOWN to a screen pixel (fantasy conquest target / supply sink), within `maxPx`. */
  nearestTown(px: number, py: number, maxPx = SNAP_PX + 4): number | null {
    let best: number | null = null;
    let bestD = maxPx;
    for (let i = 0; i < this.towns.length; i++) {
      const p = this.map.project([this.towns[i].lng, this.towns[i].lat]);
      const d = Math.hypot(p.x - px, p.y - py);
      if (d <= bestD) { bestD = d; best = i; }
    }
    return best;
  }

  /** Nearest baked RESOURCE node to a screen pixel (fantasy supply source), within `maxPx`. */
  nearestResource(px: number, py: number, maxPx = SNAP_PX + 4): number | null {
    let best: number | null = null;
    let bestD = maxPx;
    for (let i = 0; i < this.resources.length; i++) {
      const p = this.map.project([this.resources[i].lng, this.resources[i].lat]);
      const d = Math.hypot(p.x - px, p.y - py);
      if (d <= bestD) { bestD = d; best = i; }
    }
    return best;
  }

  /** Open the inspect context menu at (px,py). Right-click inspects WHATEVER is under the cursor: a moving
   *  cart or rider first (small + on top), then a station/line you built, then the baked town/resource POIs,
   *  else the empty-map power tools. Read-only — every entry routes to an existing inspect path, no Command. */
  openContextMenu(px: number, py: number, lngLat: { lng: number; lat: number }): void {
    const mk = (kind: ContextMenuState["kind"], id: number): ContextMenuState => ({ x: px, y: py, lngLat, kind, id });
    const veh = this.nearestVehicle(px, py);
    const peep = veh === null ? this.nearestPeep(px, py) : null;
    const st = veh === null && peep === null ? this.nearestStation(px, py) : null;
    if (veh !== null) this.contextMenu = mk("vehicle", veh);
    else if (peep !== null) this.contextMenu = mk("peep", peep);
    else if (st !== null) this.contextMenu = mk("station", st);
    else {
      const ln = this.nearestLine(px, py);
      if (ln !== null) this.contextMenu = mk("line", ln);
      else {
        const tn = this.nearestTown(px, py);
        if (tn !== null) this.contextMenu = mk("town", tn);
        else {
          const rs = this.nearestResource(px, py);
          this.contextMenu = rs !== null ? mk("resource", rs) : mk("empty", -1);
        }
      }
    }
    for (const cb of this.onChange) cb();
  }

  closeContextMenu(): void {
    if (this.contextMenu === null) return;
    this.contextMenu = null;
    for (const cb of this.onChange) cb();
  }

  /** The baked POI (town/resource) co-located with a station, if any — every fantasy station sits ON a
   *  town or resource node, so its inspect surfaces that node's supply-chain role (a town's tribute,
   *  needs + decadence; a resource's yield). Matched by position (within ~one terrain cell). */
  stationPoi(stationId: number): { town?: TownMarker; resource?: ResourceMarker } | null {
    const s = this.bridge.stationsView()[stationId];
    if (!s || s.removed) return null;
    const tol2 = (this.terrainCellM * 1000 || 250_000) ** 2;
    let town: TownMarker | undefined, resource: ResourceMarker | undefined;
    let bt = tol2, br = tol2;
    for (const t of this.towns) {
      const [x, y] = lngLatToMm([t.lng, t.lat]);
      const d = (x - s.xMm) ** 2 + (y - s.yMm) ** 2;
      if (d <= bt) { bt = d; town = t; }
    }
    for (const r of this.resources) {
      const [x, y] = lngLatToMm([r.lng, r.lat]);
      const d = (x - s.xMm) ** 2 + (y - s.yMm) ** 2;
      if (d <= br) { br = d; resource = r; }
    }
    return town || resource ? { town, resource } : null;
  }

  /** Read-only inspect summary for a cart (right-click → Inspect): its line identity + how much it hauls
   *  (onboard / capacity — the cargo or riders aboard). Null if the index is stale. */
  vehicleInspect(i: number): { lineId: number; name: string; color: number; onboard: number; capacity: number } | null {
    const lineIds = this.bridge.vehicleLineIds();
    if (i < 0 || i >= lineIds.length) return null;
    const loads = this.bridge.vehicleLoads();
    const lineId = lineIds[i];
    const pl = this.perLineById.get(lineId);
    return { lineId, name: pl?.name || `Line ${lineId + 1}`, color: pl?.color ?? 0x888888, onboard: loads[i * 2] ?? 0, capacity: loads[i * 2 + 1] ?? 0 };
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
          faction: s.faction ?? 0, // #13: 1 = rival realm → crimson tint
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

    // TTD L6 (track + services): which lines carry stock. `LineStat.trains` is the ASSIGNED trainset count
    // (not dispatched), so this is correct in Build mode too — a freshly-assigned line lights up at once. A
    // line absent from perLine (or with 0) is bare TRACK ⇒ rendered as grey infrastructure, not a service.
    const servicedLines = new Set(this.bridge.stats().perLine.filter((l) => l.trains > 0).map((l) => l.lineId));
    const lines = linesV
      .filter((l) => !l.removed)
      .flatMap((l) => {
        // A line with surface track over water renders red until elevated/tunnelled.
        const color = l.crossesWaterSurface ? ([214, 40, 40] as [number, number, number]) : colorToRgb(l.color);
        const mode = l.mode; // heavy/high-speed rail (4) gets distinct mainline styling
        const raided = (l.raidedRemainingMs ?? 0) > 0; // #war: a raider has CUT this line (trains frozen)
        const serviced = servicedLines.has(l.id); // false ⇒ bare track (grey infra), true ⇒ coloured service
        // The trunk, plus one path per branch (P3) — all the same id/colour so a Y-shaped line
        // (e.g. the Circle Line's Marina Bay spur) draws as one coloured service.
        const paths = [l.polylineMm, ...(l.branchPolylinesMm ?? [])];
        return paths
          .filter((p) => p.length >= 2)
          .map((p) => ({ id: l.id, color, path: p.map(([x, y]) => mmToLngLat([x, y])), mode, raided, serviced }));
      });

    // Rail-attack (#war): a "⚔ RAIDED" badge + recovery countdown at each cut line's midpoint, so the
    // player SEES which supply line a raider severed and how long until it re-opens (the front pressure).
    const raidLabels: RaidLabel[] = [];
    for (const l of linesV) {
      if (l.removed || (l.raidedRemainingMs ?? 0) <= 0 || l.polylineMm.length < 2) continue;
      const [mx, my] = l.polylineMm[Math.floor(l.polylineMm.length / 2)];
      const [lng, lat] = mmToLngLat([mx, my]);
      raidLabels.push({ lng, lat, text: `⚔ RAIDED ${Math.ceil((l.raidedRemainingMs ?? 0) / 1000)}s` });
    }

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
    const bufferPips: BufferPip[] = [];
    for (const ps of this.lastStats.perStation) {
      const s = stationsV[ps.stationId];
      if (!s) continue;
      if (ps.waiting > 0) {
        const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
        waiting.push({ lng, lat, count: ps.waiting });
      }
      // Node Forge-Line buffer gauge (#8): show only a meaningfully-filled buffer (slate→amber→red).
      if (ps.bufferFill > 0.12) {
        const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
        bufferPips.push({ lng, lat, fill: ps.bufferFill });
      }
    }

    // Connected-rail frontier (#infrastructure): the realm's network must be ONE graph rooted at the capital
    // — rail extends only from a station already wired to your seat (or a captured town). So the affordance is
    // per-NODE, not a radius: a gold halo rings every RAIL-REACHABLE station ("grow rail from here"). The
    // `reachable` flag is the core's own gate output (zero drift); before any line it is just the capital,
    // spreading as you build + conquer. Roots (the capital + captured towns) read brighter — a fresh line may
    // always seed there. Empty unless the gate is on.
    const frontier: FrontierNode[] = [];
    if (this.ruleset === "arcadia" && this.influenceHops > 0) {
      const capTown = this.towns.find((t) => t.kind === "capital");
      for (const ps of this.lastStats.perStation) {
        if (!ps.reachable) continue;
        const s = stationsV[ps.stationId];
        if (!s || s.removed) continue;
        const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
        const atCapital = !!capTown && Math.abs(lng - capTown.lng) < 1e-6 && Math.abs(lat - capTown.lat) < 1e-6;
        frontier.push({ lng, lat, root: ps.captured || atCapital });
      }
    }

    // #war legibility: SIEGE-progress rings (a town being ground down by a besieging legion — its garrison
    // shrinking) + BARRACKS badges (the ⚔ legion-spawn nodes). Both off the ~3 Hz snapshot. A town is under
    // active contest when its garrison sits between 0 and full; progress = how ground down it is (the red
    // pressure builds as capture nears). Barracks tint by global readiness (manpower ≥ a legion's cost).
    const siegeRings: SiegeRing[] = [];
    const barracksBadges: BarracksBadge[] = [];
    if (this.ruleset === "arcadia") {
      const ready = (this.lastStats.manpower ?? 0) >= LAUNCH_COST_MANPOWER;
      for (const ps of this.lastStats.perStation) {
        const s = stationsV[ps.stationId];
        if (!s || s.removed) continue;
        const gmax = ps.garrisonMax ?? 0;
        if (gmax > 0 && ps.townResistance > 0 && ps.townResistance < gmax) {
          const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
          siegeRings.push({ lng, lat, progress: 1 - ps.townResistance / gmax }); // 0 just-engaged → 1 about to fall
        }
        if (ps.isBarracks) {
          const [lng, lat] = mmToLngLat([s.xMm, s.yMm]);
          barracksBadges.push({ lng, lat, ready });
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
    const showRoads = this.showRoads || (this.mode === "build" && isDrawTool(this.tool) && this.transport === 1);
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
      trees: this.ruleset === "arcadia" ? this.trees : [], // 3D diorama pines; LOD-dropped at overview in composeAndSet
      tideCells: this.decadenceTideAt(), // fantasy S10c: the cold decadence creep (read on each refresh)
      tidePulse: this.nextTidePulse(), // tide-frontier ring alpha, advanced per ~3 Hz recompose (not per frame)
      arcadia: this.ruleset === "arcadia", // cold-violet demand overlay + arcadia LOD

      resources: this.resources, // baked fantasy supply-chain source nodes; empty for transit cities
      towns: this.towns, // baked fantasy towns (sinks + conquest targets); empty for transit cities
      decadenceAnchors: this.decadenceAnchors, // baked far-edge reservoir anchors; empty for transit cities
      rivers: this.rivers, // baked flow-accumulation drainage (cold water); empty for transit cities
      vehicles: [],
      waiting,
      bufferPips,
      frontier, // #infrastructure rail-frontier node-halos (where rail may extend); empty unless the gate is on
      raidLabels, // #war: "⚔ RAIDED" badges + countdown on cut lines; empty unless a raider severed one
      siegeRings, // #war: siege-progress rings on towns being ground down; empty unless a siege is live
      barracksBadges, // #war: ⚔ markers on legion-spawn nodes; empty for transit
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
      ghostStation: this.pendingStation ? { lng: this.pendingStation.lng, lat: this.pendingStation.lat } : null,
      nodePlates: this.nodePlatesView(),
    };
  }

  /** Node nameplates (fantasy): a name + key-stat plate over each town/resource node. The name is the
   *  co-located station's (matched by hex cell); the stats come from the baked node data (a town's
   *  tribute + needs; a resource's kind + yield). Empty for transit cities. */
  private nodePlatesView(): import("./render").NodePlate[] {
    const cm = this.cellMm;
    if (cm <= 0) return [];
    const out: import("./render").NodePlate[] = [];
    const nameByCell = new Map<string, string>();
    for (const s of this.bridge.stationsView()) {
      if (!s.removed) nameByCell.set(`${Math.floor(s.xMm / cm)},${Math.floor(s.yMm / cm)}`, s.name);
    }
    const nameAt = (lng: number, lat: number): string => {
      const [x, y] = lngLatToMm([lng, lat]);
      return nameByCell.get(`${Math.floor(x / cm)},${Math.floor(y / cm)}`) ?? "";
    };
    for (const t of this.towns) {
      const needs = t.chain === "bread" ? "needs grain+fuel" : t.chain === "arms" ? "needs ore+aether" : "";
      const title = nameAt(t.lng, t.lat) || (t.kind === "capital" ? "The Capital" : t.kind === "starter" ? "Your Hold" : "Town");
      // The capital is your seat (its conquest "value" is 0); other towns show their worth + what to supply.
      const sub = t.kind === "capital" ? "the realm's seat" : `⚜${t.value.toLocaleString()}${needs ? ` · ${needs}` : ""}`;
      out.push({ lng: t.lng, lat: t.lat, title, sub });
    }
    for (const r of this.resources) {
      const title = nameAt(r.lng, r.lat) || r.kind;
      out.push({ lng: r.lng, lat: r.lat, title, sub: `yield ${r.yield}` }); // title already names the good
    }
    return out;
  }

  /** The pre-commit snap highlight datum (or null): the station the next click would chain
   *  (line tool) or demolish (bulldozer), set by the pointer per mousemove. */
  private snapRingView(): { lng: number; lat: number; demolish: boolean } | null {
    if (this.snapStation === null || this.mode !== "build") return null;
    if (!isDrawTool(this.tool) && this.tool !== "bulldozer") return null;
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
  /** Day/night driver, set in boot (arcadia): mutate the scene sun/ambient by sim hour, return the
   *  0..1 night factor. Kept as a callback so the core game stays decoupled from deck's light types. */
  updateLighting: ((hour: number) => number) | null = null;
  /** 0 = full day, 1 = deep night — fades the warm town/train glows in. Updated on the 3 Hz hour. */
  nightFactor = 0;

  /** A milestone was crossed (rider/coverage record, "you beat the real network") — spray a celebration
   *  at the busiest station (the network's beating heart), or the map centre if nothing's moving yet.
   *  Client-only juice: no Command, no sim read beyond the last snapshot; reduced-motion degrades it to a
   *  single ack inside effects.celebrate. Called from the Beats milestone toast. */
  celebrateMilestone(): void {
    const ps = this.lastStats?.perStation ?? [];
    let bestId = -1;
    let bestB = -1;
    for (const s of ps) {
      if (s.boardings > bestB) {
        bestB = s.boardings;
        bestId = s.stationId;
      }
    }
    const ll = bestId >= 0 ? this.stationLngLat(bestId) : null;
    if (ll) this.effects.celebrate(ll[0], ll[1]);
    else {
      const c = this.map.getCenter();
      this.effects.celebrate(c.lng, c.lat);
    }
  }

  setStats(s: Stats): void {
    this.lastStats = s;
    this.perStationById = new Map(s.perStation.map((ps) => [ps.stationId, ps]));
    this.perLineById = new Map(s.perLine.map((l) => [l.lineId, l]));
    // Day/night: swing the scene sun + ambient by sim hour (two-clocks: rides this 3 Hz slice, not rAF)
    // BEFORE refresh() so the night-glow layers pick up the fresh nightFactor this pass.
    if (this.updateLighting) this.nightFactor = this.updateLighting(s.simHour);
    if (s.running) this.emitStatsJuice(s);
    if (s.running) this.emitWorldJuice(s); // working smoke at forges + delivery pops (arcadia, detail-gated)
    this.updateSupplyFlow(s); // marching cargo pips along busy served lines (arcadia, detail, running)
    this.updateNightLights(); // twinkling settlement windows at night (arcadia, detail)
    if (s.running) this.emitResourceShimmer(); // faint extraction glints at raw resource camps
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
    this.emitEconomyJuice(s, sv, deltas);
    this.emitTrainPuffs();
  }

  /** Floating profit/loss + conquest text driven off the cumulative-stat deltas between snapshots:
   *  "+N⬢" gold where cargo just landed, "+$N" fares (transit), "−$N/day" upkeep on the day roll, and
   *  a "⚔ Conquered!" boom on a town that just fell. Purely client-side acknowledgement (no sim read). */
  private emitEconomyJuice(s: Stats, sv: ReturnType<SimBridge["stationsView"]>, deltas: { id: number; d: number }[]): void {
    const at = (id: number): { lng: number; lat: number } | null => {
      const v = sv[id];
      if (!v || v.removed) return null;
      const [lng, lat] = mmToLngLat([v.xMm, v.yMm]);
      return { lng, lat };
    };
    const prev = this.prevJuice;
    if (!prev.seeded) {
      // First snapshot of a run: record baselines so we don't float the whole accumulated total at once,
      // and mark already-captured towns as celebrated so only NEW falls fire a boom (resume/load safe).
      for (const ps of s.perStation) if (ps.captured) this.celebratedTowns.add(ps.stationId);
      for (const ps of s.perStation) this.prevAlightings.set(ps.stationId, ps.alightings); // baseline (no first-tick float)
      this.prevJuice = { tribute: s.tribute, fare: s.fareRevenue, opex: s.opexSpent, towns: s.townsCaptured, day: s.simDay, seeded: true };
      return;
    }

    const arcadia = this.ruleset === "arcadia";
    // Income float at the busiest delivery point (top boarding-delta station = where cargo/riders moved).
    const top = deltas[0] ? at(deltas[0].id) : null;
    if (arcadia) {
      const dGold = Math.round(s.tribute - prev.tribute);
      // Tribute is earned where cargo ALIGHTS at a sink — float the gold at those delivery spots (spread
      // across the busiest, proportional to deliveries), so the player sees WHERE the realm earned, not a
      // lump at one station. The alighting delta is the per-spot earning signal.
      const earned: { id: number; d: number }[] = [];
      for (const ps of s.perStation) {
        const pa = this.prevAlightings.get(ps.stationId) ?? ps.alightings;
        const d = ps.alightings - pa;
        if (d > 0) earned.push({ id: ps.stationId, d });
        this.prevAlightings.set(ps.stationId, ps.alightings);
      }
      earned.sort((a, b) => b.d - a.d);
      const spots = earned.slice(0, 4);
      if (dGold > 0 && spots.length > 0) {
        const tot = spots.reduce((acc, x) => acc + x.d, 0) || 1;
        let shown = 0;
        spots.forEach((x, i) => {
          const share = i === spots.length - 1 ? dGold - shown : Math.round((dGold * x.d) / tot);
          shown += share;
          const p = at(x.id);
          if (share > 0 && p) this.effects.floatText(p.lng, p.lat, `+${share}⬢`, "235,205,110");
        });
      } else if (dGold > 0 && top) {
        this.effects.floatText(top.lng, top.lat, `+${dGold}⬢`, "235,205,110"); // fallback: lump at the busiest spot
      }
      // Loaded/unloaded CARGO floats (diegetic supply story): goods LOADED at a source (↑) + UNLOADED at a
      // sink (↓), tagged with the node's commodity glyph — so the player watches supply physically MOVE, not
      // just the gold it earns. Loads = boarding deltas (pickup at sources); unloads = the alighting deltas.
      const cm = this.cellMm;
      const glyphByCell = new Map<string, string>();
      if (cm > 0) {
        const put = (lng: number, lat: number, g: string) => {
          const [x, y] = lngLatToMm([lng, lat]);
          glyphByCell.set(`${Math.floor(x / cm)},${Math.floor(y / cm)}`, g);
        };
        for (const r of this.resources) put(r.lng, r.lat, this.cargoOf(r.kind).glyph);
        for (const t of this.towns) if (t.chain) put(t.lng, t.lat, this.cargoOf(t.chain).glyph);
      }
      const glyphAt = (id: number): string => {
        const v = sv[id];
        if (!v || cm <= 0) return "▪";
        return glyphByCell.get(`${Math.floor(v.xMm / cm)},${Math.floor(v.yMm / cm)}`) ?? "▪";
      };
      for (const { id, d } of deltas.slice(0, 3)) {
        const p = at(id);
        if (p) this.effects.floatText(p.lng, p.lat, `↑${d}${glyphAt(id)}`, "150,205,160", { rise: 20, size: 13, ttl: 1400 });
      }
      for (const { id, d } of earned.slice(0, 3)) {
        const p = at(id);
        if (p) this.effects.floatText(p.lng, p.lat, `↓${d}${glyphAt(id)}`, "140,195,225", { rise: 12, size: 13, ttl: 1400 });
      }
    } else if (s.economyEnabled) {
      const dFare = Math.round(s.fareRevenue - prev.fare);
      if (dFare > 0 && top) this.effects.floatText(top.lng, top.lat, `+$${fmtShort(dFare)}`, "120,210,140");
    }

    // Daily upkeep on the day rollover (the recurring drain made legible, as requested).
    if (s.economyEnabled && s.simDay > prev.day) {
      const dOpex = Math.round(s.opexSpent - prev.opex);
      if (dOpex > 0) {
        // Anchor at the network centroid so it reads as a realm-wide charge, not a single station's.
        const live = sv.filter((v) => !v.removed);
        if (live.length) {
          const cx = live.reduce((a, v) => a + v.xMm, 0) / live.length;
          const cy = live.reduce((a, v) => a + v.yMm, 0) / live.length;
          const [lng, lat] = mmToLngLat([cx, cy]);
          this.effects.floatText(lng, lat, `−$${fmtShort(dOpex)}/day`, "224,96,84", { rise: 26, size: 15, ttl: 1900 });
        }
      }
    }
    // Fantasy gold upkeep on the day rollover — the realm pays opex to keep the network running.
    if (arcadia && s.goldUpkeepDaily > 0 && s.simDay > prev.day) {
      const live = sv.filter((v) => !v.removed);
      if (live.length) {
        const cx = live.reduce((a, v) => a + v.xMm, 0) / live.length;
        const cy = live.reduce((a, v) => a + v.yMm, 0) / live.length;
        const [lng, lat] = mmToLngLat([cx, cy]);
        this.effects.floatText(lng, lat, `−${Math.round(s.goldUpkeepDaily)}⬢/day`, "224,96,84", { rise: 26, size: 15, ttl: 1900 });
      }
    }

    // Conquest: a town just fell — boom + "⚔ Conquered!" at each newly-captured holding (once each).
    if (s.townsCaptured > prev.towns) {
      let conquered = false;
      for (const ps of s.perStation) {
        if (ps.captured && !this.celebratedTowns.has(ps.stationId)) {
          this.celebratedTowns.add(ps.stationId);
          const p = at(ps.stationId);
          if (p) {
            this.effects.boom(p.lng, p.lat, "235,180,70");
            this.effects.floatText(p.lng, p.lat, "⚔ Conquered!", "245,200,90", { rise: 40, size: 16, ttl: 2100 });
            conquered = true;
          }
        }
      }
      if (conquered) audio.conquer(); // one triumphant swell per conquest beat (not per town)
    }
    // Territory front (#war): a holding the rival RE-CONTESTED — a reclaimer re-garrisoned a town you took
    // but didn't hold (rail to it / ward it to keep it). Flash "⚔ Lost!" once + FORGET it, so re-taking
    // re-fires the conquest boom — the oscillating front made legible. (Arcadia only; reclaimers don't exist
    // in transit.) Snapshot the lost ids first, then mutate the set (no in-iteration delete).
    if (arcadia && this.celebratedTowns.size > 0) {
      const heldNow = new Set(s.perStation.filter((ps) => ps.captured).map((ps) => ps.stationId));
      const lost = [...this.celebratedTowns].filter((id) => !heldNow.has(id));
      for (const id of lost) {
        this.celebratedTowns.delete(id);
        const p = at(id);
        if (p) {
          this.effects.boom(p.lng, p.lat, "224,64,64");
          this.effects.floatText(p.lng, p.lat, "⚔ Lost!", "230,90,90", { rise: 38, size: 16, ttl: 2100 });
        }
      }
    }

    this.prevJuice = { tribute: s.tribute, fare: s.fareRevenue, opex: s.opexSpent, towns: s.townsCaptured, day: s.simDay, seeded: true };
  }

  /** Steam/dust trail off the moving trains (arcadia steam-era flavour): each stats tick, one puff for a
   *  rotating slice of vehicles, so every train leaves a drifting trail without fogging the map. */
  private emitTrainPuffs(): void {
    if (this.ruleset !== "arcadia") return; // steam reads odd on an electric metro; fantasy carts only
    const pos = this.bridge.vehiclePositions(); // interleaved metres
    const n = pos.length / 2;
    if (n === 0) return;
    const PER_TICK = Math.min(8, n); // cap the spawn rate so the trail stays a wisp, not a cloud
    // #3d: raise the steam to the 3D model's CHIMNEY (cabin top ~110 m) instead of its wheels. Convert that
    // world height to screen px from the live zoom (mercator m/px ≈ 156543·cos(lat)/2^zoom), foreshortened
    // by the camera pitch; clamped so it's a sensible lift at any zoom (tiny at overview, taller up close).
    const zoom = this.map.getZoom();
    const latRad = (this.map.getCenter().lat * Math.PI) / 180;
    const mPerPx = (156543.03 * Math.max(0.01, Math.cos(latRad))) / 2 ** zoom;
    const pitchFactor = Math.max(0.35, Math.cos((this.map.getPitch() * Math.PI) / 180));
    const lift = Math.min(34, (110 / mPerPx) * pitchFactor);
    for (let k = 0; k < PER_TICK; k++) {
      const vi = (this.puffCursor + k) % n;
      const [lng, lat] = metersToLngLat([pos[vi * 2], pos[vi * 2 + 1]]);
      this.effects.puff(lng, lat, lift);
    }
    this.puffCursor = (this.puffCursor + PER_TICK) % n;
  }

  /** Make the supply economy VISIBLY alive (#living-supply): a node's buffer fill RISING between 3 Hz
   *  snapshots means it's working — a SOURCE (demandOrigin>demandDest) is producing → a grey chimney-lifted
   *  smoke wisp; a SINK (a town) just RECEIVED a delivery → a warm deposit pop + "+⬢". Pure outer-ring (the
   *  bufferFill / demand fields are already in the snapshot), pure FX-canvas (no deck rebuild, no sim tick).
   *  Arcadia + zoomed-IN only (the strategic overview stays a clean network read); keeps prevBufferFill
   *  current while gated so re-entering detail doesn't fire a backlog. */
  private emitWorldJuice(s: Stats): void {
    const detail = this.ruleset === "arcadia" && this.map.getZoom() >= DETAIL_ZOOM;
    const producing: number[] = [];
    const delivered: { id: number; d: number }[] = [];
    for (const ps of s.perStation) {
      const prev = this.prevBufferFill.get(ps.stationId) ?? ps.bufferFill;
      const d = ps.bufferFill - prev; // change since the last 3 Hz snapshot
      this.prevBufferFill.set(ps.stationId, ps.bufferFill);
      if (!detail) continue;
      const isSource = ps.demandOrigin > ps.demandDest * 1.5 + 0.5; // a net-source forge/resource
      if (isSource) {
        // A net-source produces into its stockpile every tick UNTIL it's capped (full = idle). The per-
        // snapshot rise is sub-noise, so gate on "has headroom" (bufferFill<0.97) rather than the delta,
        // and fire a smoke wisp probabilistically so the world's forges puff gently + variably rather than
        // all at once (Math.random is render-only — never the deterministic core).
        if (ps.bufferFill < 0.97 && Math.random() < 0.14) producing.push(ps.stationId);
      } else if (d > 0.01) {
        // A SINK whose buffer JUMPED → a cargo train just delivered supply here (a discrete event).
        delivered.push({ id: ps.stationId, d });
      }
    }
    if (producing.length === 0 && delivered.length === 0) return;
    const sv = this.bridge.stationsView(); // cached topology — cheap
    const at = (id: number): { lng: number; lat: number } | null => {
      const v = sv[id];
      if (!v || v.removed) return null;
      const [lng, lat] = mmToLngLat([v.xMm, v.yMm]);
      return { lng, lat };
    };
    // a modest chimney lift so the smoke leaves the node's roof, not the ground (cheap zoom-derived px).
    const lift = Math.min(22, 64 / ((156543.03 * Math.max(0.01, Math.cos((this.map.getCenter().lat * Math.PI) / 180))) / 2 ** this.map.getZoom()));
    for (const id of producing.slice(0, 8)) {
      const p = at(id);
      if (p) this.effects.puff(p.lng, p.lat, lift); // working smoke at the forge/source
    }
    delivered.sort((a, b) => b.d - a.d);
    for (const { id } of delivered.slice(0, 5)) {
      const p = at(id);
      if (p) {
        this.effects.burst(p.lng, p.lat, "235,200,120"); // a warm "supply landed" deposit ring at the sink
        this.effects.floatText(p.lng, p.lat, "+⬢", "245,210,140", { rise: 22, size: 13, ttl: 1000 });
      }
    }
  }

  /** Supply-FLOW pips (#living): small pips march along each busy served line's polyline so the arteries
   *  visibly PUMP (denser + faster = busier) — throughput at a glance, distinct from the discrete trains.
   *  Built from the 3 Hz per-line snapshot (ridership + load); the march itself is per-frame on the FX
   *  canvas (no deck rebuild). Arcadia + zoomed-IN + running only; replaced wholesale each slice. */
  private updateSupplyFlow(s: Stats): void {
    const detail = this.ruleset === "arcadia" && this.map.getZoom() >= DETAIL_ZOOM;
    if (!detail || !s.running) {
      this.effects.setFlows([]);
      return;
    }
    const lv = this.bridge.linesView();
    const active = [...this.perLineById.values()]
      .filter((l) => l.trains > 0 && l.ridership > 0)
      .sort((a, b) => b.ridership - a.ridership)
      .slice(0, 8); // cap so the per-frame polyline projection stays bounded
    const flows: Flow[] = [];
    for (const pl of active) {
      const l = lv[pl.lineId];
      if (!l || l.removed || l.polylineMm.length < 2) continue;
      const pts = l.polylineMm.map(([x, y]) => mmToLngLat([x, y]) as [number, number]);
      const intensity = Math.min(1, pl.loadFactor + 0.2); // busier (fuller) ⇒ denser + faster flow
      flows.push({
        pts,
        rgb: colorToRgb(l.color).join(","),
        n: Math.round(3 + intensity * 6),
        periodMs: 5200 - intensity * 2800, // busier ⇒ shorter period ⇒ faster march
        alpha: 0.3 + intensity * 0.22,
        r: 2.1,
      });
    }
    this.effects.setFlows(flows);
  }

  /** Night-window flicker (#5e): a warm, TWINKLING lit-window core at each settlement + resource camp,
   *  fading in with nightFactor — towns read as INHABITED at night, sitting over the steady deck night-glow
   *  halo. Built on the 3 Hz nightFactor slice; the twinkle is per-frame on the FX canvas. Arcadia + detail. */
  private updateNightLights(): void {
    const detail = this.ruleset === "arcadia" && this.map.getZoom() >= DETAIL_ZOOM;
    if (!detail || this.nightFactor <= 0.04) {
      this.effects.setNightLights([]);
      return;
    }
    const nf = this.nightFactor;
    const lights: NightLight[] = [];
    let seed = 1;
    for (const t of this.towns) {
      const cap = t.kind === "capital";
      lights.push({ lng: t.lng, lat: t.lat, rgb: cap ? "255,226,150" : "255,210,138", r: cap ? 7 : 5, base: nf * (cap ? 0.85 : 0.6), seed: seed++ });
    }
    for (const r of this.resources) {
      lights.push({ lng: r.lng, lat: r.lat, rgb: "255,198,124", r: 4, base: nf * 0.5, seed: seed++ });
    }
    this.effects.setNightLights(lights);
  }

  /** Extraction SHIMMER (#living): faint commodity-tinted glints at the raw resource camps (the deposits
   *  being worked) — distinct from the forge PROCESSING smoke in emitWorldJuice. A few per slice, fired
   *  probabilistically (render-only Math.random, never the deterministic core). Arcadia + zoomed-in only. */
  private emitResourceShimmer(): void {
    if (this.ruleset !== "arcadia" || this.map.getZoom() < DETAIL_ZOOM || this.resources.length === 0) return;
    for (const r of this.resources) {
      if (Math.random() < 0.07) this.effects.shimmer(r.lng, r.lat, this.cargoOf(r.kind).tint.join(","));
    }
  }

  /** Living-world (#living): build the ambient trade graph from the baked nodes — the capital trades with
   *  every town, each town with its nearest neighbour town, and each town draws from its nearest resource.
   *  Carts (ping-ponging traders) are seeded per route. Called once at load (arcadia only); render-only, so
   *  the variety RNG here never touches the deterministic core. Idempotent (clears + rebuilds). */
  /** Fantasy 3D diorama (#3d-trees): scatter lowpoly pines across the forest hexes (biome FOREST=8),
   *  jittered within each hex + varied in height/yaw/tint so a stand reads natural. Capped for perf; built
   *  once at load (arcadia only). The variety RNG is render-only — never touches the deterministic core. */
  buildTrees(): void {
    this.trees = [];
    if (this.ruleset !== "arcadia" || this.terrain.length === 0) return;
    const FOREST = 8;
    const forest = this.terrain.filter((c) => c.c === FOREST);
    if (forest.length === 0) return;
    const CAP = 4200;
    const jitterMm = this.terrainCellM * 1000 * 0.46; // up to ~46% of a hex off-centre
    // Thin the forest cells to the cap, then 1–2 pines per kept cell.
    const stride = Math.max(1, Math.ceil((forest.length * 2) / CAP));
    for (let i = 0; i < forest.length && this.trees.length < CAP; i += stride) {
      const c = forest[i];
      const [mx, my] = lngLatToMm([c.lng, c.lat]);
      const k = 1 + (Math.random() < 0.6 ? 1 : 0);
      for (let j = 0; j < k && this.trees.length < CAP; j++) {
        const [lng, lat] = mmToLngLat([mx + (Math.random() * 2 - 1) * jitterMm, my + (Math.random() * 2 - 1) * jitterMm]);
        this.trees.push({ lng, lat, scale: 120 + Math.random() * 110, yaw: Math.random() * 360, shade: Math.random() });
      }
    }
  }

  /** Cargo a trade good reads as — a glyph + a tint so you can SEE what each cart hauls. Keyed by resource
   *  kind (ore/grain/fuel/aether) or a town's demand chain (bread/arms); a plain crate is the fallback. */
  private cargoOf(kind: string): { glyph: string; tint: [number, number, number] } {
    // Glyphs are drawn from the symbol-font CARGO_CHARSET (render.ts) — NOT emoji — so they render in
    // deck's TextLayer atlas. A cart's icon matches its source node's glyph (ore=⛏, grain=✿, …).
    switch (kind) {
      case "ore": return { glyph: "⛏", tint: [170, 138, 104] };
      case "grain": return { glyph: "✿", tint: [214, 184, 86] };
      case "fuel": return { glyph: "♣", tint: [120, 116, 86] };
      case "aether": return { glyph: "✦", tint: [176, 124, 224] };
      case "bread": return { glyph: "❖", tint: [206, 172, 112] };
      case "arms": return { glyph: "⚔", tint: [182, 188, 198] };
      default: return { glyph: "◆", tint: [206, 178, 132] };
    }
  }

  /** Terrain bounds (mm) for ambient pathfinding — a cart may never wander off the baked continent. Built
   *  once from the terrain hexes (with a one-cell margin). */
  private ensureTerrainBbox(): [number, number, number, number] | null {
    if (this.terrainBboxMm) return this.terrainBboxMm;
    if (this.terrain.length === 0) return null;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const c of this.terrain) {
      const [x, y] = lngLatToMm([c.lng, c.lat]);
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
    const m = this.terrainCellM * 1000 || 250_000;
    this.terrainBboxMm = [minX - m, minY - m, maxX + m, maxY + m];
    return this.terrainBboxMm;
  }

  /** A* a land route between two lng/lat nodes over the buildability grid: 8-neighbour, blocked on WATER
   *  cells and anything off the terrain bounds. Returns a decimated lng/lat polyline (start → … → end) that
   *  hugs the coast and skirts the sea, or null if no land path exists (then the route is dropped — no cart
   *  swims). Render-only; never touches the deterministic core. */
  private landPath(ax: number, ay: number, bx: number, by: number): [number, number][] | null {
    const bbox = this.ensureTerrainBbox();
    if (!this.build.loaded || !bbox) return [[ax, ay], [bx, by]]; // no terrain oracle ⇒ keep straight (still slow)
    const cm = this.build.cellMm;
    const [minX, minY, maxX, maxY] = bbox;
    // The fantasy buildability grid stores BIOME codes (4=WATER, 6=MOUNTAIN, …) — passable land is exactly
    // the bake's own rule: not water, not mountain (build_world.py `passable = biome != WATER & != MOUNTAIN`).
    const MOUNTAIN = 6;
    const passable = (cx: number, cy: number): boolean => {
      const x = (cx + 0.5) * cm, y = (cy + 0.5) * cm;
      if (x < minX || y < minY || x > maxX || y > maxY) return false;
      const cls = this.build.classifyMm(x, y);
      return cls !== BUILD.WATER && cls !== MOUNTAIN;
    };
    const [sxMm, syMm] = lngLatToMm([ax, ay]);
    const [exMm, eyMm] = lngLatToMm([bx, by]);
    const cellOf = (xMm: number, yMm: number): [number, number] => [Math.floor(xMm / cm), Math.floor(yMm / cm)];
    // Snap each endpoint to the nearest passable cell (towns/resources sit on land, but be defensive).
    const snap = ([cx, cy]: [number, number]): [number, number] | null => {
      if (passable(cx, cy)) return [cx, cy];
      for (let r = 1; r <= 3; r++)
        for (let dx = -r; dx <= r; dx++)
          for (let dy = -r; dy <= r; dy++)
            if (passable(cx + dx, cy + dy)) return [cx + dx, cy + dy];
      return null;
    };
    const start = snap(cellOf(sxMm, syMm));
    const goal = snap(cellOf(exMm, eyMm));
    if (!start || !goal) return null;
    const key = (cx: number, cy: number) => cx * 100_000 + cy;
    const gkey = key(goal[0], goal[1]);
    const h = (cx: number, cy: number) => Math.hypot(cx - goal[0], cy - goal[1]);
    const g = new Map<number, number>();
    const came = new Map<number, number>();
    // Binary min-heap of [f, cx, cy].
    const heap: [number, number, number][] = [];
    const push = (f: number, cx: number, cy: number) => {
      heap.push([f, cx, cy]);
      let i = heap.length - 1;
      while (i > 0) { const p = (i - 1) >> 1; if (heap[p][0] <= heap[i][0]) break; [heap[p], heap[i]] = [heap[i], heap[p]]; i = p; }
    };
    const pop = (): [number, number, number] => {
      const top = heap[0], last = heap.pop()!;
      if (heap.length) { heap[0] = last; let i = 0; for (;;) { const l = 2 * i + 1, r = l + 1; let s = i; if (l < heap.length && heap[l][0] < heap[s][0]) s = l; if (r < heap.length && heap[r][0] < heap[s][0]) s = r; if (s === i) break; [heap[s], heap[i]] = [heap[i], heap[s]]; i = s; } }
      return top;
    };
    g.set(key(start[0], start[1]), 0);
    push(h(start[0], start[1]), start[0], start[1]);
    const NB: [number, number][] = [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [1, -1], [-1, 1], [-1, -1]];
    let expanded = 0;
    let found = false;
    while (heap.length && expanded < 22_000) {
      const [, cx, cy] = pop();
      const ck = key(cx, cy);
      if (ck === gkey) { found = true; break; }
      expanded++;
      const gc = g.get(ck)!;
      for (const [dx, dy] of NB) {
        const nx = cx + dx, ny = cy + dy;
        if (!passable(nx, ny)) continue;
        const nk = key(nx, ny);
        const ng = gc + (dx && dy ? 1.4142 : 1);
        if (ng < (g.get(nk) ?? Infinity)) { g.set(nk, ng); came.set(nk, ck); push(ng + h(nx, ny), nx, ny); }
      }
    }
    if (!found) return null;
    // Reconstruct cell path → lng/lat, decimated (every 2nd cell) since motion lerps along arc-length.
    const cells: number[] = [];
    let cur = gkey;
    for (;;) { cells.push(cur); const prev = came.get(cur); if (prev === undefined) break; cur = prev; }
    cells.reverse();
    const pts: [number, number][] = [[ax, ay]];
    for (let i = 1; i < cells.length - 1; i += 2) {
      const cx = Math.floor(cells[i] / 100_000), cy = cells[i] - cx * 100_000;
      pts.push(mmToLngLat([(cx + 0.5) * cm, (cy + 0.5) * cm]));
    }
    pts.push([bx, by]);
    return pts;
  }

  buildAmbientTrade(): void {
    this.ambientRoutes = [];
    this.ambientCarts = [];
    if (this.ruleset !== "arcadia") return;
    const cap = this.towns.find((t) => t.kind === "capital");
    const towns = this.towns.filter((t) => t.kind !== "capital");
    const seen = new Set<string>();
    const addRoute = (a: { lng: number; lat: number }, b: { lng: number; lat: number }, kind: string) => {
      const k = [a.lng, a.lat, b.lng, b.lat].map((n) => n.toFixed(4)).sort().join(",");
      if (seen.has(k) || (a.lng === b.lng && a.lat === b.lat)) return;
      seen.add(k);
      const pts = this.landPath(a.lng, a.lat, b.lng, b.lat);
      if (!pts || pts.length < 2) return; // no land route ⇒ no cart sails the sea
      const cum = [0];
      for (let i = 1; i < pts.length; i++) cum.push(cum[i - 1] + Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]));
      const len = cum[cum.length - 1];
      if (len <= 0) return;
      const { glyph, tint } = this.cargoOf(kind);
      this.ambientRoutes.push({ pts, cum, len, served: false, glyph, tint });
    };
    const nearest = <T extends { lng: number; lat: number }>(from: { lng: number; lat: number }, pool: T[]): T | null => {
      let best: T | null = null;
      let bd = Infinity;
      for (const p of pool) {
        if (p.lng === from.lng && p.lat === from.lat) continue;
        const d = (p.lng - from.lng) ** 2 + (p.lat - from.lat) ** 2;
        if (d < bd) { bd = d; best = p; }
      }
      return best;
    };
    for (const t of towns) {
      if (cap) addRoute(cap, t, t.chain || "bread"); // the realm trades its staple with its seat
      const nt = nearest(t, towns); // inter-town trade
      if (nt) addRoute(t, nt, t.chain || "");
      const nr = nearest(t, this.resources); // gathering raw goods from the nearest source
      if (nr) addRoute(t, nr, nr.kind);
    }
    // Seed carts: a couple of traders per route, phase-spread so each route reads as a steady two-way trickle.
    for (let r = 0; r < this.ambientRoutes.length; r++) {
      const k = 2 + Math.floor(Math.random() * 2); // 2–3 carts per route — a quiet, believable trickle
      for (let c = 0; c < k; c++) this.ambientCarts.push({ route: r, off: Math.random() });
    }
    this.refreshAmbientServed();
  }

  /** A route is "industrialised" once BOTH its endpoints sit within a player station's catchment — the
   *  freight has moved onto the railway, so its carts fade. Recomputed on refresh (station set changes),
   *  never per frame. Cheap O(routes × stations). */
  private refreshAmbientServed(): void {
    if (this.ambientRoutes.length === 0) return;
    const sv = this.bridge.stationsView().filter((s) => !s.removed);
    const stMm = sv.map((s) => [s.xMm, s.yMm] as [number, number]);
    const r2 = (CATCHMENT_M * 1500) ** 2; // a touch beyond catchment — "the rail is near this node"
    const near = (lng: number, lat: number): boolean => {
      const [x, y] = lngLatToMm([lng, lat]);
      for (const [sx, sy] of stMm) if ((sx - x) ** 2 + (sy - y) ** 2 <= r2) return true;
      return false;
    };
    for (const route of this.ambientRoutes) {
      const a = route.pts[0], b = route.pts[route.pts.length - 1];
      route.served = near(a[0], a[1]) && near(b[0], b[1]);
    }
  }

  /** Ground-speed of an ox-cart in lng/lat units per ms — deliberately a CRAWL (~real ox pace), so the
   *  continent breathes slowly rather than zipping. Constant regardless of route length (arc-length motion). */
  private static readonly AMBIENT_SPEED = 0.0000016;

  /** Living-world (#living): the ambient trade carts at wall-clock `now` — each trundles its terrain-following
   *  polyline at a constant slow ground-speed, ping-ponging end to end (a brief pause at each end reads as
   *  loading). Purely decorative; rebuilt per frame like the vehicle layer. Empty unless arcadia + toggle on. */
  ambientTradersAt(now: number): AmbientTrader[] {
    if (!this.showAmbient || this.ruleset !== "arcadia") return [];
    const out: AmbientTrader[] = [];
    for (const c of this.ambientCarts) {
      const route = this.ambientRoutes[c.route];
      const len = route.len;
      const period = (2 * len) / Game.AMBIENT_SPEED; // ms for a full there-and-back
      let d = (now + c.off * period) % period * Game.AMBIENT_SPEED; // distance into the round trip
      if (d > len) d = 2 * len - d; // ping-pong fold
      // Locate arc-length d along the polyline.
      const cum = route.cum, pts = route.pts;
      let seg = 1;
      while (seg < cum.length - 1 && cum[seg] < d) seg++;
      const t = cum[seg] > cum[seg - 1] ? (d - cum[seg - 1]) / (cum[seg] - cum[seg - 1]) : 0;
      const a = pts[seg - 1], b = pts[seg];
      out.push({ lng: a[0] + (b[0] - a[0]) * t, lat: a[1] + (b[1] - a[1]) * t, dim: route.served, glyph: route.glyph, tint: route.tint });
    }
    return out;
  }

  /** Rebuild cached topology layers from authoritative sim views; recompose with current
   *  (non-interpolated) vehicle positions. The GameLoop recomposes per frame with alpha. */
  refresh(): void {
    const { below, above } = topoLayers(this.buildView());
    this.below = below;
    this.above = above;
    // TTD L5c: rebuild the selected line's placed-signal posts + place ghost on-change (not per rAF).
    this.placedSignalLayersCache = placedSignalLayers(this.placedSignals(), this.signalGhost());
    this.refreshAmbientServed(); // re-evaluate which trade routes the rail now serves (station set changed)
    this.composeAndSet(this.currentVehicleDots(), this.vehicleCarsAt(1), this.peepLayerAt(1));
    for (const cb of this.onChange) cb();
  }

  /** Set the overlay layers: stable cached topo with the vehicle layer + peep layer spliced into
   *  z-order (catchment/lines/blueprint < vehicles < peeps < stations < waiting). Reused topo
   *  instances mean deck only re-uploads the small per-frame vehicle + peep layers. */
  /** Marching-legion dots (fantasy). Read each compose like the vehicle layer; metres→lng/lat in place.
   *  Null when there are no legions (transit always; arcadia until the first launch). */
  armyLayerAt(): Layer[] {
    const xy = this.bridge.armyPositions(); // metres (kept in metres for the march-heading derivation)
    const count = xy.length >> 1;
    if (count === 0) return [];
    const tg = this.bridge.armyTargets(); // #war: target position per legion (metres), aligned with xy
    const states = this.bridge.armyStates(); // #legion-ride-trains: 0 deciding / 1 besieging / 2 done / 3 walking / 4 waiting / 5 riding
    // #legion-3d: each AFIELD legion (deciding/walking/waiting/riding/besieging) is a 3D crimson STANDARD
    // yawed to its march direction + a NAMEPLATE. A RIDING legion mirrors its train's position, so its
    // standard draws ON the train (the "riding the rails" read). A DONE legion is inert (its holding reads
    // from the captured-town state), so it's dropped to de-litter the map. Heading is the metre-space bearing
    // to the target (yawOf calibrated for the same atan2 the vehicles use); a besieging legion faces north.
    // #daynight: a WALKING legion is CAMPED while it's dark — the sim holds its march till dawn, so it
    // reads as a lit camp here (matches the sim's tod::is_daylight 06:00–20:00 gate via the float simHour).
    const h = this.lastStats?.simHour ?? 12;
    const night = !(h >= 6 && h < 20);
    const legions: LegionDot[] = [];
    for (let i = 0; i < count; i++) {
      if ((states[i] | 0) === 2) continue; // DONE / garrisoned — skip
      const px = xy[i * 2];
      const py = xy[i * 2 + 1];
      const tx = tg[i * 2] ?? px;
      const ty = tg[i * 2 + 1] ?? py;
      const heading = tx === px && ty === py ? 0 : Math.atan2(ty - py, tx - px);
      const [lng, lat] = metersToLngLat([px, py]);
      legions.push({ lng, lat, heading, name: LEGION_NAMES[i % LEGION_NAMES.length], besieging: (states[i] | 0) === 1, camped: night && (states[i] | 0) === 3 });
    }
    if (legions.length === 0) return [];
    const camped = legions.filter((l) => l.camped);
    return camped.length > 0 ? [legionCampfireLayer(camped), legionLayer(legions), legionNameLayer(legions)] : [legionLayer(legions), legionNameLayer(legions)];
  }

  /** Legion INTENT arcs (fantasy, S11 — the AI general's "why" made spatial): a faint crimson arc from
   *  each MARCHING legion to its target town, so the player reads where the AI is sending its legions (you
   *  steer by rail + bounty; the legions execute). Idle/besieging legions emit a zero-length arc (target ==
   *  own position) which we skip. Null when no legion is marching. */
  armyIntentLayerAt(): Layer | null {
    const pos = this.bridge.armyPositions();
    const tgt = this.bridge.armyTargets();
    const count = Math.min(pos.length, tgt.length) >> 1;
    if (count === 0) return null;
    const arcs: IntentArc[] = [];
    for (let i = 0; i < count; i++) {
      const px = pos[i * 2], py = pos[i * 2 + 1], tx = tgt[i * 2], ty = tgt[i * 2 + 1];
      if (px === tx && py === ty) continue; // zero-length → idle/besieging legion: no forward intent
      const [flng, flat] = metersToLngLat([px, py]);
      const [tlng, tlat] = metersToLngLat([tx, ty]);
      arcs.push({ from: [flng, flat], to: [tlng, tlat] });
    }
    // #war clutter: fade arcs as their count climbs (a cluster reads as a gradient, not a crimson smear).
    return arcs.length > 0 ? armyIntentLayer(arcs, Math.max(0.4, Math.min(1, 7 / arcs.length))) : null;
  }

  /** Rail-attack intent (#war): toxic-green arcs from each SMART raider (a saboteur heading for your rail, a
   *  reclaimer heading for an unheld town) to its target — so the rival's targeting reads on the map and you
   *  can rail-to / defend the threatened spot. Breachers (capital-bound) are filtered out: they're the
   *  obvious rot threat AND would fan a mess of lines into the capital. Null when no smart raider marches. */
  raiderIntentLayerAt(): Layer | null {
    const pos = this.bridge.raiderPositions();
    const tgt = this.bridge.raiderTargets();
    const count = Math.min(pos.length, tgt.length) >> 1;
    if (count === 0) return null;
    const cap = this.towns.find((t) => t.kind === "capital");
    const arcs: IntentArc[] = [];
    for (let i = 0; i < count; i++) {
      const [tlng, tlat] = metersToLngLat([tgt[i * 2], tgt[i * 2 + 1]]);
      // Skip a breacher (target ≈ the capital — the same position the capital town marker sits at).
      if (cap && Math.abs(tlng - cap.lng) < 1e-5 && Math.abs(tlat - cap.lat) < 1e-5) continue;
      const [flng, flat] = metersToLngLat([pos[i * 2], pos[i * 2 + 1]]);
      arcs.push({ from: [flng, flat], to: [tlng, tlat] });
    }
    // #war clutter: fade arcs as their count climbs (a cluster reads as a gradient, not a green smear).
    return arcs.length > 0 ? raiderIntentLayer(arcs, Math.max(0.4, Math.min(1, 7 / arcs.length))) : null;
  }

  /** Decadence-raider dots (fantasy, S11 — the rival). Same metres→lng/lat-in-place path as legions.
   *  Null when no raiders march (transit always; arcadia until the rival fields one). */
  raiderLayerAt(): Layer[] {
    const xy = this.bridge.raiderPositions();
    const count = xy.length >> 1;
    if (count === 0) return [];
    const roles = this.bridge.raiderRoles(); // #war: 0 breacher / 1 saboteur / 2 reclaimer (aligned with xy)
    for (let i = 0; i < xy.length; i += 2) metersToLngLatInto(xy[i], xy[i + 1], xy, i);
    // Per-ROLE badge so the three rival roles read APART (otherwise identical green dots): ☣ breacher (marching
    // your seat) · ✂ saboteur (cutting your rail) · ⚑ reclaimer (re-taking your unheld towns). #war legibility.
    const RGLYPH = ["☣", "✂", "⚑"];
    const byRole: [number, number][][] = [[], [], []];
    for (let i = 0; i < count; i++) (byRole[roles[i] | 0] ?? byRole[0]).push([xy[i * 2], xy[i * 2 + 1]]);
    const layers: Layer[] = [raiderLayer(xy, count)];
    byRole.forEach((pos, r) => {
      if (pos.length) layers.push(entityBadgeLayer(`raider-badges-${r}`, pos, RGLYPH[r], [40, 50, 30, 235]));
    });
    return layers;
  }

  /** Spell-flash bursts (fantasy, S11 — the spell arm). `[x_m,y_m,kind,alpha,...]` → lng/lat objects.
   *  Null when nothing's casting (transit always; arcadia until SPELLCRAFT + a cast). */
  spellFlashLayerAt(): Layer | null {
    const f = this.bridge.spellFlashes();
    if (f.length === 0) return null;
    const flashes: { lng: number; lat: number; kind: number; alpha: number }[] = [];
    for (let i = 0; i + 3 < f.length; i += 4) {
      const [lng, lat] = metersToLngLat([f[i], f[i + 1]]);
      flashes.push({ lng, lat, kind: f[i + 2], alpha: f[i + 3] });
    }
    return spellFlashLayer(flashes);
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

  /** Tide-frontier ring alpha — a slow triangle pulse (150..230) advanced ONE step per buildView (the
   *  ~3 Hz recompose), so the decadence front visibly breathes without any per-frame work (two-clocks).
   *  Integer-quantized so deck's updateTriggers only fire on a real change. */
  private tidePhase = 0;
  private nextTidePulse(): number {
    this.tidePhase = (this.tidePhase + 1) % 12;
    return 150 + Math.round((80 * Math.abs(6 - this.tidePhase)) / 6); // triangle 150..230
  }

  composeAndSet(vehicles: VehicleDot[], cars: CargoCar[], peeps: Layer | null): void {
    const peep = peeps ? [peeps] : [];
    const intent = this.armyIntentLayerAt();
    const intentArcs = intent ? [intent] : []; // legion→target intent, under the legion dots (over the network)
    const rIntent = this.raiderIntentLayerAt();
    const raiderIntentArcs = rIntent ? [rIntent] : []; // #war: smart-raider→target intent (your rail / unheld towns)
    const army = this.armyLayerAt(); // [dot, ⚔ badge] — legions above carts, below peeps/labels (z-order)
    const raider = this.raiderLayerAt(); // [dot, ☣ badge] — the rival's marauders, above legions
    const flash = this.spellFlashLayerAt();
    const spells = flash ? [flash] : []; // spell bursts on top (the magic reads over everything)
    // Level-of-detail (runs per frame on the live zoom): below DETAIL_ZOOM the city-overview shows
    // only the network — drop the per-station waiting halos, the pinned label, and the vehicle
    // direction arrows (micro-detail that turns to a flashing swarm at overview). Peeps are gated
    // separately in peepLayerAt. Cheap: a filter over ~17 already-built layers, no rebuild.
    const detail = this.map.getZoom() >= DETAIL_ZOOM;
    // At the strategic overview, drop the per-car CARGO detail — the trailing wagons + their load lumps and
    // the bus/ferry cargo block (all micro-detail that's sub-pixel when zoomed out). The 3D locomotive bodies
    // stay so you still read the live network in motion (a train collapses to its loco at overview).
    const CARGO_LOD = new Set(["vehicle-cargo", "vehicle-wagons", "vehicle-wagon-cargo"]);
    // Cabin scale derived from the map's hex cell (≈4 cabins per cell-step, the sim's car-pitch matches) so
    // trains stay proportionate to the lattice + L2's platform-length constraint; a real-OSM map (no hex,
    // terrainCellM 0) keeps the diorama default. cell_step = √3 · circumradius (terrainCellM).
    const vehScale = this.terrainCellM > 0 ? (this.terrainCellM * Math.sqrt(3)) / 4 : 150;
    const vlayers = detail ? vehicleLayers(vehicles, cars, vehScale) : vehicleLayers(vehicles, cars, vehScale).filter((l) => !CARGO_LOD.has(l.id as string));
    // Exactly one waiting layer shows per frame: the full per-station halos when zoomed in, the
    // starved-only subset at overview (a starved platform must be findable at ANY zoom).
    const above = detail
      ? this.above.filter((l) => l.id !== "waiting-overview")
      : this.above.filter((l) => l.id !== "waiting" && l.id !== "station-label" && l.id !== "node-plates");
    // Arcadia LOD: at overview, drop the dense resource-POI swarm (~30 dots) + trees so the continent reads
    // as terrain + towns + the strategic picture; they return on zoom-in. The tide-front EDGE now STAYS at
    // overview (#war legibility) — it's the single best "where will the rot hit next" telegraph, a strategic
    // intent channel like the army/raider intent arcs (which are LOD-exempt), not detail clutter. (Town fills
    // + the tide WASH always stay too.) Same cheap id-filter, no rebuild.
    const below =
      this.ruleset === "arcadia" && !detail
        ? this.below.filter((l) => l.id !== "resources" && l.id !== "resource-icons" && l.id !== "trees" && l.id !== "station-depots")
        : this.below;
    // Living-world ambient trade carts (#living): ground texture under the player's network + vehicles.
    // Wall-clock animated, rebuilt per frame like the vehicle layer (small, cheap). Arcadia only, and only
    // when zoomed IN (LOD): at the strategic overview they'd swarm + compete with the player's trains, so
    // drop them below DETAIL_ZOOM (the same gate peeps use) — the continent reads as terrain + network there.
    const ambient =
      this.ruleset === "arcadia" && this.showAmbient && detail && this.ambientCarts.length > 0
        ? (() => {
            const carts = this.ambientTradersAt(performance.now());
            return [ambientTraderLayer(carts), ambientCargoLayer(carts)];
          })()
        : [];
    // TTD signals (opt-in lens): single-track block state UNDER the vehicles, so a cart rides on top of
    // the signal that gates it. Per-frame (occupancy shifts with the trains), like the other motion layers.
    const signals = this.showSignals ? [signalLayer(this.signalMarkers())] : [];
    // TTD L5c: the selected line's PLAYER-PLACED block-signal posts (+ place ghost), above the network /
    // below the vehicles — a cached on-change layer set (see `placedSignalLayersCache`), distinct glyph from
    // the occupancy aspect dots above. Only populated in build mode with a line selected.
    const placedSig = this.placedSignalLayersCache;
    // Legion NAMEPLATES drop at the strategic overview (label clutter); the 3D hosts stay so the force reads.
    const armyL = detail ? army : army.filter((l) => l.id !== "legion-names");
    // Night LIGHTS (#5e): warm glows at the capital + towns + resource camps that fade in as night
    // falls (game.nightFactor, set on the 3 Hz sim-hour slice). Above the terrain/towns, below the
    // vehicles. Arcadia + zoomed-in only; an empty array by day (nightFactor≈0) → zero cost.
    const nightGlow = this.ruleset === "arcadia" && detail ? nightGlowLayers(this.towns, this.resources, this.nightFactor) : [];
    // Train headlamps: a warm glow under each running train at night, beneath the loco mesh.
    const vehGlow = this.ruleset === "arcadia" && detail ? vehicleNightGlow(vehicles, this.nightFactor) : [];
    let layers = [...below, ...nightGlow, ...ambient, ...signals, ...placedSig, ...vehGlow, ...vlayers, ...intentArcs, ...raiderIntentArcs, ...armyL, ...raider, ...spells, ...peep, ...above];
    // Map LENS (#5): emphasise one reading of the busy arcadia map by HIDING the layers that belong to the
    // other readings (the terrain + the player's network/vehicles always stay). Cheap id-filter, no rebuild.
    if (this.ruleset === "arcadia" && this.lens !== "realm") {
      const hide = LENS_HIDE[this.lens];
      layers = layers.filter((l) => !hide.has(l.id as string));
    }
    this.overlay.setProps({ layers });
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
    const cargo = this.bridge.vehicleCargo(); // dominant commodity per vehicle (255 = empty/transit)
    const lines = this.bridge.linesView();
    const colors = lines.map((l) => l.color);
    // A rail/heavy line pulls cargo WAGONS (#multi-car) → its loco carries no on-body block (the load
    // shows on the trailing cars). tmode RAIL=0, HEAVY=4 (trainset.rs). Bus/ferry/air pull nothing.
    const pulls = lines.map((l) => l.mode === 0 || l.mode === 4);
    const dots: VehicleDot[] = [];
    for (let i = 0; i < cur.length; i += 2) {
      const vi = i / 2;
      const x = prev[i] + (cur[i] - prev[i]) * alpha;
      const y = prev[i + 1] + (cur[i + 1] - prev[i + 1]) * alpha;
      const [lng, lat] = metersToLngLat([x, y]);
      const cap = loads[vi * 2 + 1] ?? 0;
      const load = cap > 0 ? (loads[vi * 2] ?? 0) / cap : 0;
      const li = lineIds[vi];
      dots.push({ lng, lat, color: colorToRgb(colors[li] ?? 0x444444), angle: angles[vi] ?? 0, load, cargo: cargo[vi] ?? 255, pullsCars: pulls[li] ?? false });
    }
    return dots;
  }

  /** Trailing cargo WAGONS interpolated at `alpha` (#multi-car) — the string of cars each rail train pulls,
   *  one entry per car across all trains, curving behind its loco along the track. Built from the sim's
   *  `vehicleCars` (6 f32/car) + `vehicleCarsPrev` (2 f32/car) buffers; the line colour tints the chassis,
   *  the commodity colours the load lump. Empty when nothing rail is running. */
  vehicleCarsAt(alpha: number): CargoCar[] {
    const cur = this.bridge.vehicleCars(); // 6 per car: [x, y, angle, commodity, load, lineId]
    if (cur.length === 0) return [];
    const prev = this.bridge.vehicleCarsPrev(); // 2 per car: [x, y]
    const colors = this.lineColors();
    const cars: CargoCar[] = [];
    for (let i = 0, p = 0; i < cur.length; i += 6, p += 2) {
      const x = prev[p] + (cur[i] - prev[p]) * alpha;
      const y = prev[p + 1] + (cur[i + 1] - prev[p + 1]) * alpha;
      const [lng, lat] = metersToLngLat([x, y]);
      cars.push({ lng, lat, angle: cur[i + 2], cargo: cur[i + 3], load: cur[i + 4], color: colorToRgb(colors[cur[i + 5] | 0] ?? 0x444444) });
    }
    return cars;
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

  /** #13 P1c: seed the rival realm's seat (a faction-1 far-edge capital). Fired ONCE at boot AFTER the
   *  baked supply graph, so the core picks a reservoir cell clear of the player's towns. Through the command
   *  path (joins the log ⇒ resumes/replays correctly); idempotent in the core. */
  seedRival(): void {
    this.bridge.apply(cmd.seedRival());
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

  /** The next palette colour for a new line (deterministic by line index). Arcadia draws from the all-warm
   *  palette so the empire is the only warmth (the colour is logged in the CreateLine command, so this only
   *  picks the hue for NEW lines — golden-neutral). */
  nextLineColor(): number {
    const n = this.bridge.linesView().length;
    const pal = this.ruleset === "arcadia" ? ARCADIA_LINE_PALETTE : LINE_PALETTE;
    return pal[n % pal.length];
  }
}
