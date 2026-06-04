#!/usr/bin/env python3
"""Build a coarse BUILDABILITY grid per city from OpenStreetMap (Overpass) — the surface-rail
cost signal (PLAN: NIMBY punted on buildings; we add a SOFT penalty + one hard gate=water).
Each ~120 m cell is classified (priority high→low): Water > RailROW > RoadROW > Park > Built >
Open. Coarse classes (not raw footprints) keep it uniform across cities, integer-exact, and
small — sidestepping NIMBY's uneven-coverage trap.

Output: packages/app/public/data/<id>_buildability.json
  { "cellM": <m>, "bbox": [...], "cells": [ {lon,lat,c}, ... ] }   (Open cells omitted)
  c: 1=RoadROW 2=RailROW 3=Built 4=Water 5=Park   (0=Open is the default)
Re-runnable. On failure/timeout the previously committed grid (or an empty one) is kept, so
the game still works (no penalties, no water gate). Dependency-light (urllib + json).
"""
import json
import math
import os
import sys
import time
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "packages", "app", "public", "data")
ENDPOINTS = ["https://overpass-api.de/api/interpreter", "https://overpass.kumi.systems/api/interpreter"]
CELL_M = 120.0
OPEN, ROAD, RAIL, BUILT, WATER, PARK = 0, 1, 2, 3, 4, 5
M_PER_DEG_LAT = 110540.0


def m_per_deg_lng(lat):
    return 111320.0 * math.cos(math.radians(lat))


def query(bbox):
    s, w, n, e = bbox[1], bbox[0], bbox[3], bbox[2]
    b = f"({s},{w},{n},{e})"
    q = "[out:json][timeout:240];(" + "".join([
        f'way[natural=water]{b};way["water"]{b};way[waterway=riverbank]{b};way[landuse=reservoir]{b};',
        f'way[landuse~"^(residential|commercial|industrial|retail|construction)$"]{b};',
        f'way[leisure=park]{b};way[landuse~"^(forest|grass|meadow|recreation_ground|cemetery)$"]{b};way[natural=wood]{b};',
        f'way[highway~"^(motorway|trunk|primary)$"]{b};',
        f'way[railway~"^(rail|light_rail|subway)$"]{b};',
    ]) + ");out geom;"
    last = None
    for ep in ENDPOINTS:
        try:
            req = urllib.request.Request(ep, data=("data=" + urllib.parse.quote(q)).encode(),
                                         headers={"User-Agent": "onlytransits/0.1"})
            with urllib.request.urlopen(req, timeout=260) as r:
                return json.load(r)
        except Exception as ex:
            last = ex
            time.sleep(2)
    raise RuntimeError(f"overpass failed: {last}")


def classify(tags):
    if tags.get("natural") == "water" or "water" in tags or tags.get("waterway") == "riverbank" \
            or tags.get("landuse") == "reservoir":
        return WATER, True  # polygon
    if tags.get("railway") in ("rail", "light_rail", "subway"):
        return RAIL, False  # line
    if tags.get("highway") in ("motorway", "trunk", "primary"):
        return ROAD, False  # line
    if tags.get("leisure") == "park" or tags.get("natural") == "wood" \
            or tags.get("landuse") in ("forest", "grass", "meadow", "recreation_ground", "cemetery"):
        return PARK, True
    if tags.get("landuse") in ("residential", "commercial", "industrial", "retail", "construction"):
        return BUILT, True
    return None, None


def point_in_poly(px, py, poly):
    inside = False
    n = len(poly)
    j = n - 1
    for i in range(n):
        xi, yi = poly[i]
        xj, yj = poly[j]
        if ((yi > py) != (yj > py)) and (px < (xj - xi) * (py - yi) / (yj - yi + 1e-12) + xi):
            inside = not inside
        j = i
    return inside


def build(cid, bbox):
    data = query(bbox)
    west, south, east, north = bbox
    mlng = m_per_deg_lng((south + north) / 2)
    dlng = CELL_M / mlng
    dlat = CELL_M / M_PER_DEG_LAT
    cols = int((east - west) / dlng) + 1
    rows = int((north - south) / dlat) + 1
    grid = [OPEN] * (cols * rows)

    def setcell(ci, ri, cls):
        if 0 <= ci < cols and 0 <= ri < rows:
            k = ri * cols + ci
            if cls > grid[k]:  # priority: higher code wins (Water > Rail > Road > Built/Park)
                grid[k] = cls

    # Process polygons first (Built/Park/Water), then lines overlay; priority via max-code.
    order = {BUILT: 0, PARK: 1, ROAD: 2, RAIL: 3, WATER: 4}
    ways = [el for el in data["elements"] if el.get("type") == "way" and el.get("geometry")]
    ways.sort(key=lambda el: order.get(classify(el.get("tags", {}))[0] or -1, -1))
    for el in ways:
        cls, is_poly = classify(el.get("tags", {}))
        if cls is None:
            continue
        pts = [(g["lon"], g["lat"]) for g in el["geometry"]]
        if is_poly and len(pts) >= 3:
            xs = [p[0] for p in pts]
            ys = [p[1] for p in pts]
            c0 = max(0, int((min(xs) - west) / dlng))
            c1 = min(cols - 1, int((max(xs) - west) / dlng))
            r0 = max(0, int((min(ys) - south) / dlat))
            r1 = min(rows - 1, int((max(ys) - south) / dlat))
            for ri in range(r0, r1 + 1):
                py = south + (ri + 0.5) * dlat
                for ci in range(c0, c1 + 1):
                    px = west + (ci + 0.5) * dlng
                    if point_in_poly(px, py, pts):
                        setcell(ci, ri, cls)
        else:  # line: stamp cells along each segment
            for i in range(1, len(pts)):
                ax, ay = pts[i - 1]
                bx, by = pts[i]
                steps = max(1, int(math.hypot((bx - ax) / dlng, (by - ay) / dlat)) + 1)
                for t in range(steps + 1):
                    x = ax + (bx - ax) * t / steps
                    y = ay + (by - ay) * t / steps
                    setcell(int((x - west) / dlng), int((y - south) / dlat), cls)

    cells = []
    for ri in range(rows):
        for ci in range(cols):
            c = grid[ri * cols + ci]
            if c != OPEN:
                cells.append({"lon": round(west + (ci + 0.5) * dlng, 5),
                              "lat": round(south + (ri + 0.5) * dlat, 5), "c": c})
    out = {"cellM": CELL_M, "bbox": bbox, "cells": cells}
    json.dump(out, open(os.path.join(OUT, f"{cid}_buildability.json"), "w"), separators=(",", ":"))
    from collections import Counter
    hist = Counter(x["c"] for x in cells)
    print(f"[build_buildability] {cid}: {len(cells)} non-open cells @ {CELL_M:.0f}m "
          f"(road {hist[ROAD]}, rail {hist[RAIL]}, built {hist[BUILT]}, water {hist[WATER]}, park {hist[PARK]})")


def main():
    cfg = json.load(open(os.path.join(HERE, "city_demand_config.json")))
    want = sys.argv[1:] or list(cfg["cities"].keys())
    for cid in want:
        if cid not in cfg["cities"]:
            print(f"  skip unknown {cid}")
            continue
        try:
            build(cid, cfg["cities"][cid]["bbox"])
        except Exception as ex:
            print(f"  !! {cid}: {ex} — keeping existing/none")


if __name__ == "__main__":
    main()
