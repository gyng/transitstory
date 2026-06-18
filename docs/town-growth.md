# Organic / multi-cell town growth — design & scope (#23)

**Status:** scoping (no code yet). **Owner ask (2026-06-19):** "towns should grow organically, maybe even
multicell (so capital might be a few cells?)". This scopes a town **footprint** that starts multi-cell for
the capital and **grows with prosperity**, with the determinism + phasing up front so we don't half-build a
sim feature into the hot path.

---

## 1. What exists today

- A town is a single baked **station** (faction 0) at one hex cell, carrying `town_value` (siege HP /
  conquest reward), a supply `chain` (bread/arms), and `needs`. It renders as one dot + (#3d) one lowpoly
  depot building + a gilt nameplate (★/⌂ rank glyph, #22).
- Prosperity already flows: a supplied town consumes its inputs (the Forge-Line) and mints tribute; the
  stats slice carries per-station throughput (`boardings`/`serving`) and the town's value/resistance.
- The map is a hex lattice (`grid_cell_mm`); the 3D diorama (depots/trees) instances meshes per node, and
  the catchment now tiles on that lattice (#19).

So a town is **one cell, fixed size**. The ask is for it to occupy (and grow into) **several** cells.

---

## 2. The crux fork: cosmetic vs gameplay footprint

**Does a bigger town DO more, or just LOOK bigger?**

- **Cosmetic (recommended first):** the footprint is a *render* concept — a cluster of buildings sprawling
  across the town's cells, sized by a prosperity readout. The sim is unchanged (one station, same value, same
  catchment). **No hashed state, no re-pin, no balance impact** — pure visible payoff. This is almost all of
  what the ask asks for ("grow organically… capital a few cells").
- **Gameplay (a later phase):** a bigger town has more catchment / a higher conquest value / more garrison.
  That's **hashed state** (the footprint feeds capture math) ⇒ a deliberate re-pin **and** a balance pass
  (a growing capital changes the win window). Bigger blast radius; defer until the cosmetic version is in
  hand and we know we want the depth.

**Recommendation:** ship the **cosmetic** footprint first (Phase TG1). It delivers the visual the owner
described with zero determinism/balance risk; gameplay-coupling (TG2) is an opt-in follow-on.

---

## 3. The footprint (cosmetic model)

- A town has a **size** `s ∈ [1..S_max]` derived (not stored) from a **prosperity proxy** — cumulative
  supply delivered to it (or its `town_value` / held-duration), read off the ~3 Hz stats slice. The capital
  seeds at a higher base size (`★` = a few cells) so it reads as the realm's seat from the start.
- `size → footprint cells`: the town's hex cell + concentric **rings** of neighbours (hexgrid ring 0 = the
  centre, ring 1 = 6 neighbours, …). `s=1` → just the centre; `s=2` → +inner ring; the capital → ring 2.
  Pure hex math (`hexgrid` neighbours), deterministic from the centre + size, no new sim state.
- Render: the **3D depot** layer instances a building on every footprint cell (a denser cluster at the
  centre, sparser/lower on the outer ring — a town tapering into hamlets), reusing the existing
  `stationMesh` + the per-instance tint. The nameplate stays anchored to the centre. The 3D-diorama LOD +
  the iso tilt (#20) already make a multi-cell cluster read as a town.
- **Organic growth = the size readout rising as supply accrues**, animated as a CSS/scale tween off the
  3 Hz slice (like the gauge/dot tweens) — buildings fade/scale in as the town prospers. No new sim ticks
  (AGENTS: juice rides the stable-identity render path, not per-frame rebuilds).

---

## 4. Determinism plan
- **TG1 is render-only.** The footprint is *derived* from an existing readout (cumulative supply / value) +
  pure hex math — it never enters `Canonical`, never re-pins, and transit/flat worlds are untouched (a town
  with no prosperity proxy is size 1 = today's single cell). The capital's higher base size is a render
  constant.
- **TG2 (if pursued)** would make the footprint authoritative (catchment/value scale with size) ⇒ a hashed
  `town_size`/footprint, an intentional re-pin, and a `balance.rs` winnability re-check. Out of scope for the
  first cut; called out so we don't drift into it accidentally.

## 5. Seams (AGENTS-compliant)
- Render: extend the depot `stationMeshLayer` to emit one instance per footprint cell (a `footprintCells()`
  helper on `Game`, from the town centre + the size readout via `hexgrid` rings). One new helper; no new
  layer, no new sim port.
- The prosperity proxy is read from the existing `stats`/`perStation` slice — no new bridge field for TG1.
- TG2 (deferred) would add a `CityData`/`World` `town_size` + the catchment/value coupling behind the
  existing town ports — never a new mutator.

## 6. Phases
1. **TG1 — cosmetic multi-cell footprint:** capital seeds multi-cell; every town's building cluster grows
   with its prosperity readout (3 Hz tween). Render-only, golden-neutral. *(The whole visible ask.)*
2. **TG2 — gameplay coupling (opt-in, later):** footprint feeds catchment/value/garrison; hashed; one
   re-pin + a balance pass.

## 7. Open questions for the owner
1. **Cosmetic or gameplay?** Recommend cosmetic first (TG1) — zero balance risk, all the visual. Confirm
   before any TG2.
2. **Growth driver:** cumulative supply delivered (ties growth to the player feeding the town) vs held-
   duration vs `town_value`? (Recommend cumulative supply — it rewards the supply loop.)
3. **Max size / capital base:** how big does a thriving town get (S_max rings), and how many cells is the
   capital's seat (2 rings ≈ 19 cells, or just centre+1 ring ≈ 7)?
4. **Does conquering a town freeze/raze its growth** (a captured town stops sprawling, or keeps growing
   under your supply)? Pure-cosmetic answer is easy; matters more if TG2.

**Recommendation:** build **TG1** (cosmetic, render-only) when prioritized — it's the satisfying visible
change with no determinism/balance cost; hold TG2 until the cosmetic version proves the feel.
