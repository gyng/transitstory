import { describe, expect, it } from "vitest";
import {
  lngLatToMeters,
  lngLatToMm,
  metersToLngLat,
  mmToLngLat,
} from "../src/coords/geo";

describe("coords/geo boundary", () => {
  it("round-trips lng/lat <-> metres near the Singapore origin", () => {
    const ll: [number, number] = [103.85, 1.3];
    const back = metersToLngLat(lngLatToMeters(ll));
    expect(back[0]).toBeCloseTo(ll[0], 6);
    expect(back[1]).toBeCloseTo(ll[1], 6);
  });

  it("round-trips lng/lat <-> mm to sub-metre accuracy", () => {
    const ll: [number, number] = [103.9, 1.42];
    const back = mmToLngLat(lngLatToMm(ll));
    expect(back[0]).toBeCloseTo(ll[0], 4);
    expect(back[1]).toBeCloseTo(ll[1], 4);
  });

  it("maps the origin to ~(0,0) metres", () => {
    const [x, y] = lngLatToMeters([103.8198, 1.3521]);
    expect(Math.hypot(x, y)).toBeLessThan(1e-6);
  });
});
