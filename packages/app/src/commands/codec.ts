// Command builders + JSON encoder. The wire format is JSON (postcard is Rust-side save
// only). Builders produce the exact externally-tagged shape crates/sim deserializes.
import type { Command } from "../types";

export const cmd = {
  placeStation: (x_mm: number, y_mm: number, name: string | null = null): Command => ({
    PlaceStation: { x_mm, y_mm, name },
  }),
  createLine: (color: number, name: string | null = null, loop_line = false, mode = 0, literal = false): Command => ({
    CreateLine: { color, name, loop_line, mode, literal },
  }),
  addStop: (line: number, station: number, after: number | null = null): Command => ({
    AddStop: { line, station, after },
  }),
  /** Append a stop to a line's branch tree (P3). branch == current branch count creates a new
   *  branch leaving the trunk at stop `diverge_at`; branch < count extends that branch. */
  addBranchStop: (line: number, branch: number, diverge_at: number, station: number): Command => ({
    AddBranchStop: { line, branch, diverge_at, station },
  }),
  /** Set a branch's own per-span real-geometry waypoints (mm `[x,y]`) — the spur's OSM alignment. */
  setBranchWaypoints: (line: number, branch: number, waypoints: [number, number][][]): Command => ({
    SetBranchWaypoints: { line, branch, waypoints },
  }),
  /** Build mode (0=Surface,1=Elevated,2=Tunnel) for a whole branch's own track. */
  setBranchTrack: (line: number, branch: number, mode: number): Command => ({
    SetBranchTrack: { line, branch, mode },
  }),
  /** Bulldoze a branch off a line (trunk + other branches stay). */
  removeBranch: (line: number, branch: number): Command => ({ RemoveBranch: { line, branch } }),
  assignTrainset: (line: number, spec: number, count: number): Command => ({
    AssignTrainset: { line, spec, count },
  }),
  setHeadway: (line: number, headway_ms: number): Command => ({
    SetHeadway: { line, headway_ms },
  }),
  /** span = WHOLE_LINE (0xffffffff) sets every span; mode 0=Surface,1=Elevated,2=Tunnel. */
  setSegmentMode: (line: number, span: number, mode: number): Command => ({
    SetSegmentMode: { line, span, mode },
  }),
  setRunning: (running: boolean): Command => ({ SetRunning: { running } }),
  setEconomy: (enabled: boolean): Command => ({ SetEconomy: { enabled } }),
  /** Bulldoze a station (tombstone; dropped from any line through it). */
  removeStation: (station: number): Command => ({ RemoveStation: { station } }),
  /** Bulldoze a whole line (tombstone; its vehicles despawn). */
  removeLine: (line: number): Command => ({ RemoveLine: { line } }),
  /** Set the per-span control points (mm `[x,y]`) that bend a line's track between its stops. */
  setLineWaypoints: (line: number, waypoints: [number, number][][]): Command => ({ SetLineWaypoints: { line, waypoints } }),
  /** Switch the demand model: agents=true → seed-derived citizen commuters; false → gravity flow. */
  setDemandMode: (agents: boolean): Command => ({ SetDemandMode: { agents } }),
};

export const WHOLE_LINE = 0xffffffff;
export const BUILD_MODE = { SURFACE: 0, ELEVATED: 1, TUNNEL: 2 } as const;
/** Transport mode (Line.mode in the sim). Matches crates/sim trainset::tmode. */
export const TRANSPORT = { RAIL: 0, BUS: 1, FERRY: 2, AIR: 3, HEAVY: 4 } as const;
export type TransportMode = (typeof TRANSPORT)[keyof typeof TRANSPORT];

export function encodeCommand(c: Command): string {
  return JSON.stringify(c);
}
