//! The decadence area-control field's STATIC topology (fantasy S10a). The spatial corruption tide
//! creeps over the baked continent toward the capital; this module derives the substrate it spreads
//! over — the hex-cell domain, its adjacency, the cheap creep gradient (hop distance to the capital),
//! and the far-edge reservoir seed — ONCE at construction from `CityData` (the buildability raster +
//! the baked capital). It is a pure function of `CityData`, reconstructible on replay exactly like
//! `World::build_lookup`, so it is **NOT hashed** (never enters `Canonical`). The dynamic, hashed
//! per-cell decadence values (the creeping tide + PURGE/DIFFUSE CA) layer on top in S10b; this is only
//! the board they move on. Empty for transit and for demo-arcadia worlds with no baked terrain — so it
//! is golden-neutral (it adds no state to the hash; S10b's field is the re-pin).
//!
//! Determinism: cells are sorted by axial `(q, r)` (index-stable order), adjacency + BFS iterate in that
//! order, and every float is confined to `hexgrid` (immediately quantised to integer axials), so two
//! builds of the same `CityData` are bit-identical (`tests/decadence_field.rs` pins it).
use crate::city::CityData;
use crate::geo_local::PointMm;
use crate::hexgrid::{self, Axial};
use std::collections::VecDeque;

/// How many far-edge cells seed the tide (the "reservoir"). A small seed band — the diffusion CA spreads
/// it inward. Capped so a huge continent doesn't seed a wall of corruption (the S10b dynamic cap is
/// separate; this only bounds the initial seed set).
const RESERVOIR_SEEDS: usize = 8;

/// Is a terrain class part of the CA domain — passable land the corruption spreads over? Class codes
/// (`build_world.py`): 4=WATER, 6=MOUNTAIN (impassable ridge), 7=HILL, 8=FOREST, 9=LEY, 10=PLAIN. The
/// tide creeps over PASSABLE land only (not water, not the impassable ridge), routing around mountains
/// through the carved passes — matching the bake's loseability guarantee (the reservoir reaches the
/// capital over passable land). 0 (the `_ => Open` default) is treated as non-domain.
fn is_passable(c: u8) -> bool {
    matches!(c, 7 | 8 | 9 | 10)
}

/// Static topology of the decadence CA: the cell domain (axial, index-ordered), CSR hex adjacency, the
/// creep distance-to-capital gradient, the capital cell, and the reservoir seed. NOT hashed.
#[derive(Clone, Default)]
pub struct DecadenceField {
    /// Domain cells as axial `(q, r)`, sorted ⇒ deterministic, index-stable order. Index = CellId.
    pub cells: Vec<Axial>,
    /// CSR hex adjacency: cell `i`'s present neighbours are `nbr_flat[nbr_start[i]..nbr_start[i + 1]]`.
    pub nbr_start: Vec<u32>,
    pub nbr_flat: Vec<u32>,
    /// Hop distance from the capital over the domain (BFS); `u32::MAX` = unreachable. The cheap creep
    /// gradient toward the capital — computed once, never per tick.
    pub dist_to_capital: Vec<u32>,
    /// The capital cell (the lose target), if the baked capital falls on / near a domain cell.
    pub capital: Option<u32>,
    /// Tide-origin seed cells: the cells farthest from the capital (the "far edge"), reachable to it.
    pub reservoir: Vec<u32>,
}

impl DecadenceField {
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    /// Cell `i`'s in-domain hex neighbours (CSR slice).
    pub fn neighbors(&self, i: u32) -> &[u32] {
        let s = self.nbr_start[i as usize] as usize;
        let e = self.nbr_start[i as usize + 1] as usize;
        &self.nbr_flat[s..e]
    }

