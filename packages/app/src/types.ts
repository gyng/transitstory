// TS mirror of the Rust serde shapes (AGENTS code-org: keep in sync with crates/sim in the
// same commit). Commands/Events are externally-tagged ({Variant:{...}}); Command fields are
// snake_case (no rename in Rust); Stats/views are camelCase (rename_all="camelCase").

export type Command =
  | { PlaceStation: { x_mm: number; y_mm: number; name: string | null } }
  | { CreateLine: { color: number; name?: string | null; loop_line?: boolean; mode?: number } }
  | { AddStop: { line: number; station: number; after: number | null } }
  | { AssignTrainset: { line: number; spec: number; count: number } }
  | { SetHeadway: { line: number; headway_ms: number } }
  | { SetSegmentMode: { line: number; span: number; mode: number } }
  | { SetRunning: { running: boolean } }
  | { SetEconomy: { enabled: boolean } }
  | { RemoveStation: { station: number } }
  | { RemoveLine: { line: number } }
  // waypoints: per-span control points that bend the track; each span is a list of [x_mm, y_mm].
  | { SetLineWaypoints: { line: number; waypoints: [number, number][][] } };

export type Event =
  | { StationPlaced: { id: number; name: string } }
  | { LineCreated: { id: number } }
  | { StopAdded: { line: number; station: number } }
  | { TrainsetAssigned: { line: number; count: number } }
  | { HeadwaySet: { line: number; headway_ms: number } }
  | { SegmentModeSet: { line: number; span: number; mode: number } }
  | { RunningSet: { running: boolean } }
  | { EconomySet: { enabled: boolean } }
  | { StationRemoved: { station: number } }
  | { LineRemoved: { line: number } }
  | { WaypointsSet: { line: number } }
  | { Rejected: { reason: string } };

export interface PerStation {
  stationId: number;
  boardings: number;
  alightings: number;
  waiting: number;
  /** Captured gravity demand pulled from the grid: resident/origin weight + job/dest weight. */
  demandOrigin: number;
  demandDest: number;
  /** Operational lines serving this station (trainset + ≥2 stops). 0 = no service ("orphaned"). */
  serving: number;
  /** Cumulative pressure here: riders passed by a full train + riders who gave up waiting. */
  denied: number;
  abandoned: number;
}

export interface PerLine {
  lineId: number;
  name: string;
  mode: number;
  color: number;
  ridership: number;
  stops: number;
  trains: number;
  headwayMs: number;
  disruption: number;
  crossesWater: boolean;
  capitalCost: number;
  /** Mean load factor (onboard/capacity) across this line's vehicles; 0 with no vehicles. */
  loadFactor: number;
}

export interface Stats {
  simClockMs: number;
  running: boolean;
  stationCount: number;
  lineCount: number;
  vehicleCount: number;
  ridershipTotal: number;
  waitingTotal: number;
  /** Cumulative "left behind" = times a rider was passed by a full vehicle (== deniedBoardings). */
  leftBehind: number;
  deniedBoardings: number;
  /** Cumulative riders who gave up waiting (renege) — frequency/coverage pressure. */
  abandoned: number;
  /** Average end-to-end trip time (ms) over completed trips; 0 before the first arrival. */
  avgJourneyMs: number;
  /** Average platform wait (ms) per boarding; 0 before the first boarding. */
  avgWaitMs: number;
  avgLoadFactor: number;
  coverageScore: number;
  simHour: number;
  period: string;
  demandMultiplier: number;
  buildDifficulty: number;
  economyEnabled: boolean;
  balance: number;
  capitalSpent: number;
  fareRevenue: number;
  /** Cumulative recurring maintenance charged (opex); 0 unless the economy is enabled. */
  opexSpent: number;
  perStation: PerStation[];
  perLine: PerLine[];
}

export interface StationView {
  id: number;
  xMm: number;
  yMm: number;
  name: string;
  removed: boolean;
}

export interface LineView {
  id: number;
  name: string;
  mode: number;
  loopLine: boolean;
  color: number;
  stops: number[];
  polylineMm: [number, number][];
  minRadiusMm: number;
  spanModes: number[];
  crossesWaterSurface: boolean;
  removed: boolean;
}

/** The committed city manifest (frontend-facing fields; demand grid embedded for the sim). */
export interface CityData {
  id: string;
  name: string;
  originLngLat: [number, number];
  bbox: [number, number, number, number];
  center: [number, number];
  zoom: number;
  seed: number;
  demand: { cellM: number; cells: { x_mm: number; y_mm: number; origin_w: number; dest_w: number }[] };
}
