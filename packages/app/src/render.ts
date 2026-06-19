// Builds the deck.gl overlay layers from a plain RenderView (already in lng/lat — all mm
// conversion happened at the geo.ts boundary in Game). Layer array order IS the z-order
// (AGENTS IA): catchment < lines < blueprint < stations < vehicles < selection highlight.
import type { Layer } from "@deck.gl/core";
import { ArcLayer, ColumnLayer, PathLayer, ScatterplotLayer, TextLayer } from "@deck.gl/layers";
import { SimpleMeshLayer } from "@deck.gl/mesh-layers";
import { BUSY_WAITING, STARVED_WAITING } from "./config";
import { pineGeometry } from "./render/treeMesh";
import { stationMesh } from "./render/stationMesh";
import { vehicleMesh, cargoMesh, wagonMesh, legionMesh } from "./render/vehicleMesh";
import { SIM_MS_PER_CLOCK_MIN } from "./ui/react/shared";

export type Rgb = [number, number, number];

export interface StationDot {
  id: number;
  lng: number;
  lat: number;
  name: string;
  selected: boolean;
  /** Cumulative boardings — scales the dot radius so busy stations visibly grow (throughput map). */
  boardings: number;
  /** Operational lines serving this station; 0 = orphaned → muted fill until it gets service. */
  serving: number;
  /** Posted bounty (fantasy) — >0 draws a ⚑ marker so the player sees where they've baited legions. */
  bounty?: number;
  /** #13 faction — 0 = player, 1 = rival realm (the enemy AI's nodes render crimson). */
  faction?: number;
  /** #15 Phase 4 — terrain relief (m) of the station's hex, so the 3D depot sits ON the extruded top. */
  z?: number;
}
export interface LinePath {
  id: number;
  color: Rgb;
  path: [number, number][];
  mode: number; // transport mode (trainset::tmode); 4 = heavy/high-speed rail (distinct styling)
  raided?: boolean; // #war: this line is CUT (a raider severed it) — its trains are frozen; renders red
  // TTD L6 (track + services): a line with NO assigned stock is bare TRACK — drawn as muted grey
  // infrastructure, not a coloured service. It lights up to its hue the moment it gets a trainset.
  // Defaults to serviced (true) so non-L6 call sites are unchanged.
  serviced?: boolean;
  /** #13 faction — 1 = the rival realm's rail (drawn hot-crimson + bolder so the enemy advance reads as a
   *  threat). Set in the LinePath build by RIVAL_LINE_COLOR match; absent/0 = the player's own line. */
  faction?: number;
}
/** Rail-attack (#war): a "⚔ RAIDED" badge at a severed line's midpoint, with the live recovery countdown. */
export interface RaidLabel {
  lng: number;
  lat: number;
  text: string;
}
/** Siege progress (#war): a town being ground down by a besieging legion. `progress` 0 (just engaged) → 1
 *  (about to fall) — the red pressure builds as capture nears, so a sieging legion reads as "winning". */
export interface SiegeRing {
  lng: number;
  lat: number;
  progress: number;
}
/** Barracks marker (#war): the ⚔ legion-spawn node. `ready` = the realm has manpower to field a legion. */
export interface BarracksBadge {
  lng: number;
  lat: number;
  ready: boolean;
}
export interface CatchmentCircle {
  lng: number;
  lat: number;
  radiusM: number;
  /** true = transient hover peek (stroke-only, fainter); false/undefined = pinned (filled). */
  peek?: boolean;
  /** Captured-demand strength 0..1 — scales the pinned fill alpha so a station sitting on a lot
   *  of demand reads denser than one on empty land (which stations actually grab riders). */
  demand?: number;
}
export interface ShedHex {
  lng: number;
  lat: number;
  /** Distance-decay weight 0..1 from the station → fill alpha (the shed fades toward its edge). */
  intensity: number;
}
export interface VehicleDot {
  lng: number;
  lat: number;
  color: Rgb;
  /** Heading in radians (0 = +x / east), from the sim's vehicleAngles buffer — drives the
   *  directional triangle so you read which way each train is travelling. */
  angle: number;
  /** Load factor (onboard / capacity), 0..~1 — drives the in-world cargo-stack height. */
  load: number;
  /** Dominant cargo commodity (#in-world-cargo): 0 ore / 1 grain / 2 aether / 3 fuel / 4-7 processed;
   *  255 = empty / a transit rider. Colours the 3D cargo block by the goods it hauls. */
  cargo?: number;
  /** True for a rail/heavy train (it pulls a string of cargo WAGONS, #multi-car), so the LOAD shows on
   *  the trailing cars — the locomotive itself carries no stack. Bus/ferry/air are false: a single body,
   *  so the cargo block rides on the vehicle itself (they pull nothing). */
  pullsCars?: boolean;
}

/** One trailing cargo WAGON pulled by a rail train (#multi-car) — a flatcar curving behind the locomotive
 *  along the track. Built per frame from the sim's `vehicleCars` buffer (interpolated cur↔prev), one per
 *  car across all trains. The chassis takes the line `color`; the load lump takes the commodity colour. */
export interface CargoCar {
  lng: number;
  lat: number;
  /** Heading in radians (the path tangent at this car's arc-length) — yaws the wagon so it curves. */
  angle: number;
  /** Dominant commodity it hauls (0-7; 255 empty) — the load lump's colour. */
  cargo: number;
  /** Load factor 0..~1 — the load lump's HEIGHT (a tall lump = a full wagon). */
  load: number;
  /** Line colour (chassis tint) — binds the consist visually to its line, like the PathLayer. */
  color: Rgb;
}

/** #25 Single-source COMMODITY colour (Okabe-Ito-anchored, CB-safe), id-keyed so the resource node dot
 *  (resourceColor), the 3D cart block (cargoColor), and the wagon load shimmer (game.cargoOf) all paint a
 *  commodity the SAME hue — "cyan = ore" can finally be learned (they used to diverge across three tables, so a
 *  cart hauling ore rendered a different colour than the node it left). Tones with NO node twin (processed
 *  goods, forge, bread, arms, passengers) stay local to their own table. */
export const CARGO_COLOR: Rgb[] = [
  [38, 150, 168], // 0 ore — steel-cyan iron (kept OFF the load-bearing selection-blue #0072b2)
  [230, 159, 0], //  1 grain — wheat gold
  [148, 96, 210], // 2 aether — arcane violet (the ley chroma)
  [0, 158, 115], //  3 fuel — forest green
];
/** Commodity-kind STRING → CARGO_COLOR id (resource POIs speak strings; a cart's hauled cargo speaks ids). */
export const CARGO_KIND_ID: Record<string, number> = { ore: 0, grain: 1, aether: 2, fuel: 3 };

/** The 3D cargo-block colour for a vehicle's hauled commodity (#in-world-cargo): ore/grain/aether/fuel share the
 *  single CARGO_COLOR identity (= their source node); processed goods + the empty/transit-rider tone stay local. */
function cargoColor(cargo: number | undefined): Rgb {
  if (cargo !== undefined && cargo >= 0 && cargo < CARGO_COLOR.length) return CARGO_COLOR[cargo];
  switch (cargo) {
    case 4: case 5: case 6: case 7: return [176, 180, 188]; // processed (ingot/arms/…) — pale steel
    default: return [222, 210, 188]; // empty / transit riders — pale sacks/passengers
  }
}

/** Fantasy 3D diorama (#3d-trees): one lowpoly pine instanced on a forest hex. Position is lng/lat
 *  (the mesh stands up in Z); `scale` jitters the height, `yaw` the facing, `shade` the green tint, so a
 *  forest reads as a varied stand rather than a clone army. Render-only (a SimpleMeshLayer), baked once. */
export interface TreeInstance {
  lng: number;
  lat: number;
  scale: number; // height in map-metres (sizeScale ×1 mesh unit)
  yaw: number; // facing, degrees
  shade: number; // 0..1 → green tint lerp (darker valley pines → lighter highland)
  z?: number; // #15 Phase 4 — terrain relief (m) of the underlying hex, so the mesh sits ON the extruded top
}

/** Living-world (#living): one ambient ox-cart / trader trundling a baked trade route between towns,
 *  resources, and the capital — purely DECORATIVE (client-side, wall-clock animated, never sim state),
 *  the texture that makes the continent feel inhabited. `dim` carts ride a route your rail now serves
 *  (the trade "industrialised" onto the railway), so they fade as your network takes over the haulage. */
export interface AmbientTrader {
  lng: number;
  lat: number;
  dim: boolean; // route now served by rail → faded (the railway took the freight)
  glyph: string; // the cargo it hauls — a CARGO_CHARSET symbol (⛏ ore, ✿ grain, ✦ aether, …)
  tint: [number, number, number]; // cargo colour (so the good reads even without the glyph)
}
export interface WaitingDot {
  lng: number;
  lat: number;
  count: number;
}
/** A TTD-style SIGNAL marker on a single-track block: `aspect` 0 = clear (green), 1 = occupied (red),
 *  2 = a cart held here waiting for the block ahead (amber). Render-only; surfaces the invisible meet. */
export interface SignalMarker {
  lng: number;
  lat: number;
  aspect: number;
}
/** A PLAYER-PLACED block signal (TTD L5c) — the post the player dropped on a single-track span, drawn
 *  DISTINCT from the live occupancy aspect dots (`SignalMarker`): a small upright marker the player can
 *  click to remove. `addr` is its `(line, path, span, atMm)` address (for the testid + remove command);
 *  `snap` flags the pre-commit highlight (the post the next click would remove). */
export interface PlacedSignalMarker {
  lng: number;
  lat: number;
  line: number;
  path: number;
  span: number;
  atMm: number;
  snap?: boolean;
}
/** Pre-commit PLACE candidate (TTD L5c): the spot on a single-track span the next click would drop a new
 *  signal at — a translucent ghost post drawn before the click commits (AGENTS sub-100 ms feedback). */
export interface SignalGhost {
  lng: number;
  lat: number;
}
/** A node BUFFER-FILL pip (fantasy #8): `fill` 0..1 of the node's Forge-Line buffer — a backed-up source
 *  (ship it) reads high, a starved sink reads low. Rendered as a small gauge dot on the node. */
export interface BufferPip {
  lng: number;
  lat: number;
  fill: number;
}
/** Fantasy (arcadia) #infrastructure: one RAIL-FRONTIER node — a station the player may extend rail FROM
 *  (it's rail-reachable from the capital). The set is the realm's connected network: before any line it is
 *  just the capital; it spreads as you build + conquer. A gold node-halo, not a disc — connectivity, not a
 *  radius. `root` = a holding (capital / captured town): a fresh line may always seed there. */
export interface FrontierNode {
  lng: number;
  lat: number;
  root: boolean;
}
export interface HazardDot {
  lng: number;
  lat: number;
  color: Rgb; // amber = built/park, red = water
}
export interface DemandPoint {
  lng: number;
  lat: number;
  weight: number; // travel demand at this grid cell (origin+dest)
  served?: boolean; // within the catchment union of placed stations → faded; else unmet → glows
}
export interface DesireArc {
  from: [number, number]; // selected origin station (lng/lat)
  to: [number, number]; // a destination it draws riders toward
  weight: number; // gravity pull, 0..1 normalized vs the strongest link → width + dest opacity
}
export interface ReachDot {
  lng: number;
  lat: number;
  ms: number; // transit travel time from the selected station → green/amber/red ring
}
export interface RoadCell {
  lng: number;
  lat: number; // a ROAD-class cell centre — where buses run cheap + fast (the "Roads" overlay)
  density: number; // BUILT cells in its 3×3 (0..9) — drives live congestion colour with the hour
}

/** One baked fantasy terrain hex (docs/fantasy-map.md). `c` is the biome class code (4=WATER,
 *  6=MOUNTAIN, 7=HILL, 8=FOREST, 9=LEY, 10=PLAIN). Rendered as the map itself — the buildability
 *  raster IS the terrain. Empty for transit cities (no terrain layer drawn). */
export interface TerrainCell {
  lng: number;
  lat: number;
  c: number;
}

/** One decadence-tide cell (fantasy S10c) — a corrupted CA hex + its 0..1 strength `v` (the cold creep
 *  advancing toward the capital). Rendered as a low-chroma cold overlay (strength = alpha) over the
 *  terrain, under the network. Empty for transit / before the tide starts. */
export interface TideCell {
  lng: number;
  lat: number;
  v: number;
}

/** One baked fantasy resource node (docs/fantasy-map.md S2) — a supply-chain source that terrain-gates
 *  the two chains (BREAD: grain+fuel; ARMS: ore+aether). Rendered as a coloured POI dot over the grey
 *  terrain. Empty for transit cities. */
export interface ResourceMarker {
  lng: number;
  lat: number;
  kind: string; // "ore" | "grain" | "fuel" | "aether"
  yield: number;
}

/** One baked fantasy town (docs/fantasy-map.md S3/S4) — a supply SINK + conquest target. `value` is the
 *  i64 conquest reward; `decadence` is the S4 corruption floor (0 = clean). Empty for transit. */
export interface TownMarker {
  lng: number;
  lat: number;
  kind: string; // "capital" | "starter" | "neutral"
  value: number;
  decadence: number;
  chain: string; // S7e: "bread" (needs grain+fuel) | "arms" (needs ore+aether) | "" (capital/none)
}

/** A persistent NAMEPLATE above a fantasy node (town/resource): `title` = its name, `sub` = its key stats
 *  (a town's tribute + needs; a resource's kind + yield). Shown LOD-gated (zoomed in only). */
export interface NodePlate {
  lng: number;
  lat: number;
  title: string;
  sub: string;
}

/** One decadence reservoir anchor (S4) — the far-edge tide origin / raider spawn. */
export interface DecadenceAnchor {
  lng: number;
  lat: number;
}

/** One baked RIVER segment (build_world.py flow-accumulation drainage): a cell-centre→cell-centre polyline
 *  with a width class (1..4 ∝ √flux) + a ford flag (a cheap headwater crossing). Render-only believability +
 *  the cold-water-vs-warm-empire read; the rail-cost coupling is a separate balance-gated follow-up. */
export interface RiverSeg {
  from: [number, number];
  to: [number, number];
  wclass: number;
  ford: boolean;
}

/** Town colour (docs/fantasy-map.md "The look"): the CAPITAL + dominion is the only WARMTH on a dead
 *  world (gold); neutral "good" towns read sickly COLD-bright, darkening + cooling as their decadence
 *  floor rises (the corruption showing through). */
