// Command builders + JSON encoder. The wire format is JSON (postcard is Rust-side save
// only). Builders produce the exact externally-tagged shape crates/sim deserializes.
import type { Command } from "../types";

export const cmd = {
  placeStation: (x_mm: number, y_mm: number, name: string | null = null): Command => ({
    PlaceStation: { x_mm, y_mm, name },
  }),
  createLine: (color: number, name: string | null = null, loop_line = false, mode = 0): Command => ({
    CreateLine: { color, name, loop_line, mode },
  }),
  addStop: (line: number, station: number, after: number | null = null): Command => ({
    AddStop: { line, station, after },
  }),
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
};

export const WHOLE_LINE = 0xffffffff;
export const BUILD_MODE = { SURFACE: 0, ELEVATED: 1, TUNNEL: 2 } as const;
/** Transport mode (Line.mode in the sim). Matches crates/sim trainset::tmode. */
export const TRANSPORT = { RAIL: 0, BUS: 1, FERRY: 2, AIR: 3 } as const;
export type TransportMode = (typeof TRANSPORT)[keyof typeof TRANSPORT];

export function encodeCommand(c: Command): string {
  return JSON.stringify(c);
}
