//! Equivalence property test for the binary-search geometry helpers (`crates/sim/src/geom.rs`).
//!
//! Proves each helper is BIT-IDENTICAL to the LINEAR scan it replaced, over random + adversarial
//! monotone-ascending arrays (negative starts, EQUAL adjacent arclens / zero-length spans, len 0/1/2)
//! and exhaustive query points (every arclen value ±1, below first, above last, 0, extremes). This is
//! exactly the equivalence proof the deferred binary-search optimization (commit `90854e7`) was
//! reverted for lacking. The determinism goldens are the final proof that the swap is behaviour-neutral
//! in the move hot path; this test is the unit-level guarantee that feeds them.
use sim::{geom, PointMm, Path};

// --- linear REFERENCE implementations: verbatim copies of the ORIGINAL scans -----------------------
fn lin_span(arc: &[i64], s: i64) -> usize {
    if arc.len() < 2 {
        return 0;
    }
    for j in 1..arc.len() {
        if s < arc[j] {
            return j - 1;
        }
    }
    arc.len() - 2
}
/// The original bracket: first `i` in `1..len` with `s <= arc[i]`, else `len`.
fn lin_bracket(arc: &[i64], s: i64) -> usize {
    for i in 1..arc.len() {
        if s <= arc[i] {
            return i;
        }
    }
    arc.len()
}
fn lin_next_stop(arc: &[i64], s: i64, dir: i64) -> usize {
    if dir > 0 {
        for i in 0..arc.len() {
            if arc[i] > s + 1 {
                return i;
            }
        }
        arc.len().saturating_sub(1)
    } else {
        for i in (0..arc.len()).rev() {
            if arc[i] < s - 1 {
                return i;
            }
        }
        0
    }
}

// Tiny deterministic xorshift (test-local; the determinism of `crates/sim/src` is untouched by what a
// test uses for its OWN data generation).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % ((hi - lo) as u64)) as i64
    }
}

/// A monotone-ascending arclen array, possibly with EQUAL adjacent values (zero-length spans) and a
/// possibly-negative start — the adversarial shapes the original revert worried about.
fn gen_monotone(rng: &mut Rng, len: usize) -> Vec<i64> {
    let mut v = Vec::with_capacity(len);
    let mut cur = rng.range(-1000, 1000);
    for _ in 0..len {
        v.push(cur);
        let step = match rng.next() % 5 {
            0 | 1 => 0,                 // equal-adjacent (zero-length span)
            2 => 1,                     // 1-mm span
            3 => rng.range(2, 50),      // small
            _ => rng.range(50, 5000),   // large
        };
        cur += step;
    }
    v
}

/// Like `gen_monotone` but starts at 0 — the real `arclen_mm`/`stop_arclen_mm` invariant
/// (`arclen_mm[0] == 0`, cumulative non-decreasing). `point_at`/`speed_cap_at` clamp `s` to
/// `[0, length_mm()]`, so feeding all-negative arrays (which never occur in the sim) would make the
/// clamp's `max < min` panic — a property of the test data, not the code under test.
fn gen_arclen0(rng: &mut Rng, len: usize) -> Vec<i64> {
    let mut v = Vec::with_capacity(len);
    let mut cur = 0i64;
    for _ in 0..len {
        v.push(cur);
        let step = match rng.next() % 5 {
            0 | 1 => 0,
            2 => 1,
            3 => rng.range(2, 50),
            _ => rng.range(50, 5000),
        };
        cur += step;
    }
    v
}

/// Query points for a given array: every value ±1, below first, above last, 0, and extremes.
fn queries(arc: &[i64], rng: &mut Rng) -> Vec<i64> {
    let mut q = vec![i64::MIN / 2, -1, 0, 1, i64::MAX / 2];
    if let (Some(&lo), Some(&hi)) = (arc.first(), arc.last()) {
        q.extend([lo - 1, lo, lo + 1, hi - 1, hi, hi + 1]);
        for _ in 0..8 {
            q.push(rng.range(lo - 10, hi + 10));
        }
    }
    for &v in arc {
        q.extend([v - 1, v, v + 1]);
    }
    q
}

#[test]
fn span_index_matches_linear() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for len in 0..14 {
        for _ in 0..400 {
            let arc = gen_monotone(&mut rng, len);
            for s in queries(&arc, &mut rng) {
                assert_eq!(geom::span_index(&arc, s), lin_span(&arc, s), "span arc={arc:?} s={s}");
            }
        }
    }
}