function townColor(kind: string, decadence: number): [number, number, number, number] {
  if (kind === "capital") return [235, 175, 45, 255]; // gold — the seat of warmth
  if (kind === "starter") return [225, 135, 55, 255]; // warm amber-orange — your first hold (warmer than capital gold)
  // Neutral "good" towns: sickly cold-bright cyan-green (benevolent-but-unwell) → dark cold as the
  // decadence floor rises. Off the blue family + warm lines (no collision with selection-blue/the empire).
  // Normalised over a ~5000-unit frontier ceiling (clean → fully decayed) so corruption darkens legibly.
  const t = Math.max(0, Math.min(1, decadence / 5000)); // 0 clean … 1 deep frontier
  const r = Math.round(168 - 90 * t);
  const g = Math.round(200 - 92 * t);
  const b = Math.round(196 - 84 * t);
  return [r, g, b, 255];
}
/** Town RING colour = its supply CHAIN (S7e), so the player reads which good a town demands at a glance and
 *  knows which sources to connect: BREAD towns (grain+fuel) ring wheat-gold, ARMS towns (ore+aether) ring
 *  arcane-violet, the capital/none a neutral dark frame. */
function townRingColor(chain: string): [number, number, number, number] {
  if (chain === "bread") return [205, 165, 55, 235]; // deeper wheat — needs grain + fuel (distinct from capital-gold fill)
  if (chain === "arms") return [150, 100, 215, 235]; // arcane-violet — needs ore + aether
  return [30, 30, 36, 230]; // capital / none
}
/** Town radius (px): capital largest, neutrals scale with conquest value. Sized LARGER than the station
 *  dot (≤8px) so the kind-colour reads as a HALO around the placed station's dark dot, not hidden under it. */
function townRadius(kind: string, value: number): number {
  if (kind === "capital") return 15;
  if (kind === "starter") return 12;
  return Math.max(11, Math.min(14, 11 + value / 2500));
}

/** Resource POI palette — sparse, iconic gameplay markers (Okabe-Ito CB-safe), distinct from line hue:
 *  ore = iron blue, grain = wheat gold, fuel = forest green, AETHER = violet (the arcane, the ley chroma).
 *  Drawn with a white stroke so they pop on the muted grey continent. */
function resourceColor(kind: string): [number, number, number] {
  const id = CARGO_KIND_ID[kind]; // #25 ore/grain/fuel/aether → the shared commodity palette (node = cart = shimmer)
  if (id !== undefined) return CARGO_COLOR[id];
  switch (kind) {
    case "forge": return [120, 124, 130]; // steel grey — a PROCESSOR (ore → INGOT), S7e multi-stage
    default: return [200, 200, 200];
  }
}

/** Distinctive node ICONS (trial feedback #4/#13): a glyph per resource / town kind so the player reads
 *  WHAT each node is at a glance, not just its colour. BMP symbols (wide font coverage); the explicit
 *  NODE_CHARSET below seeds deck's TextLayer atlas so they render. Drawn OVER the coloured dots, which stay
 *  as the colour-blind-safe channel. */
export function resourceGlyph(kind: string): string {
  switch (kind) {
    case "ore": return "⛏";
    case "grain": return "✿";
    case "fuel": return "♣";
    case "aether": return "✦";
    case "forge": return "⚒";
    default: return "◆";
  }
}
export function townGlyph(kind: string): string {
  if (kind === "capital") return "★"; // the citadel — the seat
  if (kind === "starter") return "✪"; // your first hold
  return "⌂"; // a neutral town
}
/** Every glyph the node-icon TextLayers can emit (+ the player-barracks ⚔) — deck builds its font atlas
 *  from this explicit set so non-ASCII symbols actually render. */
const NODE_CHARSET = "⛏✿♣✦⚒◆★✪⌂⚔";

/** Fantasy terrain palette — value-not-color (docs/fantasy-map.md "The look"): a muted ash-grey
 *  ramp where elevation reads as VALUE (plains pale → mountains near-black), forest a touch cooler,
 *  WATER a desaturated blue-grey, and LEY a faint violet — the ONLY ground chroma (the aether prize).
 *  Hue is reserved for the player's network; the dead world stays grey so figure-ground holds. */
function terrainColor(c: number): [number, number, number, number] {
  switch (c) {
    case 4: return [40, 88, 120, 255];    // WATER — coastal shelf, lighter than the deep-ocean void (shallow shore)
    case 10: return [132, 132, 128, 255]; // PLAIN — pale ash (buildable lowland)
    // #25 widen the value ramp so elevation reads as VALUE: FOREST clearly above HILL (were ~4 units apart),
    // HILL distinctly darker than forest, MOUNTAIN lifted off near-black so its lit/shadowed slopes separate.
    case 8: return [100, 108, 100, 255];  // FOREST — cool swell, a clear step above hill (fuel country)
    case 7: return [86, 84, 82, 255];     // HILL — mid-dark grey (rising ground), darker than forest
    case 6: return [56, 54, 58, 255];     // MOUNTAIN — dark ridge, off near-black so sun-raked faces read
    case 9: return [120, 96, 156, 255];   // LEY — faint violet (the arcane: the ONLY ground chroma)
    default: return [70, 70, 70, 255];
  }
}

/** #15 Phase 4 — terrain RELIEF height (metres) by biome code: the VISIBLE companion to the gameplay height
 *  band (crates/sim height_band). Mountains stand tall, hills rise, forest is a gentle swell; PLAIN + WATER
 *  + every transit class sit at 0. Kept subtle (≪ the ~250 m hex) so the iso-tilted diorama reads as rolling
 *  country while the flat network (lines/vehicles at z=0, on the low ground it occupies) doesn't float badly. */
export function reliefM(c: number): number {
  switch (c) {
    case 6: return 130; // MOUNTAIN — the high ridge
    case 7: return 64; // HILL — rising ground
    case 9: return 50; // LEY — raised arcane ground
    case 8: return 34; // FOREST — a gentle swell
    default: return 0; // PLAIN / WATER / open — sea level
  }
}

/** STRUCTURAL congestion % (50 jammed … 100 flowing) at a given in-game hour + local built-up
 *  density — mirrors the time + density terms of sim `tod::congestion_at` (keep in sync). The third
 *  term, self-induced bus traffic, is dynamic and read off the buses slowing, not this static
 *  overlay — so the overlay reads as "how congestible this corridor is", a planning aid. */
function congestionPct(hour: number, density: number): number {
  const h = ((hour % 24) + 24) % 24;
  const timePen =
    h === 7 || h === 8 || h === 9 || h === 17 || h === 18 || h === 19
      ? 35
      : h >= 10 && h <= 16
        ? 15
        : h >= 20 && h <= 22
          ? 8
          : 0;
  return Math.max(50, Math.min(100, 100 - timePen - Math.min(9, density) * 4));
}

/** Road congestion colour: green (flowing) → amber → red (gridlocked). */
function roadColor(hour: number, density: number): [number, number, number, number] {
  const c = congestionPct(hour, density); // 50 (jammed) … 100 (flowing)
  if (c >= 82) return [70, 160, 95, 85]; // flowing — green
  if (c >= 62) return [225, 165, 45, 105]; // slowing — amber
  return [212, 70, 60, 125]; // jammed — red
}
export interface ControlHandle {
  lng: number;
  lat: number;
  kind: "waypoint" | "add"; // a draggable existing control point, or a "+" midpoint that adds one
}

export interface RenderView {
  stations: StationDot[];
  lines: LinePath[];
  catchments: CatchmentCircle[];
  shed: ShedHex[]; // lopsided walk-shed of the highlighted station (hexagons over reachable cells)
  blueprint: [number, number][]; // in-progress line being drawn (T11)
  vehicles: VehicleDot[]; // moving trains (T15)
  waiting: WaitingDot[]; // accumulating waiting-passenger halos (T17)
  bufferPips: BufferPip[]; // fantasy #8: node Forge-Line buffer-fill gauges (empty for transit)
  frontier: FrontierNode[]; // fantasy #infrastructure: rail-frontier node-halos (where rail may extend) — empty for transit
  raidLabels: RaidLabel[]; // fantasy #war: "⚔ RAIDED" badge + countdown on cut lines — empty unless a raider severed one
  siegeRings: SiegeRing[]; // fantasy #war: siege-progress rings on towns being ground down — empty unless a siege is live
  barracksBadges: BarracksBadge[]; // fantasy #war: ⚔ markers on the legion-spawn nodes — empty for transit
  hazards: HazardDot[]; // live built/water conflict dots along the blueprint (G2)
  demand: DemandPoint[]; // travel-demand heat overlay (toggleable map layer)
  roads: RoadCell[]; // ROAD-class corridors (where buses are cheap+fast) — toggle/auto in bus mode
  roadHour: number; // current in-game hour → recolours the roads overlay by live congestion
  demandCellM: number; // demand-grid cell pitch (m) → sizes the demand hexagons to tile the grid
  roadCellM: number; // buildability cell pitch (m) → sizes the road hexagons to tile the grid
  terrain: TerrainCell[]; // baked fantasy terrain hexes (the map itself) — empty for transit cities
  coast: TerrainCell[]; // #ocean: WATER hexes that touch land → the shore-foam edge (empty for transit)
  terrainCellM: number; // fantasy hex size (m, = gridCellMm/1000) → the hexagon circumradius
  trees: TreeInstance[]; // fantasy 3D diorama: lowpoly pines on forest hexes (empty for transit / at overview)
  townSprawl: TreeInstance[]; // #23 TG1: ring-cell buildings around towns (multi-cell settlements; empty for transit / at overview)
  tideCells: TideCell[]; // fantasy S10c: corrupted decadence-CA hexes (the cold creep) — empty for transit
  tidePulse?: number; // fantasy: tide-frontier ring alpha (150..230), advanced on the ~3 Hz recompose (NOT per frame)
  foamPhase?: number; // #ocean: shore-foam lap phase (0..23), advanced on the ~3 Hz recompose (NOT per frame)
  arcadia?: boolean; // fantasy ruleset → cold-violet demand overlay + arcadia LOD (warmth stays the empire's)
  resources: ResourceMarker[]; // baked fantasy supply-chain source nodes (POI dots) — empty for transit
  towns: TownMarker[]; // baked fantasy towns (sinks + conquest targets) — empty for transit
  decadenceAnchors: DecadenceAnchor[]; // baked far-edge reservoir anchors (the tide origin) — empty for transit
  rivers: RiverSeg[]; // baked flow-accumulation drainage segments (cold water) — empty for transit
  desire: DesireArc[]; // OD "desire lines" from the selected station (on-selection flow overlay)
  reach: ReachDot[]; // accessibility isochrone from the selected station (opt-in "Reach" overlay)
  blueprintInvalid?: boolean; // in-progress route is illegal (e.g. land mode over water) → red ghost
  blueprintColor?: [number, number, number] | null; // extension ghost dashes in the line's own colour
  controlHandles?: ControlHandle[]; // draggable draft control points (waypoints + "+" midpoints)
  pinnedLabel?: { lng: number; lat: number; text: string }; // deck label for the pinned station
  selectedLine?: number | null; // drives the wide selection casing under the selected line
  /** Pre-commit snap highlight: the station the next click would act on (chain in the line tool,
   *  demolish in the bulldozer) — drawn as a ring BEFORE the click commits (AGENTS UX). */
  snapRing?: { lng: number; lat: number; demolish: boolean } | null;
  /** Un-confirmed station the player is about to build (fantasy "confirm build") — a translucent ghost
   *  at the snapped hex cell, drawn until the confirm bar commits or cancels it. */
  ghostStation?: { lng: number; lat: number } | null;
  /** #25 Station-tool HOVER preview: the snapped cell a click WOULD drop a station on (a faint ghost,
   *  red when the one-per-cell rule blocks it) — the pre-commit "highlight the snap candidate" feedback. */
  stationHoverCell?: { lng: number; lat: number; blocked: boolean } | null;
  /** TTD signal markers (single-track block state) — only populated when the Signals lens is on. */
  signals?: SignalMarker[];
  /** TTD L5c player-placed block signals (the posts the player dropped) — shown when their line is the
   *  selected line in build mode (clickable to remove). Distinct glyph from the occupancy `signals` above. */
  placedSignals?: PlacedSignalMarker[];
  /** TTD L5c pre-commit PLACE ghost — where the next click would drop a signal (snap candidate highlight). */
  signalGhost?: SignalGhost | null;
  /** Node nameplates (town/resource name + key stats) — fantasy only; drawn LOD-gated (zoomed in). */
  nodePlates?: NodePlate[];
}

export function colorToRgb(u: number): Rgb {
  return [(u >> 16) & 0xff, (u >> 8) & 0xff, u & 0xff];
}

/** Heavy/high-speed rail mode id (crates/sim trainset::tmode::HEAVY) — gets mainline styling. */
const HEAVY_RAIL = 4;

/** Grid-cell overlays (demand heat, road corridors) draw as flat HEXAGONS, not circles — a
 *  honeycomb reads as one clean tiled field instead of a mud of overlapping discs (the "less
 *  cluttered" ask). A regular hexagon of centre-to-vertex R covers area ≈2.598·R²; matching that
 *  to a square grid cell of pitch p (R≈0.62·p) makes the tiles meet edge-to-edge with neither big
 *  gaps nor heavy overlap. ColumnLayer with diskResolution 6 + extruded:false is the flat hexagon.
 *  Slightly above the area-match so neighbours just kiss into a continuous surface. */
const HEX_FILL = 0.64;
/** Hexagon centre-to-vertex radius (m) for a grid of the given cell pitch (m). */
function hexRadius(cellM: number): number {
  return Math.max(20, cellM * HEX_FILL);
}

/** Demand heat ramp. The primary channel is SERVED vs UNMET, not raw weight: unmet demand
 *  (no station in range) glows warm + solid — the gap to fill; served demand fades cool +
 *  translucent — you've got it covered. Alpha (faint↔solid) is the colour-blind-safe channel,
 *  with warm/cool hue as the secondary cue. Weight still modulates intensity. */
