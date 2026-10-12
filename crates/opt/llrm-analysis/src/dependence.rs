//! Data dependence distances over a two-deep loop nest, and the outer unroll
//! factor that unroll-and-jam may use for them.
//!
//! # How it works
//! An `Access` is one memory reference of the nest body: an object, a list of
//! `Subscript`s (`constant + outer * i + inner * j`, `i` and `j` the iteration
//! numbers; `a[i][j]` has two, a flattened address one), loop-invariant
//! `symbols`, a width and a write flag. The caller has found the objects.
//!
//! `distances(first, second, limit, inner_trips)` solves
//! `outer * x + inner * y = constant difference` in each subscript, in both
//! directions, for outer distances `x` in `0..=limit` and one inner distance
//! `y`. Pairs with different objects are `Independent`. Pairs whose symbols,
//! width or coefficients differ are `Unknown`. Otherwise the result is the
//! lexicographically non-negative `(x, y)` vectors, with `y` bounded by
//! `inner_trips` where known. A reference no subscript moves with in the inner
//! loop repeats at `(x, 0)` and, for `x = 0`, `(0, 1)`.
//!
//! `factor(accesses, max_unroll, min_percent, inner_trips)` looks at every pair
//! of accesses. Unrolling the outer loop by N turns `(x, y)` into `(x / N, y)`,
//! so a vector bounds N when it would reorder the pair in the fused loop:
//!
//! ```text
//!  for i                     for i step N
//!    for j        ==>          for j
//!      S(i, j)                   S(i,j) S(i+1,j) ... S(i+N-1,j)
//!
//!  vector (x, y) ok if x >= N, or y > 0; y = 0 with x < N needs two reads or
//!  two accesses that both vary with j; else N is capped at x (y < 0).
//! ```
//!
//! Two reads never order anything; an `Unknown` pair with a write gives
//! `unroll = 0`. `profit` is one more than the largest outer distance at which
//! a reference repeats at `y = 0` (copies that make it go away), `removed`
//! counts such vectors, and the factor is refused (`profit = 1`) if
//! `removed * 100 / accesses` is below `min_percent`. `min_percent = 0` applies
//! always, with at least 2. The result is `min(unroll, profit, max_unroll)`.
//!
//! # Provides
//! `distances`, `factor`. Consumer: `unroll_jam`
//! (`crates/opt/llrm-transforms/src/unroll_jam.rs`).
//!
//! # See also
//! gcc `tree-data-ref.cc` distance vectors read by `adjust_unroll_factor`
//! ([`gimple-loop-jam.cc:403`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L403)),
//! `tree_loop_unroll_and_jam`
//! ([`gimple-loop-jam.cc:504`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L504));
//! LLVM `DependenceAnalysis` and `LoopUnrollAndJam`.

/// One subscript of an access, in elements or bytes: `constant + outer * i +
/// inner * j`, `i` and `j` the iteration numbers of the outer and inner loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Subscript {
    pub constant: i64,
    pub outer: i64,
    pub inner: i64,
}

/// One memory access of the nest's body: its subscripts (gcc's access
/// functions; `a[i][j]` has two, a flattened address one), and its width.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Access {
    /// The object accessed: accesses to different objects never depend.
    pub object: u64,
    pub subscripts: Vec<Subscript>,
    /// Terms that do not change in the nest (a value, its coefficient): two
    /// accesses whose terms differ are a distance the nest does not know.
    pub symbols: Vec<(u32, i64)>,
    pub width: u32,
    pub write: bool,
}

impl Access {
    /// Whether any subscript moves with the inner loop
    /// (`any_access_function_variant_p`).
    pub fn varies_inner(&self) -> bool {
        self.subscripts.iter().any(|one| one.inner != 0)
    }
}

/// How two accesses depend over the nest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Distances {
    Independent,
    /// `(outer, inner)` iterations from the access that comes first to the one
    /// that comes after, each lexicographically non-negative. A loop no
    /// subscript moves with has the distances 0 and 1 only: the nearest repeat
    /// is what bounds an unroll.
    Vectors(Vec<(i64, i64)>),
    Unknown,
}