#[test]
fn upper_bracket_matches_linear() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    for len in 0..14 {
        for _ in 0..400 {
            let arc = gen_monotone(&mut rng, len);
            for s in queries(&arc, &mut rng) {
                assert_eq!(geom::upper_bracket(&arc, s), lin_bracket(&arc, s), "bracket arc={arc:?} s={s}");
            }
        }
    }
}

#[test]
fn next_stop_index_matches_linear() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    for len in 0..14 {
        for _ in 0..400 {
            let arc = gen_monotone(&mut rng, len);
            for s in queries(&arc, &mut rng) {
                for dir in [-1i64, 0, 1] {
                    assert_eq!(
                        geom::next_stop_index(&arc, s, dir),
                        lin_next_stop(&arc, s, dir),
                        "next arc={arc:?} s={s} dir={dir}"
                    );
                }
            }
        }
    }
}

// --- Path method wiring: span_of / speed_cap_at / point_at use the helpers WITH the original guards --
fn make_path(stop_arc: Vec<i64>, arclen: Vec<i64>, caps: Vec<i64>) -> Path {
    let polyline: Vec<PointMm> = arclen.iter().map(|&a| PointMm { x_mm: a, y_mm: a / 2 }).collect();
    Path { stop_arclen_mm: stop_arc, arclen_mm: arclen, speed_cap_mm_s: caps, polyline, ..Default::default() }
}

#[test]
fn path_span_of_matches_linear_reference() {
    let mut rng = Rng(0xF00D_F00D_F00D_F00D);
    for _ in 0..3000 {
        let len = (rng.next() % 12) as usize;
        let stop_arc = gen_monotone(&mut rng, len);
        let p = make_path(stop_arc.clone(), vec![], vec![]);
        for s in queries(&stop_arc, &mut rng) {
            assert_eq!(p.span_of(s), lin_span(&stop_arc, s), "stop_arc={stop_arc:?} s={s}");
        }
    }
}

#[test]
fn path_speed_cap_at_matches_linear_reference() {
    let mut rng = Rng(0x0BAD_C0DE_0BAD_C0DE);
    for _ in 0..3000 {
        let len = (rng.next() % 12) as usize;
        let arclen = gen_arclen0(&mut rng, len);
        let caps: Vec<i64> = (0..len).map(|_| rng.range(1, 1_000_000)).collect();
        let p = make_path(vec![], arclen.clone(), caps.clone());
        let length = arclen.last().copied().unwrap_or(0);
        for s in queries(&arclen, &mut rng) {
            let want = if caps.len() < 2 {
                i64::MAX
            } else {
                let sc = s.clamp(0, length);
                let i = lin_bracket(&arclen, sc);
                if i >= arclen.len() { *caps.last().unwrap_or(&i64::MAX) } else { caps[i - 1].min(caps[i]) }
            };
            assert_eq!(p.speed_cap_at(s), want, "arclen={arclen:?} caps={caps:?} s={s}");
        }
    }
}

#[test]
fn path_point_at_matches_linear_reference() {
    let mut rng = Rng(0xABCD_EF01_2345_6789);
    for _ in 0..3000 {
        let len = (rng.next() % 12) as usize;
        let arclen = gen_arclen0(&mut rng, len);
        let p = make_path(vec![], arclen.clone(), vec![]);
        let poly: Vec<PointMm> = arclen.iter().map(|&a| PointMm { x_mm: a, y_mm: a / 2 }).collect();
        let length = arclen.last().copied().unwrap_or(0);
        for s in queries(&arclen, &mut rng) {
            let want: (i64, i64) = if poly.len() < 2 {
                if poly.is_empty() { (0, 0) } else { (poly[0].x_mm, poly[0].y_mm) }
            } else {
                let sc = s.clamp(0, length);
                let last = poly[poly.len() - 1];
                let mut out = (last.x_mm, last.y_mm);
                for i in 1..arclen.len() {
                    if sc <= arclen[i] {
                        let seg_start = arclen[i - 1];
                        let seg_len = arclen[i] - seg_start;
                        let a = poly[i - 1];
                        let b = poly[i];
                        out = if seg_len <= 0 {
                            (a.x_mm, a.y_mm)
                        } else {
                            let t = sc - seg_start;
                            (a.x_mm + (b.x_mm - a.x_mm) * t / seg_len, a.y_mm + (b.y_mm - a.y_mm) * t / seg_len)
                        };
                        break;
                    }
                }
                out
            };
            assert_eq!(p.point_at(s), want, "arclen={arclen:?} s={s}");
        }
    }
}