function demandColor(w: number, served?: boolean, cold?: boolean): [number, number, number, number] {
  const t = Math.max(0, Math.min(1, w / 5));
  // #25 transit SERVED is a neutral slate (was cool-blue [90,130,170], close enough to selection-blue [0,114,178]
  // that "you've covered this" and "this station's reach" read as one blue when a selected shed overlapped served
  // cells). Slate recedes ("covered") and keeps selection-blue the player's exclusive active-reach channel; alpha
  // stays the CB-safe served/unmet cue. Matches the arcadia served grey.
  if (served) return cold ? [120, 124, 136, Math.round(8 + t * 22)] : [120, 124, 132, Math.round(10 + t * 26)];
  // ARCADIA: warmth is reserved for the empire, so UNMET supply-demand glows cold-violet (the "cold need"
  // language of the tide), strength still on alpha (CB-safe). TRANSIT keeps the warm unmet-demand heat.
  const a = Math.round(58 + t * 112);
  if (cold) return [110, 100, 140, a];
  const r = Math.round(120 + t * 120);
  const g = Math.round(72 + (1 - t) * 36);
  const b = Math.round(60 - t * 30);
  return [r, g, b, a];
}

/** Waiting-queue band: 0 = a few waiting (faint), 1 = BUSY (amber, watch), 2 = STARVED (red, fix).
 *  Single source for the ring colour/width + its updateTrigger, mirroring the loadPip language. */
function waitBand(count: number): 0 | 1 | 2 {
  if (count >= STARVED_WAITING) return 2;
  if (count >= BUSY_WAITING) return 1;
  return 0;
}
function waitRing(count: number): { color: [number, number, number, number]; width: number } {
  const band = waitBand(count);
  if (band === 2) return { color: [214, 40, 40, 235], width: 3.5 }; // starved — vermillion
  if (band === 1) return { color: [230, 159, 0, 225], width: 2 }; // busy — amber
  // #25 band 0 (a few waiting) is RECESSIVE — a hairline at low alpha — so it doesn't flood the map with amber
  // halos that drown out genuine BUSY/STARVED pressure (the only difficulty lever). Nearly every served station
  // has ≥1 waiting; that must read as background, not alarm.
  return { color: [230, 159, 0, 70], width: 1 };
}

/** Accessibility band for the "Reach" isochrone: travel time (ms) from the selected station →
 *  green (quick) / amber (medium) / red (slow). Bands so it reads as a stoplight, not a smear. */
function reachColor(ms: number): [number, number, number, number] {
  if (ms <= 6 * SIM_MS_PER_CLOCK_MIN) return [0, 158, 115, 90]; // ≤6 clock-min — green
  if (ms <= 15 * SIM_MS_PER_CLOCK_MIN) return [230, 159, 0, 90]; // ≤15 clock-min — amber
  return [213, 94, 0, 90]; // slower — vermillion
}
function reachBand(ms: number): 0 | 1 | 2 {
  return ms <= 6 * SIM_MS_PER_CLOCK_MIN ? 0 : ms <= 15 * SIM_MS_PER_CLOCK_MIN ? 1 : 2;
}

/** Topology layers (rebuilt only on topology/selection change — cached by Game so they keep
 *  a stable identity across frames). Split into below/above the vehicle layer to preserve the
 *  z-order catchment<lines<blueprint<vehicles<stations while only vehicles update per frame. */
