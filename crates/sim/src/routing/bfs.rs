//! The shipped `Router`: minimum-transfer BFS over the line graph. From a station you can ride
//! any serving line to any of that line's stops (one leg), then transfer at a shared station.
//! Returns the fewest-leg path; direct single-line trips fall out as a 1-leg result.
//! Deterministic: index-ordered iteration only (no HashMap iteration).
use super::{Leg, Router};
use crate::ids::{LineId, StationId};
use crate::line::Line;
use std::collections::VecDeque;

/// Min-transfer BFS router. Zero-sized; the seam exists so RAPTOR can replace it later.
#[derive(Clone, Copy, Debug, Default)]
pub struct BfsRouter;

impl Router for BfsRouter {
    fn plan(
        &self,
        lines: &[Line],
        serving: &[Vec<LineId>],
        origin: StationId,
        dest: StationId,
        max_legs: usize,
    ) -> Option<Vec<Leg>> {
        plan_route(lines, serving, origin, dest, max_legs)
    }
}

/// `serving[station]` = lines stopping at that station. `max_legs` bounds transfers.
/// Free function kept for direct use in tests; `BfsRouter` wraps it for the trait seam.
pub fn plan_route(
    lines: &[Line],
    serving: &[Vec<LineId>],
    origin: StationId,
    dest: StationId,
    max_legs: usize,
) -> Option<Vec<Leg>> {
    let n = serving.len();
    let (oi, di) = (origin.index(), dest.index());
    if oi >= n || di >= n || oi == di {
        return None;
    }

    let mut visited = vec![false; n];
    let mut depth = vec![0usize; n];
    let mut parent: Vec<Option<(u32, LineId)>> = vec![None; n]; // station -> (from, via line)
    let mut q: VecDeque<usize> = VecDeque::new();
    visited[oi] = true;
    q.push_back(oi);

    while let Some(x) = q.pop_front() {
        if depth[x] >= max_legs {
            continue;
        }
        for &l in &serving[x] {
            for &y in &lines[l.index()].stops {
                let yi = y.index();
                if visited[yi] {
                    continue;
                }
                visited[yi] = true;
                depth[yi] = depth[x] + 1;
                parent[yi] = Some((x as u32, l));
                if yi == di {
                    return Some(reconstruct(&parent, oi, di));
                }
                q.push_back(yi);
            }
        }
    }
    None
}

fn reconstruct(parent: &[Option<(u32, LineId)>], oi: usize, di: usize) -> Vec<Leg> {
    let mut legs = Vec::new();
    let mut cur = di;
    while let Some((from, via)) = parent[cur] {
        legs.push(Leg {
            line: via,
            board: StationId(from),
            alight: StationId(cur as u32),
        });
        cur = from as usize;
        if cur == oi {
            break;
        }
    }
    legs.reverse();
    legs
}
