import json, pathlib
DATA = pathlib.Path("packages/app/public/data")
# (city id, rough metro population in millions) — order matches network station indices
CITIES = [
    ("singapore", 5.9), ("tokyo", 37.0), ("calgary", 1.6), ("istanbul", 15.5),
    ("manhattan", 18.8), ("dublin", 1.5), ("chicago", 9.5), ("sf", 4.7),
    ("brisbane", 2.6), ("london", 14.8), ("pyongyang", 3.0), ("glasgow", 1.8),
]
nodes = []
for cid, pop in CITIES:
    m = json.load(open(DATA / f"{cid}_city.json"))
    lng, lat = m["originLngLat"]
    nodes.append({"id": cid, "name": m["name"], "lng": lng, "lat": lat, "pop": pop})

# demand grid: one cell per city at its real lon/lat. originWeight = residents, destWeight =
# business/tourism pull (scale pop up so trips actually spawn at the sim's demand rate).
demand = {
    "cellM": 1000.0,
    "bbox": [-180, -60, 180, 75],
    "cells": [{"lon": n["lng"], "lat": n["lat"], "originWeight": n["pop"] * 6.0, "destWeight": n["pop"] * 6.0} for n in nodes],
}
json.dump(demand, open(DATA / "globe_demand.json", "w"))

# starting network: 12 cities as stations + a few flagship air routes (mode 3) so planes fly now.
idx = {n["id"]: i for i, n in enumerate(nodes)}
def air(name, a, b):
    return {"name": name, "colorHex": "cc79a7", "headwayMin": 25, "trains": 2, "mode": 3,
            "stations": [idx[a], idx[b]]}
network = {
    "cityId": "globe", "name": "World",
    "stations": [{"name": n["name"], "lng": n["lng"], "lat": n["lat"]} for n in nodes],
    "lines": [
        air("LON–NYC", "london", "manhattan"),
        air("TYO–SIN", "tokyo", "singapore"),
        air("NYC–CHI", "manhattan", "chicago"),
        air("LON–DUB", "london", "dublin"),
    ],
}
json.dump(network, open(DATA / "globe_network.json", "w"))

# manifest: world origin + view; no buildability (air ignores terrain).
manifest = {
    "id": "globe", "name": "World", "originLngLat": [10.0, 35.0],
    "bbox": [-180, -60, 180, 75], "center": [10.0, 35.0], "zoom": 1.6, "seed": 42,
    "demandGridPath": "/data/globe_demand.json", "networkPath": "/data/globe_network.json",
}
json.dump(manifest, open(DATA / "globe_city.json", "w"), indent=2)
print("wrote globe_city.json, globe_demand.json (%d cells), globe_network.json (%d stations, %d routes)"
      % (len(demand["cells"]), len(network["stations"]), len(network["lines"])))
