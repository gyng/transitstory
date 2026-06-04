#!/usr/bin/env python3
"""make_manifest.py — generate a <id>_city.json manifest for a city already defined in
scripts/city_demand_config.json, so adding a city is: config block + build_data.sh + ONE
CITIES entry in packages/app/src/sim/cities.ts (instead of also hand-authoring the manifest).

The manifest is the committed CityData the frontend loader reads (sim/city.ts). Coords are
WGS84; the demand grid stores lon/lat and is converted to mm at load against `originLngLat`,
so any reasonable origin is internally consistent — we use the bbox centre.

Existing manifests are SKIPPED (the six hand-authored ones keep their tuned name/origin/zoom)
unless --force is given.

Usage:
  scripts/make_manifest.py                 # all cities in the config (skip existing)
  scripts/make_manifest.py shanghai london # just these
  scripts/make_manifest.py --force london  # overwrite an existing manifest
"""
import json
import math
import os
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONFIG = os.path.join(ROOT, "scripts", "city_demand_config.json")
OUT_DIR = os.path.join(ROOT, "packages", "app", "public", "data")


def derive_zoom(bbox):
    """A sensible default zoom from the bbox span (the map is interactive, so it's only a start)."""
    span = max(abs(bbox[2] - bbox[0]), abs(bbox[3] - bbox[1]), 1e-6)
    return int(max(9, min(13, round(9.0 - math.log2(span)))))


def seed_for(cid):
    """Stable deterministic seed from the id, so a regenerated manifest is reproducible."""
    return zlib.crc32(cid.encode()) & 0x7FFFFFFF


def build_manifest(cid, cfg):
    bbox = cfg["bbox"]
    cx = round((bbox[0] + bbox[2]) / 2, 4)
    cy = round((bbox[1] + bbox[3]) / 2, 4)
    return {
        "id": cid,
        "name": cfg.get("name", cid.replace("_", " ").title()),
        "originLngLat": [cx, cy],
        "bbox": bbox,
        "center": [cx, cy],
        "zoom": derive_zoom(bbox),
        "seed": seed_for(cid),
        "demandGridPath": f"/data/{cid}_demand.json",
        "networkPath": f"/data/networks/{cid}.json",
        "buildabilityPath": f"/data/{cid}_buildability.json",
    }


def main(argv):
    force = "--force" in argv
    ids = [a for a in argv if not a.startswith("-")]
    cities = json.load(open(CONFIG))["cities"]
    if not ids:
        ids = list(cities.keys())

    for cid in ids:
        if cid not in cities:
            print(f"  ! {cid}: not in city_demand_config.json — skipping")
            continue
        out = os.path.join(OUT_DIR, f"{cid}_city.json")
        if os.path.exists(out) and not force:
            print(f"  = {cid}: manifest exists (use --force to overwrite) — skipping")
            continue
        m = build_manifest(cid, cities[cid])
        with open(out, "w") as f:
            json.dump(m, f, indent=2)
            f.write("\n")
        print(f"  ✓ {cid}: wrote {os.path.relpath(out, ROOT)} (zoom {m['zoom']}, seed {m['seed']})")


if __name__ == "__main__":
    main(sys.argv[1:])