/// The distance vectors of `first` and `second`, outer distances up to
/// `limit` (a larger one never bounds an unroll by at most `limit`), inner
/// ones within `inner_trips` where it is known.
pub fn distances(
    first: &Access,
    second: &Access,
    limit: i64,
    inner_trips: Option<i64>,
) -> Distances {
    if first.object != second.object {
        return Distances::Independent;
    }
    let coefficients = |one: &Access| one.subscripts.iter().map(|sub| (sub.outer, sub.inner)).collect::<Vec<_>>();
    if first.symbols != second.symbols || first.width != second.width || coefficients(first) != coefficients(second) {
        return Distances::Unknown;
    }
    let moves_outer = first.subscripts.iter().any(|sub| sub.outer != 0);
    let reach = inner_trips.map(|trips| trips - 1);
    let mut found: Vec<(i64, i64)> = Vec::new();
    // `outer * x + inner * y = target` in each subscript, from the first access
    // to the second or, negated, the other way round.
    for sign in [1, -1] {
        for x in if moves_outer { 0..=limit } else { 0..=1 } {
            let mut inner: Option<i64> = None;
            let mut consistent = true;
            for (one, other) in first.subscripts.iter().zip(&second.subscripts) {
                let rest = sign * (one.constant - other.constant) - one.outer * x;
                if one.inner == 0 {
                    consistent &= rest == 0;
                } else if rest % one.inner != 0 || inner.is_some_and(|seen| seen != rest / one.inner) {
                    consistent = false;
                } else {
                    inner = Some(rest / one.inner);
                }
            }
            if !consistent {
                continue;
            }
            match inner {
                // No subscript moves with the inner loop: the same element at
                // every inner iteration; the nearest repeat is one away.
                None => {
                    found.push((x, 0));
                    if x == 0 {
                        found.push((0, 1));
                    }
                }
                Some(y) if (x > 0 || y >= 0) && reach.is_none_or(|reach| y.abs() <= reach) => found.push((x, y)),
                Some(_) => {}
            }
        }
    }
    found.sort();
    found.dedup();
    if found.is_empty() { Distances::Independent } else { Distances::Vectors(found) }
}

/// What a nest allows of an outer unroll that fuses the inner copies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Factor {
    /// The most copies the dependences allow; below 2, none.
    pub unroll: u32,
    /// The copies that make a reference go away, 1 if none does.
    pub profit: u32,
    /// Pairs whose later access is the earlier one's repeat in the next outer
    /// iterations.
    pub removed: u32,
}

/// The outer unroll factor for `accesses`, all those of the nest: gcc's
/// `tree_loop_unroll_and_jam` over `adjust_unroll_factor`
/// ([`gimple-loop-jam.cc:504`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L504)),
/// capped by `max_unroll` and refused where fewer than `min_percent` percent
/// of the accesses go away (`min_percent` 0 applies it always, with 2).
/// `inner_trips` bounds the inner distances where the inner loop's count is
/// known.
pub fn factor(
    accesses: &[Access],
    max_unroll: u32,
    min_percent: u32,
    inner_trips: Option<i64>,
) -> Factor {
    let mut unroll = u32::MAX;
    let (mut profit, mut removed) = (1u32, 0u32);
    for (at, one) in accesses.iter().enumerate() {
        for other in &accesses[at + 1..] {
            // Two reads never order anything.
            let vectors = match distances(one, other, i64::from(max_unroll), inner_trips) {
                Distances::Independent => continue,
                Distances::Unknown if !one.write && !other.write => continue,
                Distances::Unknown => return Factor { unroll: 0, profit, removed },
                Distances::Vectors(vectors) => vectors,
            };
            for (x, y) in vectors {
                let x = x as u32;
                // `(x, y)` becomes `(x / N, y)` under an unroll by N: valid if
                // x >= N, or y is positive, or y is zero and x is positive
                // and nothing is invariant in the inner loop.
                if x >= unroll {
                } else if y == 0 {
                    // (x, 0), x < N: the copies meet in one iteration of the
                    // fused loop. Fine for two reads, and for two accesses
                    // that both move with the inner loop; any other order of
                    // them is not kept by the fusion.
                    if !(!one.write && !other.write) && !(one.varies_inner() && other.varies_inner()) {
                        unroll = x;
                    }
                } else if y > 0 {
                } else {
                    unroll = x;
                }
                if y == 0 {
                    profit = profit.max(x + 1);
                    removed += 1;
                }
            }
        }
    }
    let profitable = if min_percent == 0 {
        profit = profit.max(2);
        true
    } else {
        removed * 100 / accesses.len().max(1) as u32 >= min_percent
    };
    if !profitable {
        profit = 1;
    }
    Factor { unroll: unroll.min(profit).min(max_unroll), profit, removed }
}

#[cfg(test)]
#[path = "dependence_tests.rs"]
mod tests;
