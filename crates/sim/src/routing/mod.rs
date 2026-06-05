//! Routing seam. `trait Router` is the port the demand model plans trips through. Two impls
//! ship behind it: `RaptorRouter` (the DEFAULT — frequency-based RAPTOR minimising expected
//! travel time over K=`max_legs` rounds) and `BfsRouter` (minimum-transfer BFS, kept as the
//! simple reference + comparison baseline). The upgrade was a clean drop-in exactly as the
//! architecture intends — a new `impl Router` in a sibling module, no change to `World::apply`'s
//! signature or the demand call shape, same `Vec<Leg>` output so `Pax`/board_alight are untouched.
//! Still deferred behind this same seam: inter-station footpaths and a real departure timetable.
use crate::ids::{LineId, StationId};
use crate::line::Line;

mod bfs;
mod raptor;
pub use bfs::{plan_route, BfsRouter};
pub use raptor::RaptorRouter;

/// Default max legs (transfers + 1) a routed trip may use when a city doesn't specify one.
pub const DEFAULT_MAX_LEGS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub line: LineId,
    pub board: StationId,
    pub alight: StationId,
}

/// A routing strategy: plan an ordered sequence of legs from `origin` to `dest`, or `None` if
/// unreachable within `max_legs`. `serving[station]` = lines stopping at that station.
/// Implementations MUST be deterministic (index-ordered iteration only) — the determinism gate
/// depends on it.
pub trait Router {
    fn plan(
        &self,
        lines: &[Line],
        serving: &[Vec<LineId>],
        origin: StationId,
        dest: StationId,
        max_legs: usize,
    ) -> Option<Vec<Leg>>;

    /// One-to-all travel time (ms) from `origin` to every station (`i64::MAX` = unreachable),
    /// used by the demand model to weight destinations by network accessibility. The default
    /// returns an empty vec — "no accessibility data" — so callers fall back to a geometric
    /// model; `RaptorRouter` overrides it with real earliest-arrival labels (near-free, since
    /// RAPTOR computes the whole vector anyway). Must be deterministic (index-ordered) too.
    fn reachable(&self, _lines: &[Line], _serving: &[Vec<LineId>], _origin: StationId, _max_legs: usize) -> Vec<i64> {
        Vec::new()
    }
}