    /// Build the static CA topology from the baked terrain. Empty (no CA) when there is no terrain
    /// (transit / demo arcadia) or no `grid_cell_mm`. Deterministic.
    pub fn build(city: &CityData) -> Self {
        let size = city.grid_cell_mm;
        if size <= 0 || city.buildability.cells.is_empty() {
            return Self::default();
        }
        // 1. Domain = passable land cells, reinterpreted as axial via the SAME hexgrid transform the
        //    bake quantised them with (so a baked hex centre maps back to its own cell). Sort + dedup
        //    for a deterministic, index-stable cell order.
        let mut cells: Vec<Axial> = city
            .buildability
            .cells
            .iter()
            .filter(|c| is_passable(c.c))
            .map(|c| hexgrid::axial_of(PointMm::new(c.x_mm, c.y_mm), size))
            .collect();
        cells.sort_unstable();
        cells.dedup();
        if cells.is_empty() {
            return Self::default();
        }
        // Construction-only lookup (queried, never iterated — order-independent, like `build_lookup`).
        let index: rustc_hash::FxHashMap<Axial, u32> =
            cells.iter().enumerate().map(|(i, &a)| (a, i as u32)).collect();

        // 2. CSR adjacency over the 6 pointy-top hex neighbours present in the domain.
        const DIRS: [Axial; 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];
        let mut nbr_start: Vec<u32> = Vec::with_capacity(cells.len() + 1);
        let mut nbr_flat: Vec<u32> = Vec::new();
        nbr_start.push(0);
        for &(q, r) in &cells {
            for &(dq, dr) in &DIRS {
                if let Some(&j) = index.get(&(q + dq, r + dr)) {
                    nbr_flat.push(j);
                }
            }
            nbr_start.push(nbr_flat.len() as u32);
        }

        // 3. Capital cell: a baked world (a non-empty domain ⇒ we are here) always has one — the seat
        //    the tide races toward. The baked capital mm → axial; if that exact cell isn't in the domain
        //    (a coarse bin landed it just off a passable cell), snap to the nearest domain cell (index-
        //    ordered argmin over hex distance ⇒ deterministic). `(0, 0)` is a legitimate origin cell, NOT
        //    an "unset" sentinel — non-baked worlds already returned empty above.
        let ca = hexgrid::axial_of(PointMm::new(city.capital_x_mm, city.capital_y_mm), size);
        let capital = index.get(&ca).copied().or_else(|| {
            let mut best: Option<(i64, u32)> = None;
            for (i, &cell) in cells.iter().enumerate() {
                let d = hexgrid::distance(cell, ca);
                if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                    best = Some((d, i as u32));
                }
            }
            best.map(|(_, i)| i)
        });

        // 4. Creep distance: BFS from the capital over the domain (unweighted ⇒ unique shortest-path
        //    distances regardless of queue order). `u32::MAX` = unreachable (walled off by water/ridge).
        let mut dist_to_capital = vec![u32::MAX; cells.len()];
        if let Some(cap) = capital {
            dist_to_capital[cap as usize] = 0;
            let mut queue: VecDeque<u32> = VecDeque::new();
            queue.push_back(cap);
            while let Some(c) = queue.pop_front() {
                let nd = dist_to_capital[c as usize] + 1;
                let s = nbr_start[c as usize] as usize;
                let e = nbr_start[c as usize + 1] as usize;
                for &n in &nbr_flat[s..e] {
                    if dist_to_capital[n as usize] == u32::MAX {
                        dist_to_capital[n as usize] = nd;
                        queue.push_back(n);
                    }
                }
            }
        }

        // 5. Reservoir = the farthest REACHABLE cells (the far edge opposite the capital — the tide
        //    origin). Sort by (distance desc, axial) for a deterministic seed band, take the top few.
        //    Empty if nothing is reachable (no capital / disconnected) — then there is no winnable tide.
        let mut reachable: Vec<u32> = (0..cells.len() as u32)
            .filter(|&i| dist_to_capital[i as usize] != u32::MAX && dist_to_capital[i as usize] > 0)
            .collect();
        reachable.sort_by(|&a, &b| {
            dist_to_capital[b as usize]
                .cmp(&dist_to_capital[a as usize])
                .then(cells[a as usize].cmp(&cells[b as usize]))
        });
        reachable.truncate(RESERVOIR_SEEDS);
        let reservoir = reachable;

        DecadenceField { cells, nbr_start, nbr_flat, dist_to_capital, capital, reservoir }
    }
}