export function topoLayers(view: RenderView): { below: Layer[]; above: Layer[] } {
  // #25 busiest station's cumulative boardings — the heatmap radius normalises against THIS (below) so the
  // dot field keeps differentiating on a mature network. The old absolute sqrt cap saturated at +4 once a
  // station passed ~131 boardings, after which every busy interchange read identical — the signal died exactly
  // when there was the most ridership to show. (max 1 to avoid /0 on a fresh map.)
  const maxBoardings = view.stations.reduce((m, s) => Math.max(m, s.boardings), 1);
  const below: Layer[] = [
    // FANTASY TERRAIN (the very back — it IS the map): one flat hexagon per baked buildability cell,
    // coloured by biome as VALUE not hue (ash-grey world, faint-violet ley). The hex circumradius =
    // gridCellMm (terrainCellM) so the pointy-top lattice tiles edge-to-edge. `angle:0` matches deck's
    // hexagon to the pointy-top axial lattice. Stable data identity per city (no per-frame rebuild);
    // empty for transit cities so this layer is a no-op there.
    new ColumnLayer({
      id: "terrain",
      data: view.terrain,
      diskResolution: 6, // hexagon
      // #15 Phase 4 — EXTRUDE each hex to its biome RELIEF (mountains tall, hills rise, plains flat at 0), so
      // the iso-tilted map reads as rolling country instead of a flat plane. The diorama meshes (trees/depots/
      // sprawl) are z-raised to match (sit on the hex top). PLAIN/transit ⇒ elevation 0 ⇒ flat (no-op there).
      extruded: true,
      getElevation: (d: TerrainCell) => reliefM(d.c),
      // circumradius = hex size (×1.04 to kill sub-pixel seams; opaque back layer, overlap is benign).
      // angle:30 rotates deck's default flat-top hexagon to POINTY-TOP, matching the axial lattice
      // (centers spaced √3·size apart) so the honeycomb tiles edge-to-edge with no gaps.
      radius: view.terrainCellM * 1.04,
      radiusUnits: "meters",
      angle: 30,
      getPosition: (d: TerrainCell) => [d.lng, d.lat],
      getFillColor: (d: TerrainCell) => terrainColor(d.c),
      filled: true,
      stroked: false,
      // #25 a quiet matte material so the EXTRUDED hex faces catch the existing directional sun (overlay.ts) —
      // sun-raked slopes get a lit/shadow split, giving the pitch-45 relief real light-and-shadow FORM instead
      // of flat silhouette. No new light, no per-frame cost; the diorama meshes already carry phong materials.
      material: { ambient: 0.65, diffuse: 0.85, shininess: 2, specularColor: [20, 20, 24] },
      updateTriggers: { getFillColor: view.terrain.length, getElevation: view.terrain.length },
    }),
    // #ocean SHORE FOAM: bright surf on the coast WATER hexes (those touching land), on the sea surface
    // (z≈0, under the network). A tiny lift kills z-fighting with the flat water; semi-transparent so the
    // sea shows through, and a per-cell hash breaks the ring into textured surf. Built once at load (stable
    // identity); the wave SHIMMER is added per-frame by the foam shader in composeAndSet. Empty for transit.
    new ColumnLayer({
      id: "shore-foam",
      data: view.coast,
      diskResolution: 6,
      extruded: true,
      getElevation: 3, // a few m proud of the water (≪ the relief scale) so depth-test puts foam ON the sea, not z-fighting it
      // #25 a SMALL disc — buildCoast offsets each foam point toward its land neighbour, so a shrunk radius
      // reads as breaking surf HUGGING the shoreline, not a centre-weighted shallow-water blob out in the sea.
      radius: view.terrainCellM * 0.62,
      radiusUnits: "meters",
      angle: 30,
      getPosition: (d: TerrainCell) => [d.lng, d.lat],
      getFillColor: (d: TerrainCell) => {
        const s = Math.sin(d.lng * 91.7 + d.lat * 47.3) * 43758.5;
        const h = s - Math.floor(s); // per-cell 0..1 hash → a STABLE per-cell surf brightness (the foam line)
        const s2 = Math.sin(d.lng * 53.1 + d.lat * 88.9) * 27183.2;
        const h2 = s2 - Math.floor(s2); // a DECORRELATED hash for the lap phase, so neighbours don't pulse in step
        // #25 a brighter constant foam line (70+70h) with a GENTLE travelling shimmer (45·lap) subordinate to
        // it — surf that lives along the coast rather than the whole shore blinking in near-unison. ~3 Hz lap.
        const lap = 0.5 + 0.5 * Math.sin(((view.foamPhase ?? 0) / 24) * 6.2832 + h2 * 6.2832);
        return [210, 240, 245, Math.round(70 + 70 * h + 45 * lap)];
      },
      filled: true,
      // #24 a thin cool outline on each surf disc → a crisp shore-LINE under the animated foam brightness,
      // so the coast edge reads even at the dim phase of the lap (the fill alone was mushy at low alpha).
      stroked: true,
      getLineColor: [156, 188, 198, 95],
      lineWidthMinPixels: 1,
      updateTriggers: { getFillColor: [view.coast.length, view.foamPhase ?? 0] },
    }),
    // FANTASY 3D DIORAMA (#3d-trees): lowpoly pines standing up on the forest hexes, right on the terrain
    // (under the network/POIs). Empty for transit / at overview (LOD), so it's a no-op there.
    treeLayer(view.trees),
    // #23 TG1 town SPRAWL: ring-cell buildings around towns so a settlement reads as MULTI-CELL (the capital
    // biggest). Under the depots/POIs; arcadia only; LOD-dropped at overview in composeAndSet.
    townSprawlLayer(view.townSprawl),
    // FANTASY 3D STATION DEPOTS (#3d-stations): the player's network nodes as lowpoly platforms + houses,
    // on the terrain under the network/POIs. Arcadia only; LOD-dropped at overview in composeAndSet.
    stationMeshLayer(view.arcadia ? view.stations : []),
    // DECADENCE TIDE (fantasy S10c, over terrain, under everything else): the cold corruption creeping
    // from the far edge toward the warm capital. Value-not-hue per the art direction — a single
    // low-chroma cold violet, STRENGTH = ALPHA (faint at the front, opaque deep). Same pointy-top hex
    // geometry as the terrain so it tiles the lattice. Rebuilt on the ~3 Hz refresh (the tide creeps
    // slowly), never per frame; empty for transit / before the tide starts.
    new ColumnLayer({
      id: "decadence-tide",
      data: view.tideCells,
      diskResolution: 6,
      extruded: false,
      radius: view.terrainCellM * 1.04,
      radiusUnits: "meters",
      angle: 30,
      getPosition: (d: TideCell) => [d.lng, d.lat],
      // Cold grey-violet (low chroma so terrain VALUE reads through). Strength SQUARED → a faint leading
      // edge and a near-solid deep rear; alpha ceiling 150 (was 225) keeps the ground legible even at full
      // rot. The hard wedge edge is replaced by the animated tide-frontier rings below.
      getFillColor: (d: TideCell) => {
        const s = Math.min(1, d.v);
        return [86, 80, 104, Math.round(18 + s * s * 132)];
      },
      filled: true,
      stroked: false,
      updateTriggers: { getFillColor: view.tideCells.length },
    }),
    // TIDE FRONTIER (the ONE decadence telegraph): a moving beaded line of cold-violet rings on the
    // ADVANCING band (0.2 ≤ v ≤ 0.5) — restores the front-edge legibility the softened fill gives up, and
    // its pulse signals the rot is alive + creeping. Derived from the same tide buffer (read-only); the
    // pulse rides a quantized ~3 Hz phase, never per rAF (two-clocks). Empty for transit.
    new ScatterplotLayer({
      id: "tide-front",
      data: view.tideCells.filter((d) => d.v >= 0.2 && d.v <= 0.5),
      getPosition: (d: TideCell) => [d.lng, d.lat],
      getFillColor: [0, 0, 0, 0],
      getLineColor: [120, 110, 150, view.tidePulse ?? 200],
      radiusUnits: "meters",
      getRadius: view.terrainCellM * 0.6,
      stroked: true,
      filled: false,
      lineWidthUnits: "pixels",
      getLineWidth: 1.5,
      lineWidthMinPixels: 1.5,
      updateTriggers: { getLineColor: view.tidePulse, data: view.tideCells.length },
    }),
    // RIVERS (fantasy: baked flow-accumulation drainage, over the tide, under the resources/network): cold
    // water threading the ash continent — believability + the cold-vs-warm-empire read. Pixel width ∝ flow
    // class; fords (cheap headwater crossings) drawn a lighter notch. Stable identity (baked once). Empty for transit.
    new PathLayer({
      id: "rivers",
      data: view.rivers,
      getPath: (d: RiverSeg) => [d.from, d.to],
      // Clear cold steel-blue so the water reads against the dead ash (the cold-vs-warm-empire axis); fords a
      // pale notch. Brighter than the basemap WATER so rivers are legible threads, not lost in dark terrain.
      getColor: (d: RiverSeg) => (d.ford ? [165, 195, 215, 220] : [82, 130, 170, 220]),
      getWidth: (d: RiverSeg) => d.wclass * 1.6 + 1.5,
      widthUnits: "pixels",
      widthMinPixels: 1.5,
      capRounded: true,
      jointRounded: true,
      updateTriggers: { getColor: view.rivers.length, getWidth: view.rivers.length },
    }),
    // RAIL FRONTIER (#infrastructure): the realm's network must be ONE graph rooted at the capital — rail
    // extends only from a station already wired to your seat (or a captured town). So the affordance is
    // per-NODE, not a radius: a soft gold halo rings every RAIL-REACHABLE station ("grow rail from here").
    // A ROOT (capital / captured town) reads brighter + larger — a fresh line may always seed there. Pixel
    // radius (a UI affordance, not a spatial fact), drawn UNDER the POIs + network so it never hides a
    // clickable node. Before any line it is just the capital; it spreads as you build + conquer. Empty for
    // transit. updateTriggers keyed on the set's size+roots so the halos only rebuild when the frontier moves.
    new ScatterplotLayer({
      id: "frontier",
      data: view.frontier,
      getPosition: (d: FrontierNode) => [d.lng, d.lat],
      getRadius: (d: FrontierNode) => (d.root ? 16 : 13),
      radiusUnits: "pixels",
      radiusMinPixels: 11,
      radiusMaxPixels: 18,
      getFillColor: (d: FrontierNode) => (d.root ? [208, 162, 72, 30] : [208, 162, 72, 14]),
      getLineColor: (d: FrontierNode) => (d.root ? [240, 205, 120, 210] : [232, 192, 102, 130]),
      stroked: true,
      filled: true,
      lineWidthUnits: "pixels",
      getLineWidth: (d: FrontierNode) => (d.root ? 2.4 : 1.6),
      lineWidthMinPixels: 1.4,
      updateTriggers: {
        getRadius: view.frontier.length,
        getFillColor: view.frontier.length,
        getLineColor: view.frontier.length,
        getLineWidth: view.frontier.length,
      },
    }),
    // SIEGE-PROGRESS rings (#war): a red ring on a town being ground down by a besieging legion — its
    // intensity + width BUILD as capture nears (progress 0→1), so a sieging legion reads as "winning" and
    // you can see how close + compare two sieges (was a hover-only number). A genuine spatial fact (a town
    // under contest), drawn over the network, under the station dots so it never hides the clickable node.
    new ScatterplotLayer({
      id: "siege-rings",
      data: view.siegeRings,
      getPosition: (d: SiegeRing) => [d.lng, d.lat],
      getRadius: 13,
      radiusUnits: "pixels",
      radiusMinPixels: 10,
      radiusMaxPixels: 16,
      stroked: true,
      filled: false,
      getLineColor: (d: SiegeRing) => [220, 48, 48, Math.round(70 + 150 * Math.max(0, Math.min(1, d.progress)))],
      lineWidthUnits: "pixels",
      getLineWidth: (d: SiegeRing) => 1.5 + 3 * Math.max(0, Math.min(1, d.progress)),
      lineWidthMinPixels: 1.2,
      updateTriggers: {
        getLineColor: view.siegeRings.map((r) => Math.round(r.progress * 20)).join(","),
        getLineWidth: view.siegeRings.map((r) => Math.round(r.progress * 20)).join(","),
      },
    }),
    // FANTASY RESOURCE NODES (over terrain, under the network): the supply-chain sources that gate the
    // two chains. Pixel-radius (clamped) so they stay tappable at any zoom (Fitts); white stroke so the
    // coloured dots pop on the grey continent. Stable identity per city (baked, never per-frame).
    new ScatterplotLayer({
      id: "resources",
      data: view.resources,
      getPosition: (d: ResourceMarker) => [d.lng, d.lat],
      getFillColor: (d: ResourceMarker) => resourceColor(d.kind),
      getLineColor: [245, 245, 245, 200],
      radiusUnits: "pixels",
      // larger than the station dot (≤8px) so the kind-colour halos AROUND the placed source's dot
      getRadius: 10,
      radiusMinPixels: 9,
      radiusMaxPixels: 12,
      stroked: true,
      lineWidthUnits: "pixels",
      getLineWidth: 1,
      filled: true,
      updateTriggers: { getFillColor: view.resources.length },
    }),
    // RESOURCE ICONS (#4): a glyph per kind over the coloured source dots (⛏ ore / ✿ grain / ♣ fuel /
    // ✦ aether / ⚒ forge). A white glyph with a dark SDF OUTLINE so it reads on ANY dot colour — a plain
    // white glyph vanished on the light gold grain dot. Bumped a touch (13→15px) for legibility; the
    // characterSet seeds the atlas so the symbols render (an omitted glyph would silently draw nothing).
    new TextLayer({
      id: "resource-icons",
      data: view.resources,
      getPosition: (d: ResourceMarker) => [d.lng, d.lat],
      getText: (d: ResourceMarker) => resourceGlyph(d.kind),
      getSize: 15,
      sizeUnits: "pixels",
      getColor: [252, 252, 252, 255],
      fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",sans-serif',
      fontSettings: { sdf: true },
      outlineWidth: 2,
      outlineColor: [20, 23, 28, 235],
      characterSet: NODE_CHARSET,
      getTextAnchor: "middle",
      getAlignmentBaseline: "center",
      updateTriggers: { getText: view.resources.length },
    }),
    // DECADENCE RESERVOIR anchors (the far-edge tide origin): low-chroma cold-violet dots — the corruption
    // source the conquest race runs against. (The full creeping field is the S10 CA; this is the S4 seed.)
    new ScatterplotLayer({
      id: "decadence-anchors",
      data: view.decadenceAnchors,
      getPosition: (d: DecadenceAnchor) => [d.lng, d.lat],
      getFillColor: [86, 80, 104, 170], // low-chroma cold grey-violet (matches the tamed tide)
      radiusUnits: "pixels",
      getRadius: 7,
      radiusMinPixels: 5,
      radiusMaxPixels: 11,
      stroked: false,
      filled: true,
      updateTriggers: { getFillColor: view.decadenceAnchors.length },
    }),
    // FANTASY TOWNS (over resources, under the network): supply sinks + conquest targets. Capital/starter
    // warm (your dominion), neutrals sickly cold-bright darkening with their decadence floor. Pixel-radius
    // (value-scaled, clamped) + a dark ring so they read as settlements over the resource dots.
    new ScatterplotLayer({
      id: "towns",
      data: view.towns,
      getPosition: (d: TownMarker) => [d.lng, d.lat],
      getFillColor: (d: TownMarker) => townColor(d.kind, d.decadence),
      getLineColor: (d: TownMarker) => townRingColor(d.chain), // ring = supply chain (bread/arms)
      getRadius: (d: TownMarker) => townRadius(d.kind, d.value),
      radiusUnits: "pixels",
      radiusMinPixels: 9,
      radiusMaxPixels: 16,
      stroked: true,
      lineWidthUnits: "pixels",
      getLineWidth: 2.5,
      filled: true,
      updateTriggers: {
        getFillColor: view.towns.map((t) => `${t.kind}:${t.decadence}`).join(","),
        getLineColor: view.towns.map((t) => t.chain).join(","),
        getRadius: view.towns.map((t) => `${t.kind}:${t.value}`).join(","),
      },
    }),
    // TOWN ICONS (#13): a distinctive glyph per kind on each settlement (★ citadel / ✪ starter hold /
    // ⌂ neutral town). Dark for contrast on the bright town fills. Over the town dots, under the network.
    new TextLayer({
      id: "town-icons",
      data: view.towns,
      getPosition: (d: TownMarker) => [d.lng, d.lat],
      getText: (d: TownMarker) => townGlyph(d.kind),
      getSize: (d: TownMarker) => (d.kind === "capital" ? 18 : 14),
      sizeUnits: "pixels",
      getColor: [28, 26, 30, 245],
      fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",sans-serif',
      characterSet: NODE_CHARSET,
      getTextAnchor: "middle",
      getAlignmentBaseline: "center",
      updateTriggers: { getText: view.towns.map((t) => t.kind).join(","), getSize: view.towns.map((t) => t.kind).join(",") },
    }),
    // ROAD corridors (very back, under the network): the cells where a bus runs cheap + fast.
    // Muted slate so it reads as ground truth, not network identity. Metre-radius so it scales
    // with the map. updateTriggers unneeded — `roads` is a stable memoized array per city.
    new ColumnLayer({
      id: "roads",
      data: view.roads,
      diskResolution: 6, // hexagon (honeycomb tiling, not overlapping discs)
      extruded: false, // flat 2D fill on the ground, not a 3D column
      radius: hexRadius(view.roadCellM),
      radiusUnits: "meters",
      getPosition: (d: RoadCell) => [d.lng, d.lat],
      // Live congestion: green (flowing) → amber → red (jammed), from the cell's built-up density
      // and the current hour. Stable data identity; recolours only when the hour ticks over.
      getFillColor: (d: RoadCell) => roadColor(view.roadHour, d.density),
      filled: true,
      stroked: false,
      updateTriggers: { getFillColor: view.roadHour },
    }),
    // Travel-demand heat (bottom of the stack so the network draws over it): a HONEYCOMB of flat
    // hexagons, one per demand cell, tiling the grid. Uniform cell size keeps it tidy — weight is
    // the COLOUR channel (warm+solid unmet → cool+faint served), not the size, so it reads as one
    // clean heat field instead of a mud of weight-scaled overlapping discs.
    new ColumnLayer({
      id: "demand-heat",
      data: view.demand,
      diskResolution: 6, // hexagon
      extruded: false, // flat fill, not a 3D column
      radius: hexRadius(view.demandCellM),
      radiusUnits: "meters",
      getPosition: (d: DemandPoint) => [d.lng, d.lat],
      getFillColor: (d: DemandPoint) => demandColor(d.weight, d.served, view.arcadia),
      filled: true,
      stroked: false,
      // `demand` is a fresh array only when the served set is recomputed (topology/toggle), so
      // identity is stable across frames; this trigger guards the in-place served recolor.
      updateTriggers: { getFillColor: view.demand.map((d) => (d.served ? 1 : 0)).join("") + (view.arcadia ? "c" : "") },
    }),
    // Walk shed (hexagons over the reachable buildability cells): the REAL, lopsided catchment —
    // water severs it, a crossed motorway pinches it. Catchment blue, alpha fading with the
    // distance-decay intensity. Empty for grid-less cities (then the ring below carries the fill).
    new ColumnLayer({
      id: "walkshed",
      data: view.shed,
      diskResolution: 6, // hexagon
      extruded: false, // flat fill
      // #19 align the catchment to the terrain hex lattice: in arcadia the reachable shed cells ARE the
      // terrain hex cells, so the shed hexagons MUST match the terrain's hexagon geometry — same
      // circumradius (terrainCellM*1.04) AND pointy-top orientation (angle:30). Sizing by the road pitch
      // (hexRadius) + the default flat-top angle left them undersized + rotated 30° off the lattice
      // (the misalignment). Transit (no hex lattice) keeps the road-grid pitch + deck's default angle.
      radius: view.terrainCellM > 0 ? view.terrainCellM * 1.04 : hexRadius(view.roadCellM),
      radiusUnits: "meters",
      angle: view.terrainCellM > 0 ? 30 : 0,
      getPosition: (d: ShedHex) => [d.lng, d.lat],
      getFillColor: (d: ShedHex) => [0, 114, 178, Math.round(48 + Math.min(1, d.intensity) * 120)],
      filled: true,
      stroked: false,
      updateTriggers: { getFillColor: view.shed.length },
    }),
    new ScatterplotLayer({
      id: "catchments",
      data: view.catchments,
      getPosition: (d: CatchmentCircle) => [d.lng, d.lat],
      getRadius: (d: CatchmentCircle) => d.radiusM,
      radiusUnits: "meters",
      // Pinned (selected) station = filled + solid stroke; hover peek = stroke-only, fainter, so a
      // peek reads as provisional and never greys out what's under it. The pinned fill alpha scales
      // with captured demand (28..96) so a station on heavy demand reads denser than one on empty
      // land. BUT when we have a real walk shed (hexagons above), the ring drops to a stroke-only
      // NOMINAL-reach outline — the gap between the 500 m circle and the lopsided fill IS the
      // barrier penalty (the ped-shed deficit), so we don't fill over it.
      getFillColor: (d: CatchmentCircle) =>
        d.peek || view.shed.length > 0
          ? [0, 114, 178, 0]
          : [0, 114, 178, Math.round(28 + Math.min(1, d.demand ?? 0) * 68)],
      stroked: true,
      getLineColor: (d: CatchmentCircle) =>
        view.shed.length > 0 ? [0, 114, 178, 90] : d.peek ? [0, 114, 178, 110] : [0, 114, 178, 180],
      lineWidthMinPixels: 1.5,
      updateTriggers: {
        getFillColor: view.catchments.map((c) => `${!!c.peek}:${Math.round((c.demand ?? 0) * 10)}`).join(",") + `:${view.shed.length > 0}`,
        getLineColor: view.catchments.map((c) => !!c.peek).join(",") + `:${view.shed.length > 0}`,
      },
    }),
    // Selected-line emphasis: a wide dark casing under the picked line so it pops on the muted
    // basemap regardless of hue (width + dark frame = colour-blind-safe, not a hue change). Wider
    // than the heavy-rail casing so it frames even mainline track. Bumps only on selection change.
    // (Includes bare track — the casing is SELECTION feedback, so a selected unserved corridor frames too.)
    new PathLayer({
      id: "lines-selected-casing",
      data: view.selectedLine == null ? [] : view.lines.filter((d) => d.id === view.selectedLine),
      getPath: (d: LinePath) => d.path,
      getColor: [34, 34, 40, 220],
      getWidth: 15,
      widthUnits: "pixels",
      widthMinPixels: 11,
      capRounded: true,
      jointRounded: true,
      updateTriggers: { getColor: view.selectedLine ?? -1 },
    }),
    // Heavy / high-speed rail reads as MAINLINE track, not a flat metro stroke: a dark casing
    // under a wider colored core with a pale centre stripe (a "double-track" look). Only the
    // heavy lines are in these two extra layers; metro/bus/ferry/air stay in the flat "lines".
    new PathLayer({
      id: "lines-heavy-casing",
      data: view.lines.filter((d) => d.mode === HEAVY_RAIL && d.serviced !== false),
      getPath: (d: LinePath) => d.path,
      getColor: [34, 34, 40, 255],
      getWidth: 13,
      widthUnits: "pixels",
      widthMinPixels: 9,
      capRounded: true,
      jointRounded: true,
    }),
    // TTD L6 (track + services): BARE TRACK — a line with no assigned stock — draws as ONE muted grey rail.
    // Placed ABOVE the selected-casing (so a selected bare track shows grey rail framed by the dark casing,
    // not a solid dark blob) but BELOW the coloured `lines` (so a service routed over track lights up on top).
    // Co-located bare tracks overlap into one grey corridor; pickable so the player can select track to stock it.
    new PathLayer({
      id: "track-rails",
      data: view.lines.filter((d) => d.serviced === false),
      getPath: (d: LinePath) => d.path,
      // #25 the rival lays bare TRACK (no service) to march its hosts — so it reads here, not in the coloured
      // "lines" layer. Render the rival's track crimson + wider so the enemy's advance is a threat, not the
      // anonymous grey of the player's own unserviced infra.
      getColor: (d: LinePath) => (d.faction === 1 ? [216, 64, 52, 235] : [122, 128, 136, 205]),
      getWidth: (d: LinePath) => (d.faction === 1 ? 11 : 8),
      widthUnits: "pixels",
      widthMinPixels: 5,
      capRounded: true,
      jointRounded: true,
      pickable: true,
      updateTriggers: {
        getColor: view.lines.filter((d) => d.serviced === false).length + 7919 * view.lines.filter((d) => d.faction === 1).length,
        getWidth: view.lines.filter((d) => d.faction === 1 && d.serviced === false).length,
      },
    }),
    new PathLayer({
      id: "lines",
      // TTD L6: only SERVICED lines wear their colour; bare track is the grey `track-rails` layer below.
      data: view.lines.filter((d) => d.serviced !== false),
      getPath: (d: LinePath) => d.path,
      getColor: (d: LinePath) => d.color,
      // #25 the RIVAL's rail (faction 1) is drawn bolder than the player's so the enemy's creep toward the
      // capital is a legible threat, not a faint thread lost among the player's lines at strategic zoom.
      getWidth: (d: LinePath) => (d.faction === 1 ? 12 : d.mode === HEAVY_RAIL ? 9 : 7),
      widthUnits: "pixels",
      // The network is the FIGURE — keep the coloured ribbon wider than the station/vehicle dots
      // (~4px) so it reads as a continuous line, not a string of beads under the dot field.
      widthMinPixels: 5,
      capRounded: true,
      jointRounded: true,
      // Pickable so hovering the track raises the line inspector (under stations + trains in
      // z-order, so it only fires on bare track). The pick hit-area widens with pickingRadius.
      pickable: true,
      updateTriggers: {
        getColor: view.lines.filter((d) => d.serviced !== false).length,
        getWidth: view.lines.filter((d) => d.faction === 1).length, // rebuild widths when a rival line appears
      },
    }),
    new PathLayer({
      id: "lines-heavy-centre",
      data: view.lines.filter((d) => d.mode === HEAVY_RAIL && d.serviced !== false),
      getPath: (d: LinePath) => d.path,
      getColor: [245, 245, 250, 220],
      getWidth: 2,
      widthUnits: "pixels",
      widthMinPixels: 1,
      capRounded: true,
      jointRounded: true,
    }),
    // Rail-attack (#war): a RAIDED line glows angry red OVER its own colour — a raider severed it, its
    // trains are frozen until it recovers. Drawn just over the network so the cut reads instantly; the
    // line's hue still peeks at the casing edges, so identity isn't lost. updateTriggers keyed on the raided
    // set so the overlay only rebuilds when a line is cut or re-opens (no per-frame churn).
    new PathLayer({
      id: "lines-raided",
      // Only a SERVICED line shows the raided red stripe (bare track has no trains to freeze) — matches the
      // other coloured layers' serviced filter, so a cut never paints red over an otherwise-grey corridor.
      data: view.lines.filter((d) => d.raided && d.serviced !== false),
      getPath: (d: LinePath) => d.path,
      getColor: [224, 48, 48, 205],
      getWidth: 5,
      widthUnits: "pixels",
      widthMinPixels: 3,
      capRounded: true,
      jointRounded: true,
      updateTriggers: { getColor: view.lines.filter((d) => d.raided).length },
    }),
    // OD "desire lines" from the SELECTED station only (on-demand → never mud): curved arcs to the
    // destinations its riders are drawn toward. Origin = selection blue, destination = warm, with
    // width + dest opacity scaling with the gravity pull — "where do people here want to go". Over
    // the network lines, under vehicles/stations so the live network + dots stay on top + clickable.
    new ArcLayer({
      id: "desire",
      data: view.desire,
      getSourcePosition: (d: DesireArc) => d.from,
      getTargetPosition: (d: DesireArc) => d.to,
      getSourceColor: [0, 114, 178, 150],
      getTargetColor: (d: DesireArc) => [214, 110, 0, Math.round(70 + d.weight * 150)],
      getWidth: (d: DesireArc) => 1.5 + d.weight * 5,
      widthUnits: "pixels",
      widthMinPixels: 1.5,
      getHeight: 0.4,
      updateTriggers: {
        getTargetColor: view.desire.map((d) => Math.round(d.weight * 10)).join(","),
        getWidth: view.desire.map((d) => Math.round(d.weight * 10)).join(","),
      },
    }),
  ];

  if (view.blueprint.length > 1) {
    // Provisional ghost: muted grey for a NEW line, the line's OWN colour when EXTENDING one
    // (the ghost reads as "continuing this line", not starting another), red when illegal
    // (NIMBY's blue/red blueprint signal). updateTriggers so the colour flips with state.
    const ghost: [number, number, number, number] = view.blueprintInvalid
      ? [214, 40, 40, 220]
      : view.blueprintColor
        ? [view.blueprintColor[0], view.blueprintColor[1], view.blueprintColor[2], 210]
        : [120, 124, 130, 190];
    below.push(
      new PathLayer({
        id: "blueprint",
        data: [{ path: view.blueprint }],
        getPath: (d: { path: [number, number][] }) => d.path,
        getColor: ghost,
        getWidth: 4,
        widthUnits: "pixels",
        widthMinPixels: 3,
        capRounded: true,
        jointRounded: true,
        updateTriggers: { getColor: ghost.join(",") },
      }),
    );
  }

  const above: Layer[] = [
    // Accessibility isochrone ("Reach" overlay, opt-in): a soft green→amber→red filled halo behind
    // each station reachable from the selected one, by transit travel time. Filled (not stroked) so
    // it reads distinctly from the stroke-only waiting rings, and under the station dots so they
    // stay on top + clickable. Empty unless the toggle is on AND a served station is pinned.
    new ScatterplotLayer({
      id: "reach",
      data: view.reach,
      getPosition: (d: ReachDot) => [d.lng, d.lat],
      getRadius: 13,
      radiusUnits: "pixels",
      radiusMinPixels: 10,
      stroked: false,
      filled: true,
      getFillColor: (d: ReachDot) => reachColor(d.ms),
      updateTriggers: { getFillColor: view.reach.map((d) => reachBand(d.ms)).join(",") },
    }),
    new ScatterplotLayer({
      id: "stations",
      data: view.stations,
      getPosition: (d: StationDot) => [d.lng, d.lat],
      // Radius grows with cumulative boardings (sqrt, capped) so the static dot field becomes a
      // usage heatmap — busy stations swell. Selected adds a bump. Kept SMALLER than the line width
      // so 177 stops read as ticks ON the ribbon, not a swarm of beads obscuring it.
      // #25 a rival HOLD (faction 1) gets a larger base radius — the enemy realm expands by SEIZING nodes
      // (its rail isn't drawn as a line), so its holds ARE its visible advance and must read at strategic zoom.
      getRadius: (d: StationDot) => (d.faction === 1 ? 6.5 : d.selected ? 7 : 4) + 4 * Math.sqrt(Math.min(1, d.boardings / maxBoardings)),
      radiusUnits: "pixels",
      radiusMinPixels: 3,
      // Selected fill = selection blue (ties to its blue catchment ring). Otherwise an ORPHANED
      // station (no operational line serving it) is muted grey and a SERVED one is near-black, so
      // stations visibly "light up" as you connect + run them (place→draw→assign cause→effect).
      // #13/#25: a RIVAL-owned node (faction 1) reads as a HOT threat — a brighter, more-saturated crimson
      // so the enemy realm's holds stand out from the player's served/orphaned greys at a glance.
      getFillColor: (d: StationDot) =>
        // #25 orphaned fill cooled/darkened to [96,102,112] so a station on bare track (rail is [122,128,136])
        // reads as a distinct bead on the rail — "track but no service" stays legible in the place→draw→assign chain.
        d.selected ? [0, 114, 178] : d.faction === 1 ? [228, 52, 44] : d.serving > 0 ? [28, 32, 36] : [96, 102, 112],
      stroked: true,
      // #25 a hostile AMBER ring on rival holds (vs the player's white) — the enemy reads as menacing, not
      // just "another colour", and the warm ring pops the crimson off the grey terrain.
      getLineColor: (d: StationDot) => (d.faction === 1 ? [255, 198, 120, 245] : [255, 255, 255, 230]),
      lineWidthMinPixels: 1,
      pickable: true,
      updateTriggers: {
        getFillColor: view.stations.map((s) => `${s.selected}:${s.serving > 0}:${s.faction ?? 0}`).join(","),
        getLineColor: view.stations.map((s) => s.faction ?? 0).join(","),
        getRadius: view.stations.map((s) => `${s.selected}:${s.faction ?? 0}:${Math.round(Math.sqrt(Math.min(1, s.boardings / maxBoardings)) * 8)}`).join(","),
      },
    }),
    // Ghost station (fantasy "confirm build"): a translucent selection-blue disc at the snapped hex
    // cell, shown while a placement awaits the confirm bar. Pickable:false so it never blocks a click.
    ...(view.ghostStation
      ? [
          new ScatterplotLayer({
            id: "ghost-station",
            data: [view.ghostStation],
            getPosition: (d: { lng: number; lat: number }) => [d.lng, d.lat],
            getRadius: 8,
            radiusUnits: "pixels",
            radiusMinPixels: 6,
            getFillColor: [0, 114, 178, 90] as [number, number, number, number],
            stroked: true,
            getLineColor: [255, 255, 255, 235] as [number, number, number, number],
            lineWidthMinPixels: 2,
            pickable: false,
          }),
        ]
      : []),
    // #25 Station-tool HOVER preview: a faint ghost at the snapped cell the cursor is over (BEFORE any click),
    // tinted red when the one-per-cell rule would block it — the sub-100 ms "where will this land" feedback the
    // place gesture lacked. Fainter than the committed ghost-station; pickable:false so it never eats a click.
    ...(view.stationHoverCell
      ? [
          new ScatterplotLayer({
            id: "station-hover",
            data: [view.stationHoverCell],
            getPosition: (d: { lng: number; lat: number }) => [d.lng, d.lat],
            getRadius: 8,
            radiusUnits: "pixels",
            radiusMinPixels: 6,
            getFillColor: (d: { blocked: boolean }) => (d.blocked ? [214, 40, 40, 55] : [0, 114, 178, 55]) as [number, number, number, number],
            stroked: true,
            getLineColor: (d: { blocked: boolean }) => (d.blocked ? [214, 40, 40, 205] : [255, 255, 255, 170]) as [number, number, number, number],
            lineWidthMinPixels: 1.5,
            pickable: false,
            updateTriggers: { getFillColor: view.stationHoverCell.blocked, getLineColor: view.stationHoverCell.blocked },
          }),
        ]
      : []),
    // Bounty markers (fantasy): a gold ring around each town the player has posted a bounty on — the
    // steering lever's visual feedback (you SEE where you've baited the legions). Font-independent (a
    // ring, not a glyph). Few in number; rebuilt on refresh (bounties change via a Command), not per frame.
    new ScatterplotLayer({
      id: "bounty-markers",
      data: view.stations.filter((s) => (s.bounty ?? 0) > 0),
      getPosition: (d: StationDot) => [d.lng, d.lat],
      getRadius: 12,
      radiusUnits: "pixels",
      radiusMinPixels: 10,
      stroked: true,
      filled: false,
      // Warm ORANGE (not yellow-gold) + thicker, so the actionable bounty halo separates from the pale-gold
      // frontier root halo it co-locates with on a town (#war clutter: the gold-overload near the capital).
      getLineColor: [232, 120, 16, 255],
      lineWidthMinPixels: 2.6,
      updateTriggers: { getLineColor: view.stations.map((s) => ((s.bounty ?? 0) > 0 ? 1 : 0)).join(",") },
    }),
    // Waiting-passenger halo: a ring that grows with the queue (top, so a starved station is always
    // visible). Stroked-only so it doesn't occlude the station dot. Three bands so "filling up"
    // reads BEFORE "starved": a faint thin ring under BUSY (a few people, fine), solid amber once
    // BUSY (watch this), thick vermillion once STARVED (fix the headway). updateTriggers on the
    // band membership (a string), never per frame.
    new ScatterplotLayer({
      id: "waiting",
      data: view.waiting,
      getPosition: (d: WaitingDot) => [d.lng, d.lat],
      // Capped well below the old 8–24px: these only show zoomed-in now (LOD), so a tight 5–12px
      // ring is plenty to read the queue without ballooning into the dominant on-map mark.
      getRadius: (d: WaitingDot) => (waitBand(d.count) === 0 ? 4 : 5 + Math.min(7, Math.sqrt(d.count) * 1.5)),
      radiusUnits: "pixels",
      stroked: true,
      filled: false,
      getLineColor: (d: WaitingDot) => waitRing(d.count).color,
      getLineWidth: (d: WaitingDot) => waitRing(d.count).width,
      lineWidthUnits: "pixels",
      lineWidthMinPixels: 1.5,
      updateTriggers: {
        getRadius: view.waiting.map((w) => w.count).join(","),
        getLineColor: view.waiting.map((w) => waitBand(w.count)).join(","),
        getLineWidth: view.waiting.map((w) => waitBand(w.count)).join(","),
      },
    }),
    // NODE BUFFER-FILL gauges (fantasy #8): a small filled pip on each node whose colour reads its Forge-Line
    // buffer — slate (low) → amber (filling) → vermillion (backed up: ship it!). Driven by the ~3 Hz stats
    // slice (not per frame). Empty for transit. Bands keyed for the updateTrigger.
    new ScatterplotLayer({
      id: "buffer-pips",
      data: view.bufferPips,
      getPosition: (d: BufferPip) => [d.lng, d.lat],
      getFillColor: (d: BufferPip) =>
        d.fill >= 0.85 ? [214, 60, 40, 240] : d.fill >= 0.5 ? [230, 159, 0, 230] : [120, 132, 144, 205],
      getRadius: (d: BufferPip) => 3 + Math.min(3, d.fill * 3),
      radiusUnits: "pixels",
      radiusMinPixels: 3,
      radiusMaxPixels: 7,
      stroked: true,
      getLineColor: [20, 20, 24, 200],
      lineWidthMinPixels: 1,
      filled: true,
      updateTriggers: {
        getFillColor: view.bufferPips.map((b) => (b.fill >= 0.85 ? 2 : b.fill >= 0.5 ? 1 : 0)).join(","),
        getRadius: view.bufferPips.map((b) => Math.round(b.fill * 4)).join(","),
      },
    }),
    // Starved-only halos for the city overview: below DETAIL_ZOOM the full "waiting" layer is
    // LOD-dropped (hundreds of small queues read as a flashing swarm), but a STARVED platform is
    // exactly the signal a player zooms out to compare — so the worst band stays visible at any
    // zoom. composeAndSet shows precisely one of {waiting, waiting-overview} per frame.
    new ScatterplotLayer({
      id: "waiting-overview",
      data: view.waiting.filter((w) => w.count >= STARVED_WAITING),
      getPosition: (d: WaitingDot) => [d.lng, d.lat],
      getRadius: (d: WaitingDot) => (waitBand(d.count) === 0 ? 4 : 5 + Math.min(7, Math.sqrt(d.count) * 1.5)),
      radiusUnits: "pixels",
      stroked: true,
      filled: false,
      getLineColor: (d: WaitingDot) => waitRing(d.count).color,
      getLineWidth: (d: WaitingDot) => waitRing(d.count).width,
      lineWidthUnits: "pixels",
      lineWidthMinPixels: 1.5,
      updateTriggers: {
        getRadius: view.waiting.map((w) => w.count).join(","),
      },
    }),
    // Live build-conflict dots along the in-progress blueprint (amber built/park, red water).
    new ScatterplotLayer({
      id: "hazards",
      data: view.hazards,
      getPosition: (d: HazardDot) => [d.lng, d.lat],
      getRadius: 4,
      radiusUnits: "pixels",
      getFillColor: (d: HazardDot) => d.color,
      stroked: false,
    }),
    // Pinned-station label (deck geometry, NOT a DOM node anchored by lng/lat). One line at the
    // selected station; data length 0/1 so it costs nothing when nothing is pinned. characterSet
    // "auto" so names with non-ASCII glyphs render. updateTriggers on the label id/text only.
    new TextLayer<{ lng: number; lat: number; text: string }>({
      id: "station-label",
      data: view.pinnedLabel ? [view.pinnedLabel] : [],
      getPosition: (d) => [d.lng, d.lat],
      getText: (d) => d.text,
      characterSet: "auto",
      getSize: 12,
      sizeUnits: "pixels",
      getColor: [28, 32, 36, 255],
      getPixelOffset: [0, -16],
      fontWeight: 700,
      background: true,
      getBackgroundColor: [255, 255, 255, 235],
      backgroundPadding: [5, 3],
      getTextAnchor: "middle",
      getAlignmentBaseline: "bottom",
      updateTriggers: {
        getText: view.pinnedLabel?.text ?? "",
        getPosition: view.pinnedLabel ? `${view.pinnedLabel.lng},${view.pinnedLabel.lat}` : "",
      },
    }),
    // Rail-attack (#war): the "⚔ RAIDED Xs" badge at each cut line's midpoint — a red plate with the live
    // recovery countdown, so the severed supply line + its time-to-reopen read at a glance (front pressure).
    new TextLayer<RaidLabel>({
      id: "raid-labels",
      data: view.raidLabels,
      getPosition: (d) => [d.lng, d.lat],
      getText: (d) => d.text,
      characterSet: "auto",
      getSize: 12,
      sizeUnits: "pixels",
      getColor: [255, 244, 238, 255],
      fontWeight: 700,
      background: true,
      getBackgroundColor: [184, 28, 28, 235],
      backgroundPadding: [6, 3],
      getTextAnchor: "middle",
      getAlignmentBaseline: "center",
      updateTriggers: { getText: view.raidLabels.map((r) => r.text).join("|") },
    }),
    // BARRACKS badge (#war): a ⚔ over each legion-spawn node, GOLD when the realm can field a legion
    // (manpower ready) / GREY when starved — so the player can SEE where legions muster + whether the base
    // is fed (it was an unmarked generic station). Pixel-offset above the dot so it doesn't hide it.
    new TextLayer<BarracksBadge>({
      id: "barracks-badges",
      data: view.barracksBadges,
      getPosition: (d) => [d.lng, d.lat],
      getText: () => "⚔",
      characterSet: "⚔",
      getSize: 13,
      sizeUnits: "pixels",
      getColor: (d) => (d.ready ? [236, 188, 92, 255] : [150, 150, 156, 220]),
      getPixelOffset: [0, -14],
      fontWeight: 700,
      fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",sans-serif',
      getTextAnchor: "middle",
      getAlignmentBaseline: "center",
      updateTriggers: { getColor: view.barracksBadges.map((b) => b.ready).join(",") },
    }),
  ];

  // Pre-commit snap ring: marks the station the next click would chain (line tool, white) or
  // demolish (bulldozer, red) — the candidate is visible BEFORE the click commits. One datum,
  // costs nothing when null; sized just over the station dot + its own stroke so it reads as a
  // target reticle, not another state dot.
  if (view.snapRing) {
    above.push(
      new ScatterplotLayer({
        id: "snap-ring",
        data: [view.snapRing],
        getPosition: (d: { lng: number; lat: number }) => [d.lng, d.lat],
        getRadius: 11,
        radiusUnits: "pixels",
        stroked: true,
        filled: false,
        // Selection blue = "the next click acts here"; demolition red for the bulldozer. (Plain
        // white would wash out against the light basemap + the stations' own white strokes.)
        getLineColor: view.snapRing.demolish ? [214, 40, 40, 240] : [0, 114, 178, 240],
        getLineWidth: 2.5,
        lineWidthUnits: "pixels",
      }),
    );
  }

  // Draggable control points for the in-progress line (top of the stack so they're grabbable):
  // a faint "+" at each sub-segment midpoint (drag to bend the track there) and a solid dot per
  // existing waypoint (drag to reshape; double-click to remove). Hit-tested in screen space by
  // Game (not deck-picked), so they need no `pickable`.
  const handles = view.controlHandles ?? [];
  if (handles.length > 0) {
    above.push(
      new ScatterplotLayer({
        id: "control-add",
        data: handles.filter((h) => h.kind === "add"),
        getPosition: (h: ControlHandle) => [h.lng, h.lat],
        getRadius: 5,
        radiusUnits: "pixels",
        radiusMinPixels: 4,
        getFillColor: [255, 255, 255, 200],
        stroked: true,
        getLineColor: [120, 124, 130, 230],
        lineWidthMinPixels: 1.5,
      }),
      new ScatterplotLayer({
        id: "control-waypoint",
        data: handles.filter((h) => h.kind === "waypoint"),
        getPosition: (h: ControlHandle) => [h.lng, h.lat],
        getRadius: 6,
        radiusUnits: "pixels",
        radiusMinPixels: 5,
        getFillColor: [0, 114, 178, 235],
        stroked: true,
        getLineColor: [255, 255, 255, 255],
        lineWidthMinPixels: 2,
      }),
    );
  }

  // Node NAMEPLATES (LOD): a name + key-stat plate above each town/resource node, so the supply graph
  // reads at a glance when zoomed in. Dropped at the strategic overview (composeAndSet's !detail filter).
  if (view.nodePlates && view.nodePlates.length > 0) {
    above.push(
      new TextLayer<NodePlate>({
        id: "node-plates",
        data: view.nodePlates,
        getPosition: (d: NodePlate) => [d.lng, d.lat],
        getText: (d: NodePlate) => `${d.title}\n${d.sub}`,
        getSize: 11,
        sizeUnits: "pixels",
        getColor: [238, 240, 244, 255],
        getPixelOffset: [0, -20], // float the plate above the node glyph
        background: true,
        getBackgroundColor: [20, 24, 30, 218],
        backgroundPadding: [8, 4],
        // A thin gilt border turns the dark box into a game-like PLAQUE (#22) — reads as a placard, not a
        // debug label. Uniform low-alpha gold pairs with the icon-prefixed title.
        getBorderColor: [196, 170, 110, 150],
        getBorderWidth: 1,
        getTextAnchor: "middle",
        getAlignmentBaseline: "bottom",
        lineHeight: 1.2,
        fontFamily: '"Segoe UI Symbol", system-ui, sans-serif',
        characterSet: "auto",
        fontSettings: { sdf: true },
        outlineWidth: 1,
        outlineColor: [0, 0, 0, 180],
        pickable: false,
        updateTriggers: { getText: view.nodePlates.map((p) => `${p.title}|${p.sub}`).join(",") },
      }),
    );
  }

  return { below, above };
}


