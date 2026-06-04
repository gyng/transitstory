#!/usr/bin/env python3
"""Build a REAL travel-demand grid per city from OpenStreetMap (Overpass), replacing the
synthetic Gaussian bumps with land-use-derived weights so the coverage/ridership loop reflects
where people actually live and work (the audit's top data upgrade).

  originWeight (trip origins / homes) <- landuse=residential
  destWeight   (trip destinations / jobs) <- landuse=commercial|retail|industrial,
                                             building=office|commercial|retail|industrial,
                                             and shop/office/key-amenity POIs (density)

Output (per city, SAME schema the sim already consumes — a drop-in for build_demand.py):
  packages/app/public/data/<id>_demand.json
  { "cellM": <m>, "bbox": [...], "cells": [ {lon,lat,originWeight,destWeight}, ... ] }
Coords are WGS84; coords/geo.ts converts to mm at load. Cell size = the per-city `cellM`.

Re-runnable. On any Overpass failure the previously committed grid (synthetic or OSM) is KEPT,
exactly like build_networks.py / build_buildability.py — a network blip never breaks the game.
Dependency-light (urllib + json). Usage mirrors the others:
  scripts/build_demand_osm.py                 # all cities
  scripts/build_demand_osm.py singapore tokyo # just these
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
M_PER_DEG_LAT = 110540.0

# Per-cell weight added when its centre falls in a land-use polygon (capped so dense overlaps
# don't explode). POI nodes add a smaller per-node bump to dest (density of jobs/shops).
W_RESIDENTIAL = 2.0
W_COMMERCIAL = 2.0   # commercial / retail landuse + building
W_OFFICE = 2.6       # building=office (job-dense)
W_INDUSTRIAL = 1.0
W_POI = 0.35
ORIGIN_CAP = 6.0
DEST_CAP = 8.0
AMENITY_RE = "^(restaurant|cafe|bank|cinema|theatre|hospital|clinic|university|college|" \
             "marketplace|food_court|fast_food|pub|bar|library|townhall|courthouse)$"


def m_per_deg_lng(lat):
    return 111320.0 * math.cos(math.radians(lat))


def query(bbox):
    s, w, n, e = bbox[1], bbox[0], bbox[3], bbox[2]
    b = f"({s},{w},{n},{e})"
    q = "[out:json][timeout:240];(" + "".join([
        f'way[landuse=residential]{b};',
        f'way[landuse~"^(commercial|retail|industrial)$"]{b};',
        f'way[building~"^(office|commercial|retail|industrial|warehouse|supermarket)$"]{b};',
        f'node[shop]{b};node[office]{b};node[amenity~"{AMENITY_RE}"]{b};',
    ]) + ");out geom;"
    last = None
    for ep in ENDPOINTS:
        try:
            req = urllib.request.Request(ep, data=("data=" + urllib.parse.quote(q)).encode(),
                                         headers={"User-Agent": "transitstory/0.1"})
            with urllib.request.urlopen(req, timeout=260) as r:
                return json.load(r)
        except Exception as ex:
            last = ex
            time.sleep(2)
    raise RuntimeError(f"overpass failed: {last}")


def way_weight(tags):
    """(originDelta, destDelta) for a land-use / building polygon."""
    lu = tags.get("landuse", "")
    bld = tags.get("building", "")
    if lu == "residential":
        return W_RESIDENTIAL, 0.0
    if lu in ("commercial", "retail") or bld in ("commercial", "retail", "supermarket"):
        return 0.0, W_COMMERCIAL
    if bld == "office":
        return 0.0, W_OFFICE
    if lu == "industrial" or bld in ("industrial", "warehouse"):
        return 0.0, W_INDUSTRIAL
    return 0.0, 0.0


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


def build(cid, cfg):
    bbox = cfg["bbox"]
    cell_m = cfg["cellM"]
    data = query(bbox)
    west, south, east, north = bbox
    mlng = m_per_deg_lng((south + north) / 2)
    dlng = cell_m / mlng
    dlat = cell_m / M_PER_DEG_LAT
    cols = int((east - west) / dlng) + 1
    rows = int((north - south) / dlat) + 1
    origin = [0.0] * (cols * rows)
    dest = [0.0] * (cols * rows)

    for el in data["elements"]:
        t = el.get("tags", {})
        if el.get("type") == "way" and el.get("geometry"):
            ow, dw = way_weight(t)
            if ow == 0.0 and dw == 0.0:
                continue
            pts = [(g["lon"], g["lat"]) for g in el["geometry"]]
            if len(pts) < 3:
                continue
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
                        k = ri * cols + ci
                        origin[k] = min(ORIGIN_CAP, origin[k] + ow)
                        dest[k] = min(DEST_CAP, dest[k] + dw)
        elif el.get("type") == "node" and "lat" in el:
            # POI node (shop/office/amenity): a small destination-density bump.
            ci = int((el["lon"] - west) / dlng)
            ri = int((el["lat"] - south) / dlat)
            if 0 <= ci < cols and 0 <= ri < rows:
                k = ri * cols + ci
                dest[k] = min(DEST_CAP, dest[k] + W_POI)

    cells = []
    for ri in range(rows):
        clat = south + (ri + 0.5) * dlat
        if clat > north:  # the +1 row's centre can spill past the bbox edge — keep cells inside
            continue
        for ci in range(cols):
            clon = west + (ci + 0.5) * dlng
            if clon > east:
                continue
            k = ri * cols + ci
            o, d = origin[k], dest[k]
            if o + d > 0.05:
                cells.append({
                    "lon": round(clon, 5),
                    "lat": round(clat, 5),
                    "originWeight": round(o, 3),
                    "destWeight": round(d, 3),
                })
    if not cells:
        raise RuntimeError("no land-use cells found (empty result) — keeping existing")
    out = {"cellM": cell_m, "bbox": bbox, "cells": cells}
    json.dump(out, open(os.path.join(OUT, f"{cid}_demand.json"), "w"), separators=(",", ":"))
    print(f"[build_demand_osm] {cid}: OSM {len(cells)} cells @ {cell_m:.0f}m "
          f"(origin {sum(c['originWeight'] for c in cells):.0f}, dest {sum(c['destWeight'] for c in cells):.0f})")


def main():
    cfg = json.load(open(os.path.join(HERE, "city_demand_config.json")))
    want = sys.argv[1:] or list(cfg["cities"].keys())
    for cid in want:
        if cid not in cfg["cities"]:
            print(f"  skip unknown {cid}")
            continue
        try:
            build(cid, cfg["cities"][cid])
        except Exception as ex:
            print(f"  !! {cid}: {ex} — keeping existing demand grid")


if __name__ == "__main__":
    main()
