// Wasm-in-node smoke (the ONLY test that touches wasm; sim logic is tested natively in
// crates/sim). Proves the bundler-target wasm imports + instantiates under Vite/Vitest,
// commands round-trip through the JSON wire, views marshal, and replay is deterministic.
// (Vehicle-buffer-populated + ridership assertions arrive with T14/T16.)
import { describe, expect, it } from "vitest";
import { SimBridge } from "../src/sim/SimBridge";
import { cmd } from "../src/commands/codec";

function build(): SimBridge {
  const b = new SimBridge(42, "{}");
  b.apply(cmd.placeStation(0, 0));
  b.apply(cmd.placeStation(5_000_000, 0));
  b.apply(cmd.placeStation(10_000_000, 2_000_000));
  b.apply(cmd.createLine(0x0072b2));
  b.apply(cmd.addStop(0, 0));
  b.apply(cmd.addStop(0, 1));
  b.apply(cmd.addStop(0, 2));
  b.apply(cmd.assignTrainset(0, 0, 3));
  b.apply(cmd.setHeadway(0, 240_000));
  b.apply(cmd.setRunning(true));
  for (let i = 0; i < 50; i++) b.tick(50);
  return b;
}

describe("SimBridge (wasm-in-node)", () => {
  it("applies JSON commands and exposes authoritative views", () => {
    const b = build();

    const stations = b.stationsView();
    expect(stations).toHaveLength(3);
    expect(stations[0].name).toBe("Station 1"); // deterministic auto-name
    expect(stations[2].name).toBe("Station 3");

    const lines = b.linesView();
    expect(lines).toHaveLength(1);
    expect(lines[0].stops).toEqual([0, 1, 2]);
    expect(lines[0].polylineMm).toHaveLength(3); // geometry rebuilt on AddStop

    const stats = b.stats();
    expect(stats.stationCount).toBe(3);
    expect(stats.lineCount).toBe(1);
    expect(stats.perLine[0].trains).toBe(3);
    expect(typeof stats.simClockMs).toBe("number"); // not BigInt

    const pos = b.vehiclePositions();
    expect(pos).toBeInstanceOf(Float32Array);
    expect(pos.length % 2).toBe(0); // interleaved x,y (populated in T14)
  });

  it("rejects invalid commands without throwing", () => {
    const b = new SimBridge(0, "{}");
    const events = b.apply(cmd.addStop(9, 9)); // no such line/station
    expect(events[0]).toHaveProperty("Rejected");
  });

  it("is deterministic: identical command+tick sequence => identical state_hash", () => {
    expect(build().stateHash()).toBe(build().stateHash());
  });
});