/** The per-frame vehicle layers (moving trains): a line-coloured body whose radius grows and
 *  whose outline shifts white→amber→red with load (identity + crowding, always visible against
 *  the same-coloured track via its contrasting stroke), with a small WHITE triangle on top
 *  rotated to the heading so you read which way each train is travelling. Both below stations so
 *  platforms stay clickable. Returned as an array spliced into the z-order between topo below/above. */
/** Living-world (#living): the ambient trade carts as one small-dot ScatterplotLayer. Warm earthy ox-cart
 *  brown; served (industrialised) routes fade out. Rebuilt per frame like the vehicle layer (small, cheap —
 *  the "never rebuild" rule guards the heavy cached topo layers, not these per-frame motion dots). Not pickable. */
export function ambientTraderLayer(carts: AmbientTrader[]): Layer {
  return new ScatterplotLayer({
    id: "ambient-traders",
    data: carts,
    getPosition: (d: AmbientTrader) => [d.lng, d.lat],
    // The cart is COLOURED BY ITS CARGO (so the good reads at a glance); a railed route fades.
    getFillColor: (d: AmbientTrader) => (d.dim ? [d.tint[0], d.tint[1], d.tint[2], 70] : [d.tint[0], d.tint[1], d.tint[2], 235]),
    getRadius: 3.4,
    radiusUnits: "pixels",
    radiusMinPixels: 2.2,
    radiusMaxPixels: 5,
    stroked: true,
    getLineColor: (d: AmbientTrader) => (d.dim ? [40, 34, 28, 70] : [38, 30, 22, 210]),
    getLineWidth: 0.7,
    lineWidthUnits: "pixels",
    lineWidthMinPixels: 0.6,
    pickable: false,
    updateTriggers: { getFillColor: carts.map((c) => `${c.dim ? "d" : ""}${c.tint.join("")}`).join("") },
  });
}

