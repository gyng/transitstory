#!/usr/bin/env python3
"""Pull REAL transit networks from OpenStreetMap (Overpass API) and emit them as
packages/app/public/data/networks/<id>.json — the same schema Game.applyNetwork consumes.
Re-runnable to refresh/update from OSM. OSM models transit as route relations
(route=subway/light_rail/monorail/tram/train/ferry) whose ordered "stop" members are the stations and
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
# Route types to pull per city: rapid-transit rail + heavy/commuter rail (route=train) + ferries.
# Long-distance / high-speed / sleeper / car-shuttle "train" services are filtered out in build()
# (see SKIP_SERVICE) so we keep urban + commuter/suburban/regional rail only. Buses number in the
# hundreds and would swamp a starting network, so they're left out of the default pull.
ROUTE_RE = "^(subway|light_rail|monorail|tram|train|ferry)$"
# route=train services that are NOT urban transit (intercity, Shinkansen/TGV, sleepers, car trains).
SKIP_SERVICE = {"high_speed", "long_distance", "night", "car", "car_shuttle"}

# OSM route tag -> sim transport mode (crates/sim trainset::tmode: 0 rail,1 bus,2 ferry,3 air,
# 4 heavy/high-speed rail). Metro-family routes are regular rail (0); route=train (commuter /
# regional / mainline) imports as HEAVY rail (4) so it gets the fast trainset + mainline styling.
ROUTE_MODE = {
    "subway": 0, "light_rail": 0, "monorail": 0, "tram": 0,
    "train": 4,
    "bus": 1, "trolleybus": 1,
    "ferry": 2,
}


def route_mode(tags):
    return ROUTE_MODE.get(tags.get("route", ""), 0)
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
        # `out geom` carries each relation's member WAY geometry inline (the real track alignment);
        # `>;out body` then resolves the member nodes with tags (station names).
        f"out geom;>;out body qt;"
    )
    last = None
    for ep in ENDPOINTS:
        try:
            req = urllib.request.Request(ep, data=("data=" + urllib.parse.quote(q)).encode(),
                                         headers={"User-Agent": "transitstory/0.1 (build script)"})
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


# Geometry simplification tolerance (degrees ≈ 30 m). The raw OSM track has a vertex every few
# metres — far denser than the game needs — which bloats the save, the state hash, and (badly) the
# boot-time geometry+buildability rebuild on big networks. Douglas-Peucker collapses near-straight
# runs while keeping every real curve, so the alignment looks identical.
RDP_EPS = 0.0003


def simplify_rdp(pts, eps=RDP_EPS):
    """Ramer-Douglas-Peucker on a (lng,lat) polyline. Iterative (no recursion-depth blowups on the
    long dense OSM ways). Endpoints are always kept, so per-span station anchoring is preserved."""
    n = len(pts)
    if n < 3:
        return pts
    keep = [False] * n
    keep[0] = keep[n - 1] = True
    stack = [(0, n - 1)]
    while stack:
        a, b = stack.pop()
        ax, ay = pts[a]
        bx, by = pts[b]
        dx, dy = bx - ax, by - ay
        l2 = dx * dx + dy * dy
        dmax, idx = 0.0, -1
        for i in range(a + 1, b):
            px, py = pts[i][0] - ax, pts[i][1] - ay
            if l2 == 0:
                d = math.hypot(px, py)
            else:
                t = (px * dx + py * dy) / l2
                d = math.hypot(px - t * dx, py - t * dy)
            if d > dmax:
                dmax, idx = d, i
        if dmax > eps and idx != -1:
            keep[idx] = True
            stack.append((a, idx))
            stack.append((idx, b))
    return [pts[i] for i in range(n) if keep[i]]


def stitch_ways(r):
    """Stitch a route relation's member WAY geometries into one continuous (lng,lat) polyline so an
    imported line follows the REAL track alignment, not a synthesised curve. Ways come in member
    order but in arbitrary direction; chain greedily by matching endpoints (≈25 m tolerance). A gap
    just concatenates (the per-span split tolerates it)."""
    ways = [[(g["lon"], g["lat"]) for g in m["geometry"]]
            for m in r.get("members", [])
            if m.get("type") == "way" and m.get("geometry")]
    ways = [w for w in ways if len(w) >= 2]
    if not ways:
        return []
    TOL2 = (25.0 / 111320.0) ** 2  # ~25 m, in squared degrees (good enough at metro latitudes)

    def near(a, b):
        return (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 <= TOL2

    poly = list(ways[0])
    for w in ways[1:]:
        if near(poly[-1], w[0]):
            poly += w[1:]
        elif near(poly[-1], w[-1]):
            poly += list(reversed(w))[1:]
        elif near(poly[0], w[-1]):
            poly = w[:-1] + poly
        elif near(poly[0], w[0]):
            poly = list(reversed(w))[1:] + poly
        else:
            poly += w  # gap — concatenate; the span split still works off nearest vertices
    return poly


def span_geometry(poly, seq, stations, loop):
    """Split the stitched track polyline into per-span intermediate vertices: geometry[j] is the
    (lng,lat) points strictly BETWEEN station seq[j] and seq[j+1] (the closing span too, for a loop).
    Each station maps to its nearest polyline vertex; the slice is oriented from j to j+1. Returns a
    list aligned with the line's spans, or None if there's no usable polyline (→ straight fallback)."""
    if len(poly) < 2 or len(seq) < 2:
        return None

    def nearest(si):
        p = (stations[si]["lng"], stations[si]["lat"])
        bi, bd = 0, None
        for i, q in enumerate(poly):
            d = (q[0] - p[0]) ** 2 + (q[1] - p[1]) ** 2
            if bd is None or d < bd:
                bd, bi = d, i
        return bi

    idx = [nearest(si) for si in seq]
    pairs = list(zip(seq, seq[1:]))
    if loop:
        pairs.append((seq[-1], seq[0]))
        idx = idx + [idx[0]]
    geom = []
    for j in range(len(pairs)):
        a, b = idx[j], idx[j + 1]
        seg = poly[a + 1:b] if a <= b else list(reversed(poly[b + 1:a]))
        seg = simplify_rdp(seg)  # decimate dense straight runs (keeps curves)
        # Drop a span whose path WILDLY exceeds the straight gap — that's a non-revenue detour
        # (depot/siding) or a bad stitch, not the real alignment (e.g. the DTL into Gali Batu depot,
        # the LRT loops' closing spans). A real curve is ≲2×; >3× is always spurious.
        sa = (stations[pairs[j][0]]["lng"], stations[pairs[j][0]]["lat"])
        sb = (stations[pairs[j][1]]["lng"], stations[pairs[j][1]]["lat"])
        straight = dist_km(sa, sb)
        pts = [sa] + [(x, y) for (x, y) in seg] + [sb]
        plen = sum(dist_km(pts[k], pts[k + 1]) for k in range(len(pts) - 1))
        if straight > 0.05 and plen > 3.0 * straight:
            seg = []  # straight fallback for this span
        geom.append([[round(x, 5), round(y, 5)] for (x, y) in seg])
    # All-empty (stations are the only vertices) ⇒ no real shape to carry.
    return geom if any(g for g in geom) else None


def branch_from_variant(trunk, var):
    """If route variant `var` (a station-index sequence) shares a prefix with `trunk` then continues
    into NEW stations, that continuation is a BRANCH (P3): returns (diverge_at, [branch stops]) where
    diverge_at is the trunk index of the junction. Tries both orientations of `var`. None otherwise —
    so a merely shorter/equal variant never invents a branch. This recovers, e.g., the Circle Line's
    Marina Bay spur: OSM carries it as a `ref=CCL` variant that shares HarbourFront…Promenade with the
    main arc then diverges to Bayfront/Marina Bay."""
    tset = set(trunk)
    for v in (var, list(reversed(var))):
        i = 0
        while i < len(v) and v[i] in tset:
            i += 1
        # v[:i] is shared with the trunk; v[i:] is the would-be branch.
        if i >= 2 and i < len(v):
            tail = v[i:]
            if all(s not in tset for s in tail):  # the whole continuation is new track
                return (trunk.index(v[i - 1]), tail)
    return None


def build(cid, meta):
    data = overpass(meta["bbox"])
    # Clip stops to the city bbox (+~2km): a route relation carries its FULL extent, so a
    # long-distance train / intercity sleeper / international ferry that merely clips the area
    # would otherwise streak hundreds of km across the map. Keeping only in-bbox stops trims each
    # line to its local segment; lines left with <min_stops inside are dropped below.
    cw, cs, ce, cn = meta["bbox"]
    BBOX_MARGIN = 0.02
    nodes, rels = {}, []
    for el in data["elements"]:
        if el["type"] == "node":
            nodes[el["id"]] = el
        elif el["type"] == "relation" and el.get("tags", {}).get("route"):
            rels.append(el)

    # Group route variants by ref|name. We keep ALL variants per line (not just the longest): the
    # longest becomes the trunk, and the others are mined for BRANCHES (a variant that shares a prefix
    # then diverges into new track — e.g. the Circle Line's Marina Bay spur).
    variants = {}
    for r in rels:
        t = r.get("tags", {})
        name = t.get("name") or t.get("ref")
        if not name:
            continue
        # Skip under-construction / proposed routes (incomplete stops -> stub lines).
        if t.get("route") in ("construction", "proposed") or t.get("state") in ("construction", "proposed") \
                or any(k.startswith(("construction", "proposed")) for k in t):
            continue
        if t.get("service") in SKIP_SERVICE:  # heavy-rail intercity/HSR/sleeper -> not urban transit
            continue
        stops = [m["ref"] for m in r.get("members", [])
                 if m["type"] == "node" and str(m.get("role", "")).startswith("stop")]
        if len(stops) < 2:  # fall back to platforms if no stop roles
            stops = [m["ref"] for m in r.get("members", [])
                     if m["type"] == "node" and "platform" in str(m.get("role", ""))]
        if len(stops) < 2:
            continue
        key = t.get("ref") or name
        variants.setdefault(key, []).append((r, stops))

    # Dedup stations by name (merging same-name stops => interchanges).
    stations, idx_by_name = [], {}

    def station_index(node_id):
        nd = nodes.get(node_id)
        if not nd or "lat" not in nd:
            return None
        if not (cw - BBOX_MARGIN <= nd["lon"] <= ce + BBOX_MARGIN
                and cs - BBOX_MARGIN <= nd["lat"] <= cn + BBOX_MARGIN):
            return None  # stop outside the play area -> trims long-distance routes to their local run
        nm = (nd.get("tags", {}).get("name") or nd.get("tags", {}).get("name:en")
              or f"Stop {node_id}")
        if nm in idx_by_name:
            return idx_by_name[nm]
        i = len(stations)
        idx_by_name[nm] = i
        stations.append({"name": nm, "lng": round(nd["lon"], 5), "lat": round(nd["lat"], 5)})
        return i

    def seq_of(stops):
        s, last = [], None
        for nid in stops:
            si = station_index(nid)
            if si is not None and si != last:
                s.append(si)
                last = si
        return s

    lines = []
    for ci, (key, vlist) in enumerate(sorted(variants.items())):
        # Station seq for every variant (this also registers all branch-only stations).
        seqs = [(r, seq_of(stops)) for (r, stops) in vlist]
        seqs = [(r, s) for (r, s) in seqs if len(s) >= 2]
        if not seqs:
            continue
        # Trunk = the longest variant: its tags drive the line, its geometry the trunk shape.
        r, seq = max(seqs, key=lambda rs: len(rs[1]))
        t = r.get("tags", {})
        mode = route_mode(t)
        # Loop detection: tagged roundtrip, or the route returns to its start station.
        loop = t.get("roundtrip") == "yes"
        if len(seq) >= 3 and seq[0] == seq[-1]:
            loop = True
            seq = seq[:-1]
        # Rail needs >=3 stops (2-stop = under-construction / people-mover stub); ferries and
        # buses are legitimately point-to-point, so 2 stops is fine for them.
        min_stops = 2 if mode in (1, 2) else 3
        if len(seq) < min_stops:
            continue
        # Branches: other variants that share a prefix with the trunk then continue into new track.
        # (Out-and-back only — loop lines have no clean single junction in this simple model.)
        branches, seen = [], set()
        if not loop:
            for (vr, vseq) in seqs:
                if vr is r:
                    continue
                b = branch_from_variant(seq, vseq)
                if not b:
                    continue
                dpos, tail = b
                ktail = tuple(tail)
                if ktail in seen:
                    continue
                seen.add(ktail)
                br = {"divergeAt": dpos, "stations": tail}
                # The spur's own real geometry: split this variant's track at [junction, *tail].
                bgeom = span_geometry(stitch_ways(vr), [seq[dpos]] + tail, stations, False)
                if bgeom is not None:
                    br["geometry"] = bgeom
                branches.append(br)
        # Real OSM track alignment, split into per-span intermediate vertices (so the line follows
        # the actual layout instead of a synthesised curve). None ⇒ straight fallback.
        geom = span_geometry(stitch_ways(r), seq, stations, loop)
        line = {
            "name": t.get("name") or t.get("ref") or key,
            "colorHex": norm_colour(t, ci),
            "headwayMin": 10 if t.get("route") == "train" else (4 if mode == 0 else (8 if mode == 1 else 20)),
            "trains": max(2, min(12, len(seq) // 3)),
            "loop": loop,
            "mode": mode,
            "stations": seq,
        }
        if geom is not None:
            line["geometry"] = geom
        if branches:
            line["branches"] = branches
        lines.append(line)

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
