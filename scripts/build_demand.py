#!/usr/bin/env python3
"""Generate a DETERMINISTIC synthetic Singapore demand grid (PLAN §0 / risk register:
go straight to synthetic; pyrosm/OSM-derived weights are a post-slice nicety — the sim
consumes the committed JSON identically either way).

Output: packages/app/public/data/singapore_demand.json
  { "cellM": <m>, "bbox": [...], "cells": [ {lon,lat,originWeight,destWeight}, ... ] }
Coords are WGS84 lng/lat; the frontend (coords/geo.ts) converts to local mm before
embedding into Sim::new — the sim never sees lng/lat.

Weights are a sum of Gaussian bumps over real-ish Singapore residential / employment
centres, so the demand surface is plausible and non-uniform without any OSM dependency.
Deterministic: fixed RNG seed, so re-running reproduces the committed file byte-for-byte.
"""
import json
import math
import os
import random

BBOX = (103.55, 1.13, 104.15, 1.50)  # west, south, east, north
CELL_M = 600.0
SEED = 12345

M_PER_DEG_LAT = 110540.0
def m_per_deg_lng(lat):
    return 111320.0 * math.cos(math.radians(lat))

# (lng, lat, amplitude, sigma_km) — employment/retail cores (destinations).
JOB_CENTERS = [
    (103.851, 1.283, 1.00, 3.0),   # CBD / Raffles Place
    (103.838, 1.304, 0.70, 2.5),   # Orchard
    (103.744, 1.333, 0.55, 3.0),   # Jurong East
    (103.946, 1.353, 0.45, 3.0),   # Tampines
    (103.790, 1.437, 0.40, 3.0),   # Woodlands
    (103.988, 1.356, 0.35, 3.5),   # Changi / Expo
    (103.847, 1.370, 0.35, 2.5),   # Ang Mo Kio / Bishan
]
# Residential heartlands (origins) — broader spread.
HOME_CENTERS = [
    (103.945, 1.353, 0.9, 4.0), (103.790, 1.437, 0.9, 4.5), (103.744, 1.339, 0.8, 4.0),
    (103.930, 1.324, 0.8, 3.5), (103.851, 1.370, 0.8, 3.5), (103.910, 1.403, 0.7, 3.5),
    (103.765, 1.315, 0.7, 3.0), (103.835, 1.429, 0.7, 3.5), (103.852, 1.310, 0.6, 3.0),
    (104.000, 1.350, 0.4, 3.0),
]


def bump(lon, lat, centers):
    total = 0.0
    for clng, clat, amp, sigma_km in centers:
        dlat_km = (lat - clat) * M_PER_DEG_LAT / 1000.0
        dlng_km = (lon - clng) * m_per_deg_lng(lat) / 1000.0
        d2 = dlat_km * dlat_km + dlng_km * dlng_km
        total += amp * math.exp(-d2 / (2.0 * sigma_km * sigma_km))
    return total


def main():
    rng = random.Random(SEED)
    west, south, east, north = BBOX
    dlat = CELL_M / M_PER_DEG_LAT
    cells = []
    lat = south
    while lat <= north:
        dlng = CELL_M / m_per_deg_lng(lat)
        lon = west
        while lon <= east:
            # jitter-free cell centre; deterministic multiplicative noise per cell.
            noise = 0.75 + 0.5 * rng.random()
            origin = bump(lon, lat, HOME_CENTERS) * noise
            dest = bump(lon, lat, JOB_CENTERS) * (0.75 + 0.5 * rng.random())
            if origin + dest > 0.04:  # drop sea / cross-border cells (no nearby centre)
                cells.append({
                    "lon": round(lon, 5),
                    "lat": round(lat, 5),
                    "originWeight": round(origin, 3),
                    "destWeight": round(dest, 3),
                })
            lon += dlng
        lat += dlat

    out = {"cellM": CELL_M, "bbox": list(BBOX), "cells": cells}
    here = os.path.dirname(os.path.abspath(__file__))
    dest_path = os.path.join(here, "..", "packages", "app", "public", "data", "singapore_demand.json")
    os.makedirs(os.path.dirname(dest_path), exist_ok=True)
    with open(dest_path, "w") as f:
        json.dump(out, f, separators=(",", ":"))
    total_o = sum(c["originWeight"] for c in cells)
    total_d = sum(c["destWeight"] for c in cells)
    print(f"[build_demand] SYNTHETIC grid: {len(cells)} cells @ {CELL_M:.0f}m "
          f"(origin sum {total_o:.0f}, dest sum {total_d:.0f}) -> {os.path.relpath(dest_path, here)}")


if __name__ == "__main__":
    main()