/** Every cargo glyph an ambient cart can carry — seeds deck's TextLayer atlas so the symbols render. */
const CARGO_CHARSET = "⛏✿♣✦⚒◆⚔❖";

/** Living-world (#living): the cargo a cart hauls as a tiny symbol riding just above it (so you can SEE
 *  what's being transported). Reuses the node-glyph language; small + muted so it doesn't fight the
 *  player's network for figure-ground. Rebuilt per frame with the carts; empty unless there are carts. */
export function ambientCargoLayer(carts: AmbientTrader[]): Layer {
  return new TextLayer<AmbientTrader>({
    id: "ambient-cargo",
    data: carts,
    getPosition: (d: AmbientTrader) => [d.lng, d.lat],
    getText: (d: AmbientTrader) => d.glyph,
    getSize: 11,
    sizeUnits: "pixels",
    getColor: (d: AmbientTrader) => (d.dim ? [232, 224, 210, 110] : [245, 240, 230, 255]),
    getPixelOffset: [0, -8],
    outlineWidth: 2,
    outlineColor: [24, 20, 16, 220],
    fontSettings: { sdf: true },
    fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",sans-serif',
    characterSet: CARGO_CHARSET,
    getTextAnchor: "middle",
    getAlignmentBaseline: "center",
    pickable: false,
    updateTriggers: { getText: carts.map((c) => c.glyph).join(""), getColor: carts.map((c) => (c.dim ? 1 : 0)).join("") },
  });
}

/** Fantasy 3D diorama (#3d-trees): the forest as instanced lowpoly pines (one SimpleMeshLayer). Stands up
 *  in Z so it reads under the tilted arcadia camera. Per-instance scale/yaw/tint for a varied stand; flat-
 *  shaded under deck's default lighting for the faceted lowpoly look. Static (baked once), so it rides the
 *  cached topo path — never rebuilt per frame. Empty for transit / when zoomed too far out (LOD). */
/** 3D station DEPOTS (#3d-stations): a lowpoly platform + pitched-roof house standing on each of the
 *  player's stations — the fantasy diorama's network nodes made solid (the trains + peeps already animate
 *  at the platform, so the static depot reads as a working berth). Served stations are warm stone, idle
 *  (no line) a cool grey. Fixed diorama scale (≈ a tree); empty for transit / at overview (LOD), a no-op. */
export function stationMeshLayer(stations: StationDot[]): Layer {
  return new SimpleMeshLayer<StationDot>({
    id: "station-depots",
    data: stations,
    mesh: stationMesh(),
    getPosition: (d) => [d.lng, d.lat, d.z ?? 0], // #15 sit on the extruded terrain top
    // #13: a rival depot (faction 1) is crimson stone — the enemy realm's hold reads as hostile in 3D too.
    getColor: (d) => (d.faction === 1 ? [188, 96, 84] : d.serving > 0 ? [224, 212, 190] : [156, 152, 144]),
    getOrientation: [0, 0, 0],
    getScale: [165, 165, 165],
    sizeScale: 1,
    pickable: false,
    material: { ambient: 0.6, diffuse: 0.72, shininess: 18, specularColor: [60, 55, 48] },
    updateTriggers: { getColor: stations.map((d) => `${d.faction ?? 0}:${d.serving > 0 ? 1 : 0}`).join("") },
  });
}

/** #23 TG1 — town SPRAWL: small depot buildings clustered on the ring cells around a town's centre, so a
 *  prosperous town reads as a MULTI-CELL settlement (the capital the biggest) rather than one lone depot.
 *  Reuses the depot mesh at a smaller scale; warm stone with slight per-building variation. Render-only
 *  (cosmetic, #23 TG1), arcadia + LOD-gated like the depots. Reuses TreeInstance's {lng,lat,scale,yaw,shade}. */
export function townSprawlLayer(buildings: TreeInstance[]): Layer {
  return new SimpleMeshLayer<TreeInstance>({
    id: "town-sprawl",
    data: buildings,
    mesh: stationMesh(),
    getPosition: (d) => [d.lng, d.lat, d.z ?? 0], // #15 sit on the extruded terrain top
    getColor: (d) => {
      const v = Math.round(188 + d.shade * 34); // warm stone, varied per building
      return [v, Math.round(v * 0.92), Math.round(v * 0.82)];
    },
    getOrientation: (d) => [0, d.yaw, 0],
    getScale: (d) => [d.scale, d.scale, d.scale],
    sizeScale: 1,
    pickable: false,
    material: { ambient: 0.6, diffuse: 0.72, shininess: 18, specularColor: [60, 55, 48] },
    updateTriggers: { getColor: buildings.length, getScale: buildings.length },
  });
}

export function treeLayer(trees: TreeInstance[]): Layer {
  return new SimpleMeshLayer<TreeInstance>({
    id: "trees",
    data: trees,
    mesh: pineGeometry(),
    getPosition: (d) => [d.lng, d.lat, d.z ?? 0], // #15 sit on the extruded forest hex top
    getColor: (d) => {
      // valley pine (deep green) → highland fir (cooler, lighter); a touch of per-tree variation via shade.
      const g = Math.round(86 + d.shade * 46);
      return [Math.round(34 + d.shade * 24), g, Math.round(40 + d.shade * 18)];
    },
    getOrientation: (d) => [0, d.yaw, 0],
    getScale: (d) => [d.scale, d.scale, d.scale],
    sizeScale: 1,
    pickable: false,
    material: { ambient: 0.55, diffuse: 0.7, shininess: 16, specularColor: [40, 50, 40] },
    updateTriggers: { getColor: trees.length, getScale: trees.length },
  });
}

// 3D-vehicle world scale (metres) — calibrated to the tree/diorama scale (~120-230) so a vehicle reads as a
// chunky little cart on the continent. The mesh is ~1 unit long (X = forward), so this ≈ the body length.
// Default cabin scale (metres ≈ the mesh's ~1-unit body length) for non-grid maps (real OSM track). On a
// GRID/fantasy map the caller derives it from the cell (`cabin = cell_step ÷ 4` ≈ 4 cabins per hex) so
// trains stay proportionate to the lattice + the L2 platform-length constraint (docs/ttd-track-model.md).
const VEHICLE_SCALE_DEFAULT = 150;

