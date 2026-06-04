// Command builders + JSON encoder. The wire format is JSON (postcard is Rust-side save
// only). Builders produce the exact externally-tagged shape crates/sim deserializes.
import type { Command } from "../types";

export const cmd = {
  placeStation: (x_mm: number, y_mm: number, name: string | null = null): Command => ({
    PlaceStation: { x_mm, y_mm, name },
  }),
  createLine: (color: number): Command => ({ CreateLine: { color } }),
  addStop: (line: number, station: number, after: number | null = null): Command => ({
    AddStop: { line, station, after },
  }),
  assignTrainset: (line: number, spec: number, count: number): Command => ({
    AssignTrainset: { line, spec, count },
  }),
  setHeadway: (line: number, headway_ms: number): Command => ({
    SetHeadway: { line, headway_ms },
  }),
  setRunning: (running: boolean): Command => ({ SetRunning: { running } }),
};

export function encodeCommand(c: Command): string {
  return JSON.stringify(c);
}
