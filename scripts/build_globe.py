#!/usr/bin/env python3
"""Generate the globe (global-airline) data: world cities as air nodes + flagship routes.
Pure content — the sim runs it as just another CityData (cities=stations, AIR routes=lines).
Cities carry only lng/lat + a rough metro population; they need no per-city detail (you don't
drill into them — they're air destinations). Re-runnable + deterministic."""
import json, pathlib
DATA = pathlib.Path(__file__).resolve().parent.parent / "packages/app/public/data"

# (name, lng, lat, rough metro population in millions). A spread of major world hubs.
CITIES = [
    ("Singapore", 103.82, 1.35, 5.9), ("Tokyo", 139.69, 35.68, 37.0),
    ("Calgary", -114.07, 51.05, 1.6), ("Istanbul", 28.98, 41.01, 15.5),
    ("New York", -74.01, 40.71, 18.8), ("Dublin", -6.26, 53.35, 1.5),
    ("Chicago", -87.63, 41.88, 9.5), ("San Francisco", -122.42, 37.77, 4.7),
    ("Brisbane", 153.03, -27.47, 2.6), ("London", -0.13, 51.51, 14.8),
    ("Pyongyang", 125.76, 39.04, 3.0), ("Glasgow", -4.25, 55.86, 1.8),
    ("Los Angeles", -118.24, 34.05, 12.5), ("Paris", 2.35, 48.86, 11.1),
    ("Frankfurt", 8.68, 50.11, 2.3), ("Dubai", 55.27, 25.20, 3.5),
    ("Sydney", 151.21, -33.87, 5.3), ("Beijing", 116.41, 39.90, 21.5),
    ("Shanghai", 121.47, 31.23, 28.5), ("Delhi", 77.21, 28.61, 32.0),
    ("Mumbai", 72.88, 19.08, 20.7), ("São Paulo", -46.63, -23.55, 22.4),
    ("Mexico City", -99.13, 19.43, 21.8), ("Toronto", -79.38, 43.65, 6.4),
    ("Moscow", 37.62, 55.76, 12.6), ("Cairo", 31.24, 30.04, 21.3),
    ("Johannesburg", 28.05, -26.20, 6.0), ("Hong Kong", 114.17, 22.32, 7.5),
    ("Bangkok", 100.50, 13.76, 10.7),
]
idx = {n: i for i, (n, *_ ) in enumerate(CITIES)}

# demand grid: one cell per city. originWeight=residents, destWeight=business/tourism pull.
demand = {"cellM": 1000.0, "bbox": [-180, -60, 180, 75],
          "cells": [{"lon": lng, "lat": lat, "originWeight": pop * 6.0, "destWeight": pop * 6.0}
                    for (n, lng, lat, pop) in CITIES]}
json.dump(demand, open(DATA / "globe_demand.json", "w"))

def air(a, b):
    return {"name": f"{a[:3].upper()}–{b[:3].upper()}", "colorHex": "cc79a7",
            "headwayMin": 25, "trains": 2, "mode": 3, "stations": [idx[a], idx[b]]}
ROUTES = [("London","New York"),("Tokyo","Singapore"),("New York","Chicago"),("London","Dublin"),
          ("Dubai","London"),("Singapore","Sydney"),("Los Angeles","Tokyo"),("Paris","New York"),
          ("Hong Kong","Singapore"),("Frankfurt","New York"),("Delhi","Dubai"),("São Paulo","New York")]
network = {"cityId": "globe", "name": "World",
           "stations": [{"name": n, "lng": lng, "lat": lat} for (n, lng, lat, _) in CITIES],
           "lines": [air(a, b) for a, b in ROUTES]}
json.dump(network, open(DATA / "globe_network.json", "w"))

manifest = {"id": "globe", "name": "World", "originLngLat": [10.0, 35.0],
            "bbox": [-180, -60, 180, 75], "center": [10.0, 35.0], "zoom": 1.6, "seed": 42,
            "demandGridPath": "/data/globe_demand.json", "networkPath": "/data/globe_network.json"}
json.dump(manifest, open(DATA / "globe_city.json", "w"), indent=2)
print(f"globe: {len(CITIES)} cities, {len(ROUTES)} starter routes")