/** Heading (CCW radians from +x = east) → deck SimpleMeshLayer orientation so the +X-forward mesh faces its
 *  travel direction. The angle is the path TANGENT (heading_at), updated each tick, so the model turns
 *  through CURVES automatically. Calibrated empirically (Playwright): deck's MIDDLE Euler component is the
 *  vertical yaw for this Z-up mesh, and +degrees aligns the prow with the heading (verified heading == the
 *  actual motion vector). */
function yawOf(angle: number): [number, number, number] {
  return [0, (angle * 180) / Math.PI, 0];
}

/** Packed trains run HOT: above ~0.7 load factor, blend the line colour toward a warm crowding red so a
 *  full train reads at a glance (capacity is one of the two player levers). Subtle below the knee, clear
 *  when jammed. A per-frame accessor — the vehicle data is rebuilt each frame, so this re-evaluates live. */
function crowdTint(rgb: Rgb, load: number): [number, number, number] {
  const t = Math.max(0, Math.min(1, (load - 0.7) / 0.3)) * 0.55;
  return [
    Math.round(rgb[0] + (255 - rgb[0]) * t),
    Math.round(rgb[1] + (96 - rgb[1]) * t),
    Math.round(rgb[2] + (64 - rgb[2]) * t),
  ];
}

export function vehicleLayers(dots: VehicleDot[], cars: CargoCar[] = [], scale: number = VEHICLE_SCALE_DEFAULT): Layer[] {
  // #3d-vehicles + #multi-car: real instanced 3D models on the world. A rail train is a LOCOMOTIVE (the
  // boxy cab, line-coloured) PULLING a string of cargo WAGONS that curve behind it along the track — each
  // wagon a line-tinted flatcar with a commodity-coloured load lump whose HEIGHT reads the load (goods you
  // SEE, not a ring). Bus/ferry/air pull nothing, so THEY carry the cargo block on their own bed (the
  // `vehicle-cargo` layer below, gated to non-train dots). Object-array + per-frame rebuild like the old
  // dots (bounded visible count); the three shared meshes upload once. `scale` (≈ cabin length in metres)
  // is cell-derived on a grid map so a consist reads as ~4 cabins per hex (the sim's car-pitch matches it).
  const bedM = 0.46 * scale; // cargo sits on the cabin top (mesh z 0.46) → raise it this many metres
  const wagonBedM = 0.14 * scale; // a wagon's flatbed top (mesh z 0.14) → the load lump sits here
  const lumpScale = 0.9 * scale; // the load lump is inset to sit between the wagon end-walls
  const bodyCargo = dots.filter((d) => !d.pullsCars && d.load > 0.05); // bus/ferry/air carry their own load
  const lumps = cars.filter((c) => c.load > 0.05);
  return [
    new SimpleMeshLayer<VehicleDot>({
      id: "vehicles",
      data: dots,
      mesh: vehicleMesh(),
      getPosition: (d) => [d.lng, d.lat],
      getColor: (d) => crowdTint(d.color, d.load), // line colour, running hot when packed
      getOrientation: (d) => yawOf(d.angle),
      getScale: [scale, scale, scale],
      sizeScale: 1,
      pickable: true, // id "vehicles" so the train inspector (getTooltip dispatch on layer.id) still fires
      material: { ambient: 0.62, diffuse: 0.72, shininess: 24, specularColor: [70, 70, 80] },
      updateTriggers: { getOrientation: dots.length, getColor: dots.length },
    }),
    // Trailing cargo WAGONS (rail/heavy only): a line-coloured flatcar per car, yawed to its OWN track
    // tangent so the consist curves through bends instead of rigidly following the loco's heading.
    new SimpleMeshLayer<CargoCar>({
      id: "vehicle-wagons",
      data: cars,
      mesh: wagonMesh(),
      getPosition: (d) => [d.lng, d.lat],
      getColor: (d) => d.color,
      getOrientation: (d) => yawOf(d.angle),
      getScale: [scale, scale, scale],
      sizeScale: 1,
      pickable: false,
      material: { ambient: 0.62, diffuse: 0.72, shininess: 24, specularColor: [70, 70, 80] },
      updateTriggers: { getOrientation: cars.length, getColor: cars.length },
    }),
    // The load lump on each WAGON — commodity-coloured, HEIGHT ∝ load, raised onto the flatbed.
    new SimpleMeshLayer<CargoCar>({
      id: "vehicle-wagon-cargo",
      data: lumps,
      mesh: cargoMesh(),
      getPosition: (d) => [d.lng, d.lat],
      getColor: (d) => cargoColor(d.cargo),
      getOrientation: (d) => yawOf(d.angle),
      getScale: (d) => [lumpScale, lumpScale, scale * (0.07 + d.load * 0.5)],
      getTranslation: [0, 0, wagonBedM],
      sizeScale: 1,
      pickable: false,
      material: { ambient: 0.7, diffuse: 0.66, shininess: 12, specularColor: [50, 50, 55] },
      updateTriggers: {
        getScale: lumps.map((d) => Math.round(d.load * 12)).join(","),
        getOrientation: lumps.length,
        getColor: lumps.map((d) => d.cargo).join(","),
      },
    }),
    // Bus/ferry/air carry their cargo block on their OWN bed (no wagons to pull). Same `vehicle-cargo` id
    // the LOD filter already drops at overview.
    new SimpleMeshLayer<VehicleDot>({
      id: "vehicle-cargo",
      data: bodyCargo,
      mesh: cargoMesh(),
      getPosition: (d) => [d.lng, d.lat],
      getColor: (d) => cargoColor(d.cargo),
      getOrientation: (d) => yawOf(d.angle),
      getScale: (d) => [scale, scale, scale * (0.07 + d.load * 0.5)],
      getTranslation: [0, 0, bedM],
      sizeScale: 1,
      pickable: false,
      material: { ambient: 0.7, diffuse: 0.66, shininess: 12, specularColor: [50, 50, 55] },
      updateTriggers: {
        getScale: bodyCargo.map((d) => Math.round(d.load * 12)).join(","),
        getOrientation: bodyCargo.length,
        getColor: bodyCargo.map((d) => d.cargo ?? 255).join(","),
      },
    }),
  ];
}

/** Marching-legion dots (fantasy/arcadia, S8): AI armies riding the rails toward enemy towns. Crimson
 *  with a gold ring so they read distinctly from the line-tinted commodity carts. `positionsLngLat` is
 *  interleaved lng/lat (the caller converts from the sim's metres). Few in number (capped), so a plain
 *  per-compose ScatterplotLayer is cheap — no binary-attribute path needed. */
/** A legion INTENT arc (fantasy/arcadia, S11 — the AI general's "why" made spatial): from a marching
 *  legion to the town it's headed for. The caller filters out zero-length (idle/besieging) arcs. */
export interface IntentArc {
  from: [number, number];
  to: [number, number];
}

/** Legion INTENT arcs: faint crimson curves from each marching legion to its target town — so the player
 *  reads WHERE the AI is sending its legions (you steer by rail + bounty, the legions execute). Under the
 *  legion dots (the dot stays on top of its own line), over the network. Few + short-lived, so a plain
 *  per-compose ArcLayer is cheap (mirrors the army/raider dot layers). */
export function armyIntentLayer(arcs: IntentArc[], alpha = 1): Layer {
  // #war legibility: scale alpha DOWN as the arc count rises so a cluster of same-hue arcs reads as a
  // gradient rather than a solid crimson smear (clutter fix). updateTriggers keyed on the alpha so deck
  // re-evaluates the colour accessors when density changes.
  return new ArcLayer({
    id: "army-intent",
    data: arcs,
    getSourcePosition: (d: IntentArc) => d.from,
    getTargetPosition: (d: IntentArc) => d.to,
    getSourceColor: [150, 24, 24, Math.round(50 * alpha)], // faint at the legion
    getTargetColor: [150, 24, 24, Math.round(165 * alpha)], // stronger at the destination (where the intent points)
    getWidth: 2,
    widthUnits: "pixels",
    widthMinPixels: 1.5,
    getHeight: 0.5,
    updateTriggers: { getSourceColor: alpha, getTargetColor: alpha },
  });
}

/** Rail-attack intent (#war): the RIVAL's targeting made legible — a toxic-green arc from each SMART raider
 *  (a saboteur heading for your rail, a reclaimer heading for an unheld town) to its target, so you see the
 *  smart enemy coming and can rail-to / defend it. Toxic green = the raiders' own rot hue (distinct from the
 *  player's red legion intent). Breachers (capital-bound) are filtered out upstream so the map stays legible. */
export function raiderIntentLayer(arcs: IntentArc[], alpha = 1): Layer {
  // #war legibility: alpha scales down with density (clutter fix) — see armyIntentLayer.
  return new ArcLayer({
    id: "raider-intent",
    data: arcs,
    getSourcePosition: (d: IntentArc) => d.from,
    getTargetPosition: (d: IntentArc) => d.to,
    getSourceColor: [120, 170, 70, Math.round(40 * alpha)], // faint at the raider
    getTargetColor: [150, 200, 90, Math.round(175 * alpha)], // stronger at what it's coming for (your rail / unheld town)
    getWidth: 1.5,
    widthUnits: "pixels",
    widthMinPixels: 1,
    getHeight: 0.3,
    updateTriggers: { getSourceColor: alpha, getTargetColor: alpha },
  });
}

export function armyLayer(positionsLngLat: Float32Array, count: number): Layer {
  return new ScatterplotLayer({
    id: "armies",
    data: { length: count, attributes: { getPosition: { value: positionsLngLat, size: 2 } } },
    getFillColor: [150, 24, 24],
    getLineColor: [255, 214, 110],
    stroked: true,
    lineWidthMinPixels: 1.5,
    getRadius: 5,
    radiusUnits: "pixels",
    radiusMinPixels: 4,
    radiusMaxPixels: 9,
  });
}

/** One legion as an in-world 3D standard (#legion-3d): a crimson banner yawed to its march heading, with a
 *  nameplate. `heading` is the metre-space march direction (atan2(dy,dx)); `besieging` dims it to read as
 *  "arrived, holding" vs an advancing host. */
export interface LegionDot {
  lng: number;
  lat: number;
  heading: number;
  name: string;
  besieging: boolean;
  camped: boolean; // #daynight: holding camp through the night (a foot-march rests at dark) — pitches a campfire
}

const LEGION_SCALE = 150; // diorama scale (a touch larger than a train — an army is a notable force)

/** The legion HOST as a real 3D model (replaces the flat crimson dot): a banner standard yawed to its
 *  march direction. Crimson advancing; muted brick besieging; a banked ember when CAMPED for the night
 *  (so "advancing" vs "holding" vs "resting" all read at a glance). */
export function legionLayer(legions: LegionDot[]): Layer {
  return new SimpleMeshLayer<LegionDot>({
    id: "armies",
    data: legions,
    mesh: legionMesh(),
    getPosition: (d) => [d.lng, d.lat],
    getColor: (d) => (d.camped ? [126, 66, 44] : d.besieging ? [150, 70, 56] : [176, 26, 26]),
    getOrientation: (d) => yawOf(d.heading),
    getScale: [LEGION_SCALE, LEGION_SCALE, LEGION_SCALE],
    sizeScale: 1,
    pickable: false,
    material: { ambient: 0.66, diffuse: 0.7, shininess: 20, specularColor: [80, 60, 60] },
    updateTriggers: { getOrientation: legions.length, getColor: legions.map((d) => (d.camped ? 2 : d.besieging ? 1 : 0)).join("") },
  });
}

/** Campfires under the CAMPED legions (#daynight): a warm ember glow so a foot-march resting through the
 *  night reads as a lit camp, not a stalled dot. Drawn UNDER the standards; empty (zero cost) by day or
 *  with no camped host. Pixel-radius so it reads at any zoom; depthTest off to sit on the dark ground. */
export function legionCampfireLayer(camped: LegionDot[]): Layer {
  return new ScatterplotLayer<LegionDot>({
    id: "legion-campfires",
    data: camped,
    getPosition: (d) => [d.lng, d.lat],
    getRadius: 7,
    radiusUnits: "pixels",
    radiusMinPixels: 5,
    radiusMaxPixels: 11,
    getFillColor: [255, 154, 62, 150],
    stroked: false,
    parameters: { depthTest: false },
    updateTriggers: { getFillColor: camped.length },
  });
}

/** Legion NAMEPLATES (#legion-3d): the host's name floating above its standard, so a player can name + track
 *  their AI armies. A deck TextLayer with a dark plate for legibility over the busy map; pixel-offset up so
 *  it clears the 3D banner; LOD-gated by the caller (drops at the strategic overview). */
export function legionNameLayer(legions: LegionDot[]): Layer {
  return new TextLayer<LegionDot>({
    id: "legion-names",
    data: legions,
    getPosition: (d) => [d.lng, d.lat],
    getText: (d) => `⚔ ${d.name}`,
    getSize: 11,
    sizeUnits: "pixels",
    getColor: [255, 236, 210, 245],
    getPixelOffset: [0, -26],
    background: true,
    getBackgroundColor: [28, 18, 18, 200],
    backgroundPadding: [4, 2],
    fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",system-ui,sans-serif',
    characterSet: ENTITY_CHARSET + "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789·'",
    getTextAnchor: "middle",
    getAlignmentBaseline: "center",
    updateTriggers: { getText: legions.map((d) => d.name).join(",") },
  });
}

/** Decadence-RAIDER dots (fantasy/arcadia, S11 — the rival): enemy marauders marching the capital off-rail.
 *  A sickly decay-green with a dark ring so they read as a THREAT, distinct from the crimson legions and the
 *  line-tinted carts. `positionsLngLat` interleaved lng/lat (caller converts from metres). Capped + few, so
 *  a plain per-compose ScatterplotLayer is cheap. Drawn above legions (the incoming danger sits on top). */
export function raiderLayer(positionsLngLat: Float32Array, count: number): Layer {
  return new ScatterplotLayer({
    id: "raiders",
    data: { length: count, attributes: { getPosition: { value: positionsLngLat, size: 2 } } },
    getFillColor: [120, 170, 70],
    getLineColor: [30, 40, 24],
    stroked: true,
    lineWidthMinPixels: 1.5,
    getRadius: 5,
    radiusUnits: "pixels",
    radiusMinPixels: 4,
    radiusMaxPixels: 9,
  });
}

