//! Routing seam. `trait Router` is the port the demand model plans trips through; `BfsRouter`
//! (minimum-transfer BFS over the line graph) is the shipped impl. A future RAPTOR (K>1 rounds
//! + footpaths + frequency-aware arrival times) slots in as a NEW `impl Router` in a sibling
//! module — with no change to `World::apply`'s signature or the demand call shape (AGENTS
//! architecture: aspirational systems attach behind the existing trait, not by a core rewrite).
use crate::ids::{LineId, StationId};
use crate::line::Line;

mod bfs;
pub use bfs::{plan_route, BfsRouter};

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
}
