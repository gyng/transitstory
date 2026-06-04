// Validates the committed manifests + the lng/lat -> mm core-city build (the exact shape
// Sim::new deserializes), and that the synthetic grid is non-empty and within the bbox.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { buildCoreCity, type RawCity, type RawDemand } from "../src/sim/city";

const dir = fileURLToPath(new URL(".", import.meta.url));
const raw: RawCity = JSON.parse(
  readFileSync(`${dir}../public/data/singapore_city.json`, "utf8"),
);
const demand: RawDemand = JSON.parse(
  readFileSync(`${dir}../public/data/singapore_demand.json`, "utf8"),
);

describe("city manifest + demand grid", () => {
  it("manifest has the expected fields", () => {
    expect(raw.id).toBe("singapore");
    expect(raw.seed).toBeGreaterThan(0);
    expect(raw.demandGridPath).toBe("/data/singapore_demand.json");
  });

  it("synthetic demand grid is non-empty and within the bbox", () => {
    expect(demand.cells.length).toBeGreaterThan(200);
    const [w, s, e, n] = demand.bbox;
    for (const c of demand.cells) {
      expect(c.lon).toBeGreaterThanOrEqual(w);
      expect(c.lon).toBeLessThanOrEqual(e);
      expect(c.lat).toBeGreaterThanOrEqual(s);
      expect(c.lat).toBeLessThanOrEqual(n);
      expect(c.originWeight + c.destWeight).toBeGreaterThan(0);
    }
  });

  it("builds a core city JSON with mm demand cells (snake_case, parseable by the sim)", () => {
    const { json, cellCount } = buildCoreCity(raw, demand);
    expect(cellCount).toBe(demand.cells.length);
    const core = JSON.parse(json);
    expect(core.seed).toBe(raw.seed);
    expect(core.demand.cell_m).toBe(demand.cellM);
    expect(core.demand.cells[0]).toHaveProperty("x_mm");
    expect(core.demand.cells[0]).toHaveProperty("origin_w");
    expect(Number.isInteger(core.demand.cells[0].x_mm)).toBe(true);
  });
});