/** #13 P1d — the RIVAL's mustered HOSTS (the symmetric AI's legions): crimson dots marching overland at your
 *  captured towns. Crimson = the rival realm's hue (matches its hold), DISTINCT from the rot's toxic-green
 *  raiders — so the player reads "the RIVAL is on the move", not "the rot is seeping". */
export function rivalHostLayer(positionsLngLat: Float32Array, count: number): Layer {
  return new ScatterplotLayer({
    id: "rival-hosts",
    data: { length: count, attributes: { getPosition: { value: positionsLngLat, size: 2 } } },
    getFillColor: [190, 55, 55],
    getLineColor: [255, 220, 150],
    stroked: true,
    lineWidthMinPixels: 1.5,
    getRadius: 6,
    radiusUnits: "pixels",
    radiusMinPixels: 5,
    radiusMaxPixels: 11,
  });
}

/** #13 — the RIVAL's INTENT (the telegraph, the owner's #1 pillar): a crimson arc from each marching host to
 *  the captured town it's coming to re-contest, so you SEE the threat forming and can rail-to / defend it
 *  BEFORE it lands. Crimson (the rival's hue), distinct from the raiders' toxic-green intent. Alpha scales
 *  down with density (clutter fix), like the army/raider arcs. */
export function rivalIntentLayer(arcs: IntentArc[], alpha = 1): Layer {
  return new ArcLayer({
    id: "rival-intent",
    data: arcs,
    getSourcePosition: (d: IntentArc) => d.from,
    getTargetPosition: (d: IntentArc) => d.to,
    getSourceColor: [150, 40, 40, Math.round(45 * alpha)], // faint at the host
    getTargetColor: [210, 60, 60, Math.round(185 * alpha)], // stronger at the town it's coming for
    getWidth: 2,
    widthUnits: "pixels",
    widthMinPixels: 1.5,
    getHeight: 0.35,
    updateTriggers: { getSourceColor: alpha, getTargetColor: alpha },
  });
}

/** #13 P2 telegraph — the rival's EXPANSION GHOST: a faint, provisional crimson spur from its rail-head
 *  toward your capital, showing where its track is creeping NEXT (the build intent, ANNOUNCED — the owner's
 *  pillar). Low-alpha + thin so it reads as planned-not-laid, distinct from the solid committed rail. */
export function rivalBuildGhostLayer(from: [number, number], to: [number, number]): Layer {
  return new PathLayer({
    id: "rival-build-ghost",
    data: [{ from, to }],
    getPath: (d: { from: [number, number]; to: [number, number] }) => [d.from, d.to],
    getColor: [205, 70, 70, 120], // faint crimson — provisional
    getWidth: 2.5,
    widthUnits: "pixels",
    widthMinPixels: 2,
    capRounded: true,
    jointRounded: true,
    updateTriggers: { getPath: [from, to] },
  });
}

/** Entity BADGE glyphs (fantasy #10): a small symbol pinned on each moving unit so legions (⚔) + raider
 *  marauders (☣) read as what they are at a glance, not just coloured dots. `positions` are lng/lat (tiny
 *  counts → a plain array). characterSet seeds the atlas. Trains already carry their load ring; peeps stay
 *  plain dots (cosmetic). */
const ENTITY_CHARSET = "⚔☣✂⚑";
export function entityBadgeLayer(
  id: string,
  positions: [number, number][],
  glyph: string,
  color: [number, number, number, number],
): Layer {
  return new TextLayer({
    id,
    data: positions,
    getPosition: (d: [number, number]) => d,
    getText: () => glyph,
    getSize: 11,
    sizeUnits: "pixels",
    getColor: color,
    fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",sans-serif',
    characterSet: ENTITY_CHARSET,
    getTextAnchor: "middle",
    getAlignmentBaseline: "center",
  });
}

/** Spell FLASH bursts (fantasy/arcadia, S11 — the spell arm): a brief coloured pop at each auto-cast site.
 *  `data` is interleaved [lng,lat,kind,alpha,...] (caller converts metres→lng/lat). kind picks the hue
 *  (0 Purge teal · 1 Smite gold · 2 Warpath crimson); alpha fades it out. Few + brief, so a plain
 *  per-compose ScatterplotLayer with accessors is cheap. Drawn on top (the magic reads over everything). */
// FX-burst hues by kind — spells (0-2) + render-only #war event bursts (3-5) that echo AI actions which
// otherwise vanished silently. All ride the one spell-flash buffer/layer (a brief growing-fading pop).
const SPELL_HUE: [number, number, number][] = [
  [68, 170, 153], // 0 Purge — teal (matches the tide-purge theme)
  [240, 200, 70], // 1 Smite — gold bolt
  [200, 60, 60], // 2 Warpath — crimson
  [150, 230, 140], // 3 KILL — clean bright green: the rail cordon cut a raider down (network defended)
  [235, 60, 50], // 4 BREACH — alarm red: a raider struck the capital (the lose-driver)
  [230, 150, 70], // 5 LAUNCH — warm orange: a legion mustered from a barracks
];
// Per-kind base radius (px) — a BREACH reads as a bigger alarm than a routine kill/launch pop.
const FX_BURST_SIZE: number[] = [8, 8, 8, 9, 16, 11];
export function spellFlashLayer(flashes: { lng: number; lat: number; kind: number; alpha: number }[]): Layer {
  return new ScatterplotLayer({
    id: "spell-flashes",
    data: flashes,
    getPosition: (d: { lng: number; lat: number }) => [d.lng, d.lat],
    getFillColor: (d: { kind: number; alpha: number }) => {
      const [r, g, b] = SPELL_HUE[d.kind] ?? [255, 255, 255];
      return [r, g, b, Math.round(200 * d.alpha)];
    },
    // Grow as it fades (a pop), pixel radius; base size per kind (a breach alarm is bigger than a kill pop).
    getRadius: (d: { kind: number; alpha: number }) => (FX_BURST_SIZE[d.kind] ?? 8) + (1 - d.alpha) * 18,
    radiusUnits: "pixels",
    radiusMinPixels: 6,
    stroked: false,
    updateTriggers: { getFillColor: flashes, getRadius: flashes },
  });
}

/** Individual rider "peeps" via deck BINARY attributes (data.attributes) — NO per-object accessors,
 *  so it scales to the core's MAX_VISIBLE_PEEPS at 60fps where an object-array layer would cliff.
 *  `positionsLngLat` is interleaved [lng,lat,...] (f32) and `colors` interleaved RGBA (u8); both are
 *  fresh typed arrays each frame, so deck re-uploads (no stale-reference reuse). Small + faint + NOT
 *  pickable — the train/station inspectors stay the pick targets; peeps are spatial texture. */
export function peepLayer(positionsLngLat: Float32Array, colors: Uint8Array, count: number): Layer {
  return new ScatterplotLayer({
    id: "peeps",
    data: {
      length: count,
      attributes: {
        getPosition: { value: positionsLngLat, size: 2 },
        getFillColor: { value: colors, size: 4, normalized: true }, // u8 0..255 → deck normalizes
      },
    },
    getRadius: 2,
    radiusUnits: "pixels",
    radiusMinPixels: 1.4,
    radiusMaxPixels: 4.5,
    stroked: false,
  });
}

/** TTD-style SIGNAL markers on single-track blocks: green (clear) / red (occupied) / amber (a cart held
 *  here, waiting for the block ahead). Surfaces the otherwise-invisible meet so the player sees WHY a cart
 *  waits. Small stroked dots UNDER the vehicles (a train rides on top of the signal that gates it). Not
 *  pickable; rebuilt per frame like the other motion layers (occupancy shifts with the trains). */
export function signalLayer(signals: SignalMarker[]): Layer {
  const color = (a: number): [number, number, number, number] =>
    a === 1 ? [214, 40, 40, 235] : a === 2 ? [230, 159, 0, 240] : [0, 158, 115, 205];
  return new ScatterplotLayer<SignalMarker>({
    id: "signals",
    data: signals,
    getPosition: (d: SignalMarker) => [d.lng, d.lat],
    getFillColor: (d: SignalMarker) => color(d.aspect),
    getRadius: 5,
    radiusUnits: "pixels",
    radiusMinPixels: 3.5,
    radiusMaxPixels: 8,
    stroked: true,
    getLineColor: [245, 245, 245, 230],
    getLineWidth: 1,
    lineWidthUnits: "pixels",
    lineWidthMinPixels: 1,
    pickable: false,
    updateTriggers: { getFillColor: signals.map((s) => s.aspect).join("") },
  });
}

/** TTD L5c — the PLAYER-PLACED block signals (the posts dropped on single-track spans) + the pre-commit
 *  place ghost. Drawn DISTINCT from the amber/red/green occupancy aspect dots (`signalLayer`): an upright
 *  white post with a dark casing (a "signal mast"), reading as infrastructure the player owns rather than a
 *  live aspect. The snap candidate (the post the next click would remove) gets a bright selection-blue ring;
 *  the place ghost is a translucent post. Above the line PathLayer, below the vehicles (composeAndSet z-order).
 *  Not deck-pickable — hit-tested in screen space by Game, like the waypoint control handles. */
export function placedSignalLayers(placed: PlacedSignalMarker[], ghost: SignalGhost | null): Layer[] {
  const out: Layer[] = [];
  if (placed.length > 0) {
    out.push(
      new ScatterplotLayer<PlacedSignalMarker>({
        id: "placed-signals",
        data: placed,
        getPosition: (d: PlacedSignalMarker) => [d.lng, d.lat],
        // White post, selection-blue when it's the snap (remove) candidate.
        getFillColor: (d: PlacedSignalMarker) => (d.snap ? [0, 114, 178, 255] : [245, 247, 250, 245]),
        getLineColor: (d: PlacedSignalMarker) => (d.snap ? [255, 255, 255, 255] : [28, 32, 40, 235]),
        getRadius: 6,
        radiusUnits: "pixels",
        radiusMinPixels: 4.5,
        radiusMaxPixels: 9,
        stroked: true,
        getLineWidth: 2,
        lineWidthUnits: "pixels",
        lineWidthMinPixels: 1.5,
        pickable: false,
        updateTriggers: { getFillColor: placed.map((s) => (s.snap ? 1 : 0)).join(""), getLineColor: placed.map((s) => (s.snap ? 1 : 0)).join("") },
      }),
    );
  }
  if (ghost) {
    out.push(
      new ScatterplotLayer<SignalGhost>({
        id: "signal-ghost",
        data: [ghost],
        getPosition: (d: SignalGhost) => [d.lng, d.lat],
        getFillColor: [0, 114, 178, 110], // translucent provisional post (dashed-blueprint cousin)
        getLineColor: [0, 114, 178, 220],
        getRadius: 6,
        radiusUnits: "pixels",
        radiusMinPixels: 4.5,
        stroked: true,
        getLineWidth: 1.5,
        lineWidthUnits: "pixels",
        lineWidthMinPixels: 1.5,
        pickable: false,
      }),
    );
  }
  return out;
}

interface GlowPoint {
  position: [number, number];
  cap: boolean;
}

/** Warm NIGHT-LIGHT glows at the settled places (capital + towns + resource camps), fading in with the
 *  0..1 `night` factor — lit windows / hearth-fires against the cool dark. Three stacked PIXEL-radius discs
 *  per point (a wide bloom + a mid ring + a bright hot-core). Arcadia only; the caller skips it by day
 *  (night≈0) and at the strategic overview. Bounded (~50 points) — cheap, `depthTest:false` so the glow
 *  reads over the tilted terrain. Rides the per-frame compose like the other motion layers. */
export function nightGlowLayers(towns: TownMarker[], resources: ResourceMarker[], night: number): Layer[] {
  if (night <= 0.02) return [];
  const pts: GlowPoint[] = [
    ...towns.map((t) => ({ position: [t.lng, t.lat] as [number, number], cap: t.kind === "capital" })),
    ...resources.map((r) => ({ position: [r.lng, r.lat] as [number, number], cap: false })),
  ];
  if (pts.length === 0) return [];
  const trig = Math.round(night * 20); // quantize so the updateTrigger only bumps on a real change
  // Pixel radii (not metres) so a glow reads as a compact LIGHT halo at any zoom, never a terrain-wide
  // wash. Three stacked translucent discs fake a soft radial falloff: a faint wide bloom, a mid ring,
  // a bright hot core. Bounded; depthTest:false so they read over the tilted terrain.
  const disc = (id: string, capPx: number, px: number, rgb: [number, number, number], capA: number, a: number) =>
    new ScatterplotLayer<GlowPoint>({
      id,
      data: pts,
      getPosition: (d: GlowPoint) => d.position,
      radiusUnits: "pixels",
      getRadius: (d: GlowPoint) => (d.cap ? capPx : px),
      radiusMinPixels: 1.5,
      getFillColor: (d: GlowPoint) => [rgb[0], rgb[1], rgb[2], Math.round((d.cap ? capA : a) * night)],
      stroked: false,
      pickable: false,
      parameters: { depthTest: false },
      updateTriggers: { getFillColor: trig, getRadius: 0 },
    });
  return [
    disc("night-bloom", 46, 30, [255, 188, 92], 30, 22), // faint wide bloom
    disc("night-mid", 22, 14, [255, 206, 120], 46, 36), // warmer mid
    disc("night-core", 5.5, 3.6, [255, 236, 184], 235, 230), // bright hot core
  ];
}

/** Warm headlamp glow under each train at night (fades in with `night`), so a running line reads as
 *  lit carriages crossing the dark. A single soft pixel-radius disc beneath the loco mesh — bounded by
 *  the vehicle count, rides the per-frame compose like the vehicle layers themselves. */
export function vehicleNightGlow(vehicles: VehicleDot[], night: number): Layer[] {
  if (night <= 0.02 || vehicles.length === 0) return [];
  return [
    new ScatterplotLayer<VehicleDot>({
      id: "vehicle-night-glow",
      data: vehicles,
      getPosition: (d: VehicleDot) => [d.lng, d.lat],
      radiusUnits: "pixels",
      getRadius: 11,
      radiusMinPixels: 4,
      getFillColor: [255, 214, 140, Math.round(72 * night)],
      stroked: false,
      pickable: false,
      parameters: { depthTest: false },
      updateTriggers: { getFillColor: Math.round(night * 20) },
    }),
  ];
}
