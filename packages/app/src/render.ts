// Builds the deck.gl overlay layers from a plain RenderView (already in lng/lat — all mm
// conversion happened at the geo.ts boundary in Game). Layer array order IS the z-order
// (AGENTS IA): catchment < lines < blueprint < stations < vehicles < selection highlight.
import type { Layer } from "@deck.gl/core";
import { ArcLayer, ColumnLayer, IconLayer, PathLayer, ScatterplotLayer, TextLayer } from "@deck.gl/layers";
import { SimpleMeshLayer } from "@deck.gl/mesh-layers";
import { BUSY_WAITING, STARVED_WAITING } from "./config";
import { pineGeometry } from "./render/treeMesh";
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
}
export interface LinePath {
  id: number;
  color: Rgb;
  path: [number, number][];
  mode: number; // transport mode (trainset::tmode); 4 = heavy/high-speed rail (distinct styling)
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
  /** Load factor (onboard / capacity), 0..~1 — drives the crowding ring colour + train size. */
  load: number;
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
/** A node BUFFER-FILL pip (fantasy #8): `fill` 0..1 of the node's Forge-Line buffer — a backed-up source
 *  (ship it) reads high, a starved sink reads low. Rendered as a small gauge dot on the node. */
export interface BufferPip {
  lng: number;
  lat: number;
  fill: number;
}
/** Fantasy (arcadia) #9: one AREA-OF-INFLUENCE disc — a holding (the capital or a captured town) and
 *  the build reach around it (metres). Their union is the realm border; you may only lay rail inside it. */
export interface InfluenceDisc {
  lng: number;
  lat: number;
  radiusM: number;
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
  // decadence floor rises. Off the blue family + warm lines (no collision with selection-blue/the empire);
  // ≥80 value units clean→decayed so corruption still darkens legibly.
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
  switch (kind) {
    case "ore": return [38, 150, 168];    // steel-cyan iron — kept OFF the load-bearing selection-blue #0072b2
    case "grain": return [230, 159, 0];   // wheat gold
    case "fuel": return [0, 158, 115];    // forest green
    case "aether": return [148, 96, 210]; // arcane violet
    case "forge": return [120, 124, 130];  // steel grey — a PROCESSOR (ore → INGOT), S7e multi-stage
    default: return [200, 200, 200];
  }
}

/** Distinctive node ICONS (trial feedback #4/#13): a glyph per resource / town kind so the player reads
 *  WHAT each node is at a glance, not just its colour. BMP symbols (wide font coverage); the explicit
 *  NODE_CHARSET below seeds deck's TextLayer atlas so they render. Drawn OVER the coloured dots, which stay
 *  as the colour-blind-safe channel. */
function resourceGlyph(kind: string): string {
  switch (kind) {
    case "ore": return "⛏";
    case "grain": return "✿";
    case "fuel": return "♣";
    case "aether": return "✦";
    case "forge": return "⚒";
    default: return "◆";
  }
}
function townGlyph(kind: string): string {
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
    case 4: return [34, 40, 52, 255];     // WATER — near-neutral cold sea (slightly darker/deader than land)
    case 10: return [128, 128, 124, 255]; // PLAIN — pale ash (buildable lowland)
    case 8: return [96, 100, 96, 255];    // FOREST — ~30 value below plain (fuel country), barely cooler
    case 7: return [96, 94, 92, 255];     // HILL — mid grey (rising ground)
    case 6: return [40, 38, 40, 255];     // MOUNTAIN — near-black ridge (impassable)
    case 9: return [120, 96, 156, 255];   // LEY — faint violet (the arcane: the ONLY ground chroma)
    default: return [70, 70, 70, 255];
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
  influence: InfluenceDisc[]; // fantasy #9: realm-border discs (capital + captured towns) — empty for transit
  hazards: HazardDot[]; // live built/water conflict dots along the blueprint (G2)
  demand: DemandPoint[]; // travel-demand heat overlay (toggleable map layer)
  roads: RoadCell[]; // ROAD-class corridors (where buses are cheap+fast) — toggle/auto in bus mode
  roadHour: number; // current in-game hour → recolours the roads overlay by live congestion
  demandCellM: number; // demand-grid cell pitch (m) → sizes the demand hexagons to tile the grid
  roadCellM: number; // buildability cell pitch (m) → sizes the road hexagons to tile the grid
  terrain: TerrainCell[]; // baked fantasy terrain hexes (the map itself) — empty for transit cities
  terrainCellM: number; // fantasy hex size (m, = gridCellMm/1000) → the hexagon circumradius
  trees: TreeInstance[]; // fantasy 3D diorama: lowpoly pines on forest hexes (empty for transit / at overview)
  tideCells: TideCell[]; // fantasy S10c: corrupted decadence-CA hexes (the cold creep) — empty for transit
  tidePulse?: number; // fantasy: tide-frontier ring alpha (150..230), advanced on the ~3 Hz recompose (NOT per frame)
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
  if (served) return cold ? [120, 124, 136, Math.round(8 + t * 22)] : [90, 130, 170, Math.round(10 + t * 26)];
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
  return { color: [230, 159, 0, 130], width: 1.5 }; // a few waiting — faint amber
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
      extruded: false, // flat fill on the ground
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
      updateTriggers: { getFillColor: view.terrain.length },
    }),
    // FANTASY 3D DIORAMA (#3d-trees): lowpoly pines standing up on the forest hexes, right on the terrain
    // (under the network/POIs). Empty for transit / at overview (LOD), so it's a no-op there.
    treeLayer(view.trees),
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
    // AREA OF INFLUENCE (#9): the realm border — a faint warm wash + gold ring per HOLDING (the capital +
    // each captured town), radius = the build reach in METRES (a true spatial fact, so it scales with zoom
    // like the catchment). Their union is where rail may go; conquest grows it outward. Drawn over the
    // terrain/water but UNDER the POIs + network so it never hides a clickable node. The low fill alpha
    // keeps overlaps from muddying (a slightly warmer heartland reads as a feature). Empty for transit.
    new ScatterplotLayer({
      id: "influence",
      data: view.influence,
      getPosition: (d: InfluenceDisc) => [d.lng, d.lat],
      getRadius: (d: InfluenceDisc) => d.radiusM,
      radiusUnits: "meters",
      getFillColor: [208, 162, 72, 18], // faint warm gold — the empire's reach (warmth vs the cold frontier)
      getLineColor: [232, 192, 102, 150], // a soft gold ring marks the buildable border (a load-bearing affordance)
      stroked: true,
      filled: true,
      lineWidthUnits: "pixels",
      getLineWidth: 2,
      lineWidthMinPixels: 1.5,
      updateTriggers: { getRadius: view.influence.length, getFillColor: view.influence.length },
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
    // ✦ aether / ⚒ forge). White for contrast on the dot. characterSet seeds the atlas so symbols render.
    new TextLayer({
      id: "resource-icons",
      data: view.resources,
      getPosition: (d: ResourceMarker) => [d.lng, d.lat],
      getText: (d: ResourceMarker) => resourceGlyph(d.kind),
      getSize: 13,
      sizeUnits: "pixels",
      getColor: [250, 250, 250, 240],
      fontFamily: '"Segoe UI Symbol","Noto Sans Symbols2","Apple Symbols","DejaVu Sans",sans-serif',
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
      radius: hexRadius(view.roadCellM), // the shed cells ARE buildability cells → road-grid pitch
      radiusUnits: "meters",
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
      data: view.lines.filter((d) => d.mode === HEAVY_RAIL),
      getPath: (d: LinePath) => d.path,
      getColor: [34, 34, 40, 255],
      getWidth: 13,
      widthUnits: "pixels",
      widthMinPixels: 9,
      capRounded: true,
      jointRounded: true,
    }),
    new PathLayer({
      id: "lines",
      data: view.lines,
      getPath: (d: LinePath) => d.path,
      getColor: (d: LinePath) => d.color,
      getWidth: (d: LinePath) => (d.mode === HEAVY_RAIL ? 9 : 7),
      widthUnits: "pixels",
      // The network is the FIGURE — keep the coloured ribbon wider than the station/vehicle dots
      // (~4px) so it reads as a continuous line, not a string of beads under the dot field.
      widthMinPixels: 5,
      capRounded: true,
      jointRounded: true,
      // Pickable so hovering the track raises the line inspector (under stations + trains in
      // z-order, so it only fires on bare track). The pick hit-area widens with pickingRadius.
      pickable: true,
    }),
    new PathLayer({
      id: "lines-heavy-centre",
      data: view.lines.filter((d) => d.mode === HEAVY_RAIL),
      getPath: (d: LinePath) => d.path,
      getColor: [245, 245, 250, 220],
      getWidth: 2,
      widthUnits: "pixels",
      widthMinPixels: 1,
      capRounded: true,
      jointRounded: true,
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
      getRadius: (d: StationDot) => (d.selected ? 7 : 4) + Math.min(4, Math.sqrt(d.boardings) * 0.35),
      radiusUnits: "pixels",
      radiusMinPixels: 3,
      // Selected fill = selection blue (ties to its blue catchment ring). Otherwise an ORPHANED
      // station (no operational line serving it) is muted grey and a SERVED one is near-black, so
      // stations visibly "light up" as you connect + run them (place→draw→assign cause→effect).
      getFillColor: (d: StationDot) =>
        d.selected ? [0, 114, 178] : d.serving > 0 ? [28, 32, 36] : [120, 126, 134],
      stroked: true,
      getLineColor: [255, 255, 255, 230],
      lineWidthMinPixels: 1,
      pickable: true,
      updateTriggers: {
        getFillColor: view.stations.map((s) => `${s.selected}:${s.serving > 0}`).join(","),
        getRadius: view.stations.map((s) => `${s.selected}:${Math.round(Math.sqrt(s.boardings))}`).join(","),
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
    // Bounty markers (fantasy): a gold ring around each town the player has posted a bounty on — the
    // steering lever's visual feedback (you SEE where you've baited the legions). Font-independent (a
    // ring, not a glyph). Few in number; rebuilt on refresh (bounties change via a Command), not per frame.
    new ScatterplotLayer({
      id: "bounty-markers",
      data: view.stations.filter((s) => (s.bounty ?? 0) > 0),
      getPosition: (d: StationDot) => [d.lng, d.lat],
      getRadius: 11,
      radiusUnits: "pixels",
      radiusMinPixels: 9,
      stroked: true,
      filled: false,
      getLineColor: [214, 158, 0, 255], // gold bounty halo
      lineWidthMinPixels: 2,
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
      getRadius: (d: WaitingDot) => 5 + Math.min(7, Math.sqrt(d.count) * 1.5),
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
      getRadius: (d: WaitingDot) => 5 + Math.min(7, Math.sqrt(d.count) * 1.5),
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

  return { below, above };
}

// A white triangle pointing +x (east) at angle 0, baked once to a data URL so the IconLayer can
// rotate it by heading. `mask:true` makes it a tintable stencil so getColor carries line identity.
function arrowIconUrl(): string {
  const s = 64;
  const c = document.createElement("canvas");
  c.width = s;
  c.height = s;
  const g = c.getContext("2d")!;
  g.fillStyle = "#fff";
  g.beginPath();
  g.moveTo(58, 32); // tip (east)
  g.lineTo(14, 13);
  g.lineTo(26, 32);
  g.lineTo(14, 51);
  g.closePath();
  g.fill();
  return c.toDataURL();
}
const ARROW_ICON = typeof document !== "undefined" ? arrowIconUrl() : "";
const ARROW_MAPPING = { arrow: { x: 0, y: 0, width: 64, height: 64, mask: true, anchorX: 32, anchorY: 32 } };

/** Crowding band for a moving train, mirroring loadPip / the waiting-ring language so "busy" and
 *  "crush" read the same colour wherever they appear. Outline only — the body keeps line identity. */
function loadRing(load: number): { color: [number, number, number, number]; width: number } {
  if (load >= 0.9) return { color: [214, 40, 40, 255], width: 4 }; // crush — thick vermillion (unmistakable)
  if (load >= 0.6) return { color: [230, 159, 0, 245], width: 3 }; // busy — amber
  return { color: [255, 255, 255, 230], width: 1.5 }; // healthy — thin white
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
export function treeLayer(trees: TreeInstance[]): Layer {
  return new SimpleMeshLayer<TreeInstance>({
    id: "trees",
    data: trees,
    mesh: pineGeometry(),
    getPosition: (d) => [d.lng, d.lat],
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

export function vehicleLayers(dots: VehicleDot[]): Layer[] {
  return [
    // Body: the crowding-aware dot. Line-colour fill = identity; radius + outline colour/width
    // track load (white healthy → amber busy → red crush, the loadPip/waiting-ring language).
    // Pickable, id "vehicles" so the train inspector (getTooltip dispatch on layer.id) still fires.
    new ScatterplotLayer({
      id: "vehicles",
      data: dots,
      getPosition: (d: VehicleDot) => [d.lng, d.lat],
      // Load reads as SIZE — an empty train is a small dot, a crush-loaded one a big fat blob (the "more
      // cars = more load" cue, without per-car path geometry). The thicker white→amber→red ring reinforces it.
      getRadius: (d: VehicleDot) => 5 + d.load * 11,
      radiusUnits: "pixels",
      radiusMinPixels: 5,
      radiusMaxPixels: 18,
      getFillColor: (d: VehicleDot) => d.color,
      stroked: true,
      getLineColor: (d: VehicleDot) => loadRing(d.load).color,
      getLineWidth: (d: VehicleDot) => loadRing(d.load).width,
      lineWidthUnits: "pixels",
      lineWidthMinPixels: 1.5,
      pickable: true,
      updateTriggers: {
        getRadius: dots.map((d) => Math.round(d.load * 10)).join(","),
        getLineColor: dots.map((d) => loadRing(d.load).width).join(","),
        getLineWidth: dots.map((d) => loadRing(d.load).width).join(","),
      },
    }),
    // Direction: a small WHITE triangle rotated to the train's heading (deck getAngle is CCW
    // degrees; our heading is CCW radians from +x → straight conversion). White so it reads on the
    // line-coloured body; smaller than the dot so the identity colour still rings it. Not pickable.
    new IconLayer({
      id: "vehicle-dir",
      data: dots,
      getPosition: (d: VehicleDot) => [d.lng, d.lat],
      getIcon: () => "arrow",
      iconAtlas: ARROW_ICON,
      iconMapping: ARROW_MAPPING,
      getColor: [255, 255, 255, 235],
      getAngle: (d: VehicleDot) => (d.angle * 180) / Math.PI,
      getSize: (d: VehicleDot) => 9 + d.load * 3,
      sizeUnits: "pixels",
      sizeMinPixels: 7,
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
export function armyIntentLayer(arcs: IntentArc[]): Layer {
  return new ArcLayer({
    id: "army-intent",
    data: arcs,
    getSourcePosition: (d: IntentArc) => d.from,
    getTargetPosition: (d: IntentArc) => d.to,
    getSourceColor: [150, 24, 24, 50], // faint at the legion
    getTargetColor: [150, 24, 24, 165], // stronger at the destination (where the intent points)
    getWidth: 2,
    widthUnits: "pixels",
    widthMinPixels: 1.5,
    getHeight: 0.5,
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

/** Entity BADGE glyphs (fantasy #10): a small symbol pinned on each moving unit so legions (⚔) + raider
 *  marauders (☣) read as what they are at a glance, not just coloured dots. `positions` are lng/lat (tiny
 *  counts → a plain array). characterSet seeds the atlas. Trains already carry their load ring; peeps stay
 *  plain dots (cosmetic). */
const ENTITY_CHARSET = "⚔☣";
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
const SPELL_HUE: [number, number, number][] = [
  [68, 170, 153], // Purge — teal (matches the tide-purge theme)
  [240, 200, 70], // Smite — gold bolt
  [200, 60, 60], // Warpath — crimson
];
export function spellFlashLayer(flashes: { lng: number; lat: number; kind: number; alpha: number }[]): Layer {
  return new ScatterplotLayer({
    id: "spell-flashes",
    data: flashes,
    getPosition: (d: { lng: number; lat: number }) => [d.lng, d.lat],
    getFillColor: (d: { kind: number; alpha: number }) => {
      const [r, g, b] = SPELL_HUE[d.kind] ?? [255, 255, 255];
      return [r, g, b, Math.round(200 * d.alpha)];
    },
    // Grow as it fades (a pop), pixel radius.
    getRadius: (d: { alpha: number }) => 8 + (1 - d.alpha) * 18,
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
