//! Pure binary-search helpers over **monotone-ascending** arc-length arrays.
//!
//! These replace the per-train, per-tick LINEAR scans in the move hot path (`Path::span_of` /
//! `speed_cap_at` / `point_at` / `heading_at` and `vehicle::next_stop_index`) — the optimization
//! commit `90854e7` named as deferred (it was reverted because the boundary semantics are subtle).
//! Each helper REPLICATES the exact `<` / `<=` / `> s+1` gate of the linear scan it replaces, so the
//! swap is behaviour-preserving: `tests/geom_search.rs` proves bit-identical equivalence against a
//! linear reference over random + edge arrays (negative, on-gate, below-first, above-last, equal
//! adjacent arclens, len 0/1/2), and the determinism goldens are the final proof.
//!
//! Determinism: pure `i64`, no float, no allocation, no HashMap — `slice::partition_point` is an
//! index-ordered binary search over a sorted slice, so it stays bit-for-bit deterministic.

/// Inter-stop **span index** containing forward arc-length `s` — the binary form of
/// `Path::span_of`'s linear `for j in 1..len { if s < arc[j] { return j-1 } } len-2`.
///
/// `arc` is the per-stop arclength array (`stop_arclen_mm`), monotone ascending. Returns the largest
/// span index in `0..=len-2`. Strict upper gate (`s < arc[j]`), matching the original.
#[inline]
pub fn span_index(arc: &[i64], s: i64) -> usize {
    if arc.len() < 2 {
        return 0;
    }
    // First j>=1 with arc[j] > s (i.e. NOT arc[j] <= s); `len` if none. partition_point over arc[1..]
    // counts the leading elements <= s, so `1 + that` is the first index (>=1) with arc[j] > s.
    let j = 1 + arc[1..].partition_point(|&a| a <= s);
    if j >= arc.len() {
        arc.len() - 2
    } else {
        j - 1
    }
}

/// Upper **bracket index** `i` such that `s <= arc[i]` first holds (i.e. first `i>=1` with
/// `arc[i] >= s`) — the binary form of the linear `for i in 1..len { if s <= arc[i] { .. } }` bracket
/// used by `speed_cap_at` / `point_at` / `heading_at`. Returns `arc.len()` when no such `i` exists
/// (s past the end) so the caller takes its "last" fallthrough. `arc` (`arclen_mm`) is monotone
/// ascending; callers clamp `s` to `[0, length]` exactly as before.
#[inline]
pub fn upper_bracket(arc: &[i64], s: i64) -> usize {
    if arc.len() < 2 {
        return arc.len();
    }
    // First i>=1 with arc[i] >= s. partition_point over arc[1..] counts leading elements < s.
    1 + arc[1..].partition_point(|&a| a < s)
}

/// Index of the **next stop** in the travel direction — the binary form of `vehicle::next_stop_index`'s
/// linear scan (`dir>0`: first `i` with `arc[i] > s+1`, else `len-1`; `dir<=0`: last `i` with
/// `arc[i] < s-1`, else `0`). `arc` is the stop arclength array, monotone ascending.
#[inline]
pub fn next_stop_index(arc: &[i64], s: i64, dir: i64) -> usize {
    if dir > 0 {
        // First i with arc[i] > s+1 (NOT arc[i] <= s+1); `len-1` if none.
        let i = arc.partition_point(|&a| a <= s + 1);
        if i >= arc.len() {
            arc.len().saturating_sub(1)
        } else {
            i
        }
    } else {
        // Last i with arc[i] < s-1; `0` if none. partition_point counts leading elements < s-1, so the
        // last such index is `pp - 1` (and `0` when pp == 0, matching the original's `else 0`).
        let pp = arc.partition_point(|&a| a < s - 1);
        pp.saturating_sub(1)
    }
}
