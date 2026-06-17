//! Routing seam. `trait Router` is the port the demand model plans trips through. Two impls
//! ship behind it: `RaptorRouter` (the DEFAULT — frequency-based RAPTOR minimising expected
//! travel time over K=`max_legs` rounds) and `BfsRouter` (minimum-transfer BFS, kept as the
//! simple reference + comparison baseline). The upgrade was a clean drop-in exactly as the
//! architecture intends — a new `impl Router` in a sibling module, no change to `World::apply`'s
//! signature or the demand call shape, same `Vec<Leg>` output so `Pax`/board_alight stay thin.
//! Inter-station footpaths now ship: a `footpaths` adjacency lets a router transfer on foot
//! between unconnected lines whose stops are close (legs stay ride-only with a walk GAP). The
//! transfer wait is also phase-aware now (a coordinated 3/8·headway vs the origin's cold
//! headway/2) — an auto-timetable derived purely from headways, no stored departures. Still
//! deferred behind this same seam: real stored departure phases + time-of-day service scaling
//! (both need a non-teleporting variable-fleet dispatch layer first).
use crate::ids::{LineId, StationId};
use crate::line::Line;

mod bfs;
mod raptor;
pub mod segment_graph;
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
/// Footpath adjacency: `footpaths[station]` = nearby stations reachable on foot, each with its
/// walk time (ms). Lets a router transfer between unconnected lines whose stops are close. An
/// empty slice (or empty per-station list) means "no walking" — pure transit routing.
pub type Footpaths = [Vec<(u32, i64)>];

pub trait Router {
    fn plan(
        &self,
        lines: &[Line],
        serving: &[Vec<LineId>],
        footpaths: &Footpaths,
        origin: StationId,
        dest: StationId,
        max_legs: usize,
    ) -> Option<Vec<Leg>>;

    /// One-to-all travel time (ms) from `origin` to every station (`i64::MAX` = unreachable),
    /// used by the demand model to weight destinations by network accessibility. The default
    /// returns an empty vec — "no accessibility data" — so callers fall back to a geometric
    /// model; `RaptorRouter` overrides it with real earliest-arrival labels (near-free, since
    /// RAPTOR computes the whole vector anyway). Must be deterministic (index-ordered) too.
    fn reachable(&self, _lines: &[Line], _serving: &[Vec<LineId>], _footpaths: &Footpaths, _origin: StationId, _max_legs: usize) -> Vec<i64> {
        Vec::new()
    }
}
