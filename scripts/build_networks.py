#!/usr/bin/env python3
"""Pull REAL transit networks from OpenStreetMap (Overpass API) and emit them as
packages/app/public/data/networks/<id>.json — the same schema Game.applyNetwork consumes.
Re-runnable to refresh/update from OSM. OSM models transit as route relations
(route=subway/light_rail/monorail/tram) whose ordered "stop" members are the stations and
whose `colour` tag is the line colour; route_master groups direction variants.

Network only (build-time tool); on failure the previously committed network JSON is kept.
Dependency-free (urllib + json). Usage:
  python3 scripts/build_networks.py [city_id ...]      # default: all configured cities
"""
import json
import math
import os
import sys
import time
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_DIR = os.path.join(HERE, "..", "packages", "app", "public", "data", "networks")
ENDPOINTS = [
    "https://overpass-api.de/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
]
# Route types to pull per city (Tokyo's heavy-rail "train" network is enormous, so subway+LR).
ROUTE_RE = "^(subway|light_rail|monorail|tram)$"
PALETTE = ["d42e12", "009645", "9e28b5", "fa9e0d", "0aa1dd", "00b0b9",
           "e6007e", "6f2c91", "f68b1f", "84bd00", "ee2737", "0067a5"]


def cities():
    cfg = json.load(open(os.path.join(HERE, "city_demand_config.json")))
    names = {"singapore": "Singapore MRT", "tokyo": "Tokyo", "calgary": "Calgary C-Train"}
    return {cid: {"bbox": c["bbox"], "name": names.get(cid, cid)} for cid, c in cfg["cities"].items()}


def overpass(bbox):
    s, w, n, e = bbox[1], bbox[0], bbox[3], bbox[2]
    q = (
        f"[out:json][timeout:180];"
        f'(relation["type"="route"]["route"~"{ROUTE_RE}"]({s},{w},{n},{e}););'
        f"out body;>;out body qt;"
    )
    last = None
    for ep in ENDPOINTS:
        try:
            req = urllib.request.Request(ep, data=("data=" + urllib.parse.quote(q)).encode(),
                                         headers={"User-Agent": "onlytransits/0.1 (build script)"})
            with urllib.request.urlopen(req, timeout=200) as r:
                return json.load(r)
        except Exception as ex:  # try the next mirror
            last = ex
            time.sleep(2)
    raise RuntimeError(f"overpass failed: {last}")


def norm_colour(tags, idx):
    c = (tags.get("colour") or tags.get("color") or "").strip().lstrip("#")
    if len(c) == 3:
        c = "".join(ch * 2 for ch in c)
    if len(c) == 6 and all(ch in "0123456789abcdefABCDEF" for ch in c):
        return c.lower()
    return PALETTE[idx % len(PALETTE)]


def dist_km(a, b):
    return math.hypot((a[0] - b[0]) * 111.32 * math.cos(math.radians(a[1])),
                      (a[1] - b[1]) * 110.54)


def build(cid, meta):
    data = overpass(meta["bbox"])
    nodes, rels = {}, []
    for el in data["elements"]:
        if el["type"] == "node":
            nodes[el["id"]] = el
        elif el["type"] == "relation" and el.get("tags", {}).get("route"):
            rels.append(el)

    # Group route variants into one line by ref|name, keep the variant with the most stops.
    best = {}
    for r in rels:
        t = r.get("tags", {})
        name = t.get("name") or t.get("ref")
        if not name:
            continue
        stops = [m["ref"] for m in r.get("members", [])
                 if m["type"] == "node" and str(m.get("role", "")).startswith("stop")]
        if len(stops) < 2:  # fall back to platforms if no stop roles
            stops = [m["ref"] for m in r.get("members", [])
                     if m["type"] == "node" and "platform" in str(m.get("role", ""))]
        if len(stops) < 2:
            continue
        key = t.get("ref") or name
        if key not in best or len(stops) > len(best[key][1]):
            best[key] = (r, stops)

    # Dedup stations by name (merging same-name stops => interchanges).
    stations, idx_by_name = [], {}

    def station_index(node_id):
        nd = nodes.get(node_id)
        if not nd or "lat" not in nd:
            return None
        nm = (nd.get("tags", {}).get("name") or nd.get("tags", {}).get("name:en")
              or f"Stop {node_id}")
        if nm in idx_by_name:
            return idx_by_name[nm]
        i = len(stations)
        idx_by_name[nm] = i
        stations.append({"name": nm, "lng": round(nd["lon"], 5), "lat": round(nd["lat"], 5)})
        return i

    lines = []
    for ci, (key, (r, stops)) in enumerate(sorted(best.items())):
        t = r.get("tags", {})
        seq, last = [], None
        for nid in stops:
            si = station_index(nid)
            if si is not None and si != last:
                seq.append(si)
                last = si
        if len(seq) < 2:
            continue
        lines.append({
            "name": t.get("name") or t.get("ref") or key,
            "colorHex": norm_colour(t, ci),
            "headwayMin": 4,
            "trains": max(4, min(12, len(seq) // 3)),
            "stations": seq,
        })

    if not lines:
        raise RuntimeError("no usable routes parsed")
    net = {"cityId": cid, "name": meta["name"], "stations": stations, "lines": lines}
    os.makedirs(OUT_DIR, exist_ok=True)
    json.dump(net, open(os.path.join(OUT_DIR, f"{cid}.json"), "w"), separators=(",", ":"))
    inter = sum(1 for nm in idx_by_name
                if sum(1 for l in lines if idx_by_name[nm] in l["stations"]) > 1)
    print(f"[build_networks] {cid}: {len(lines)} lines, {len(stations)} stations, ~{inter} interchanges (OSM)")


def main():
    want = sys.argv[1:] or list(cities().keys())
    cfg = cities()
    for cid in want:
        if cid not in cfg:
            print(f"  skip unknown city {cid}")
            continue
        try:
            build(cid, cfg[cid])
        except Exception as ex:
            print(f"  !! {cid}: {ex} — keeping existing committed network")


if __name__ == "__main__":
    main()
