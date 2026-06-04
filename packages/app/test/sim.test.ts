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
    // Curved track (F1): the polyline is a dense smoothed curve, denser than the 3 stops.
    expect(lines[0].polylineMm.length).toBeGreaterThan(3);

    const stats = b.stats();
    expect(stats.stationCount).toBe(3);
    expect(stats.lineCount).toBe(1);
    expect(stats.perLine[0].trains).toBe(3);
    expect(typeof stats.simClockMs).toBe("number"); // not BigInt

    const pos = b.vehiclePositions();
    expect(pos).toBeInstanceOf(Float32Array);
    expect(pos.length % 2).toBe(0); // interleaved x,y
  });

  it("dispatches vehicles and moves them when running (T14)", () => {
    const b = build(); // build() assigns 3 trains, runs, ticks 50x
    expect(b.vehicleCount()).toBe(3);
    const before = Array.from(b.vehiclePositions());
    for (let i = 0; i < 200; i++) b.tick(50);
    const after = Array.from(b.vehiclePositions());
    const moved = before.some((v, i) => v !== after[i]);
    expect(moved).toBe(true); // a vehicle advanced along the line
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

describe("save/load + undo (Rank 3)", () => {
  it("undo rebuilds from seed + log[..-1] (never splices state)", () => {
    const b = new SimBridge(7, "{}");
    b.apply(cmd.placeStation(0, 0));
    b.apply(cmd.placeStation(5_000_000, 0));
    b.apply(cmd.createLine(0x0072b2));
    b.apply(cmd.addStop(0, 0));
    const hashBefore = b.stateHash();
    b.apply(cmd.addStop(0, 1)); // the command to undo
    expect(b.stateHash()).not.toBe(hashBefore);
    expect(b.undo()).toBe(true);
    expect(b.stateHash()).toBe(hashBefore); // back to the exact pre-command state
    expect(b.log.length).toBe(4);
  });

  it("undo on an empty log is a no-op", () => {
    expect(new SimBridge(1, "{}").undo()).toBe(false);
  });

  it("loadLog reconstructs an identical world (save = seed + log)", () => {
    const a = new SimBridge(7, "{}");
    const log = [
      cmd.placeStation(0, 0),
      cmd.placeStation(5_000_000, 0),
      cmd.createLine(0x0072b2),
      cmd.addStop(0, 0),
      cmd.addStop(0, 1),
      cmd.assignTrainset(0, 0, 2),
    ];
    for (const c of log) a.apply(c);

    const b = new SimBridge(7, "{}");
    b.loadLog(a.log.all());
    expect(b.stateHash()).toBe(a.stateHash());
    expect(b.linesView()).toEqual(a.linesView());
  });

  it("onCommit fires on apply and undo (autosave hook)", () => {
    const b = new SimBridge(3, "{}");
    let n = 0;
    b.onCommit = () => {
      n++;
    };
    b.apply(cmd.placeStation(0, 0));
    b.apply(cmd.placeStation(1, 0));
    expect(n).toBe(2);
    b.undo();
    expect(n).toBe(3);
  });
});
