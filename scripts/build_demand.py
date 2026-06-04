#!/usr/bin/env python3
"""Generate DETERMINISTIC synthetic demand grids for every city in
scripts/city_demand_config.json (PLAN §0 / risk register: synthetic over pyrosm; the sim
consumes the committed JSON identically). Each grid is a sum of Gaussian bumps over the
city's employment (job) and residential (home) centres — plausible, non-uniform, and
reproducible byte-for-byte from a fixed seed.

Output (per city): packages/app/public/data/<id>_demand.json
  { "cellM": <m>, "bbox": [...], "cells": [ {lon,lat,originWeight,destWeight}, ... ] }
Coords are WGS84 lng/lat; the frontend (coords/geo.ts) converts to mm before embedding.
"""
import json
import math
import os
import random

HERE = os.path.dirname(os.path.abspath(__file__))
M_PER_DEG_LAT = 110540.0


def m_per_deg_lng(lat):
    return 111320.0 * math.cos(math.radians(lat))


def bump(lon, lat, centers):
    total = 0.0
    for clng, clat, amp, sigma_km in centers:
        dlat_km = (lat - clat) * M_PER_DEG_LAT / 1000.0
        dlng_km = (lon - clng) * m_per_deg_lng(lat) / 1000.0
        d2 = dlat_km * dlat_km + dlng_km * dlng_km
        total += amp * math.exp(-d2 / (2.0 * sigma_km * sigma_km))
    return total


def build_city(cid, cfg, seed):
    rng = random.Random(seed)
    west, south, east, north = cfg["bbox"]
    cell_m = cfg["cellM"]
    jobs = cfg["jobCenters"]
    homes = cfg["homeCenters"]
    dlat = cell_m / M_PER_DEG_LAT
    cells = []
    lat = south
    while lat <= north:
        dlng = cell_m / m_per_deg_lng(lat)
        lon = west
        while lon <= east:
            origin = bump(lon, lat, homes) * (0.75 + 0.5 * rng.random())
            dest = bump(lon, lat, jobs) * (0.75 + 0.5 * rng.random())
            if origin + dest > 0.05:  # drop empty/water cells (no nearby centre)
                cells.append({
                    "lon": round(lon, 5),
                    "lat": round(lat, 5),
                    "originWeight": round(origin, 3),
                    "destWeight": round(dest, 3),
                })
            lon += dlng
        lat += dlat
    out = {"cellM": cell_m, "bbox": cfg["bbox"], "cells": cells}
    dest_path = os.path.join(HERE, "..", "packages", "app", "public", "data", f"{cid}_demand.json")
    os.makedirs(os.path.dirname(dest_path), exist_ok=True)
    with open(dest_path, "w") as f:
        json.dump(out, f, separators=(",", ":"))
    print(f"[build_demand] {cid}: SYNTHETIC {len(cells)} cells @ {cell_m:.0f}m "
          f"(origin {sum(c['originWeight'] for c in cells):.0f}, dest {sum(c['destWeight'] for c in cells):.0f})")


def main():
    cfg = json.load(open(os.path.join(HERE, "city_demand_config.json")))
    # Deterministic per-city seed (stable across runs).
    seeds = {"singapore": 20260604, "tokyo": 20260605, "calgary": 20260606}
    for cid, ccfg in cfg["cities"].items():
        build_city(cid, ccfg, seeds.get(cid, 1))


if __name__ == "__main__":
    main()
