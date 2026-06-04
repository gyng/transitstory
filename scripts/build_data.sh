#!/usr/bin/env bash
#
# build_data.sh — bake all committed city-data artifacts for an area, in one command.
#
# A "city" is an entry in scripts/city_demand_config.json (its bbox + demand centres).
# This orchestrates the three per-stage builders, all driven by that one config:
#
#   1. build_demand.py        synthetic demand grid   OFFLINE · deterministic (seeded)
#   2. build_networks.py      real lines from OSM      ONLINE  · Overpass · keep-on-fail
#   3. build_buildability.py  land-class grid from OSM ONLINE  · Overpass · keep-on-fail
#
# Outputs (committed) land in packages/app/public/data/:
#   <id>_demand.json · networks/<id>.json · <id>_buildability.json
#
# Determinism: stage 1 is pure + seeded (no network). Stages 2-3 hit the Overpass API
# and, on any failure/timeout, KEEP the previously committed JSON — so a network blip
# never breaks the game (it just ships slightly staler OSM). There is no runtime Overpass;
# the game only ever reads these committed files (see AGENTS.md: the CityData seam).
#
# Add a new area: add a bbox + jobCenters/homeCenters block to city_demand_config.json,
# run `scripts/build_data.sh <id>`, generate the manifest with `scripts/make_manifest.py <id>`
# (it derives origin/center/zoom/seed from the bbox), then add a CITIES entry in
# packages/app/src/sim/cities.ts. See docs/osm-data.md for what each OSM layer contains.
#
# Usage:
#   scripts/build_data.sh                  # all cities in city_demand_config.json
#   scripts/build_data.sh istanbul dublin  # just these
#   DEMAND_ONLY=1 scripts/build_data.sh    # skip the two online OSM stages
#
set -euo pipefail
cd "$(dirname "$0")/.."

# Pass the requested ids (if any) through to every builder. The "${arr[@]+...}" guard
# keeps an empty array safe under `set -u` (older bash) — empty means "all cities".
CITIES=("$@")
ids=("${CITIES[@]+${CITIES[@]}}")

run() { echo "▸ python3 $*"; python3 "$@"; }

echo "══ 1/3  demand  (synthetic · offline · deterministic) ════════════════════"
run scripts/build_demand.py "${ids[@]+${ids[@]}}"

if [ "${DEMAND_ONLY:-0}" = "1" ]; then
  echo "✓ demand baked (DEMAND_ONLY set — skipped the online OSM stages)"
  exit 0
fi

echo "══ 2/3  networks  (real lines · OSM Overpass · keep-on-fail) ══════════════"
run scripts/build_networks.py "${ids[@]+${ids[@]}}"

echo "══ 3/3  buildability  (land classes · OSM Overpass · keep-on-fail) ════════"
run scripts/build_buildability.py "${ids[@]+${ids[@]}}"

echo "✓ bake complete — committed JSON in packages/app/public/data/"
