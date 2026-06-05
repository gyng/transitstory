// The hand-mirrored wire contract (AGENTS code-org: types.ts + codec.ts mirror crates/sim).
// Pins the Command vocabulary so the codec can't silently drift from the Rust enum — the Rust
// side is pinned by crates/sim/tests/contract.rs against this same canonical list.
import { describe, expect, it } from "vitest";
import { cmd, encodeCommand } from "../src/commands/codec";

// Canonical, mirrored from crates/sim/src/command.rs `enum Command`.
const COMMAND_TAGS = [
  "AddStop",
  "AssignTrainset",
  "CreateLine",
  "PlaceStation",
  "SetEconomy",
  "SetDemandMode",
  "SetHeadway",
  "SetLineWaypoints",
  "SetRunning",
  "SetSegmentMode",
].sort();

describe("command wire contract", () => {
  it("every builder emits exactly one externally-tagged variant in the canonical set", () => {
    const builders = [
      cmd.placeStation(0, 0),
      cmd.createLine(0),
      cmd.addStop(0, 0),
      cmd.assignTrainset(0, 0, 1),
      cmd.setHeadway(0, 0),
      cmd.setSegmentMode(0, 0, 0),
      cmd.setRunning(false),
      cmd.setEconomy(false),
      cmd.setLineWaypoints(0, []),
      cmd.setDemandMode(false),
    ];
    const tags = builders
      .map((c) => {
        const keys = Object.keys(JSON.parse(encodeCommand(c)));
        expect(keys).toHaveLength(1);
        return keys[0];
      })
      .sort();
    expect(tags).toEqual(COMMAND_TAGS);
  });
});
