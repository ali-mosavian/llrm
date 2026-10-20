//! Data dependence distances over a two-deep loop nest, the outer unroll
//! factor that unroll-and-jam may use for them, and whether two loops may be
//! fused.
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
//! `fusable(first, second, trips)` asks it of two loops of the same trip
//! count, `first` run before `second`: fused, iteration `i` of `second` runs
//! before iteration `i + 1` of `first`, so a pair that touches the same bytes
//! with the later loop's access at an earlier iteration than the earlier
//! loop's is reordered. Both loops' accesses are `inner` subscripts. For a
//! pair with one write, on one object, of one coefficient `c` and the same
//! symbols, the accesses at `a + c * i` (width `w`) and `b + c * j` (width `v`)
//! overlap where `b - a - w < c * (i - j) < b - a + v`; the pair prevents
//! fusion when some `i - j` in `1..trips` does (`trips` unknown: any). A pair
//! with other coefficients or symbols is `Unknown` and prevents it.
//!
//! `meetings(first, second, trips)` is the same question of two accesses of one
//! body, `first` earlier in it: whether they meet with `first` at or before
//! `second` (`forward`, `i - j` in `1 - trips..=0`) or after it (`backward`,
//! `1..trips`). Loop distribution puts the two in one loop where both hold and
//! in two, in body order, where only `forward` does.
//!
//! ```text
//!   loop 0: a[i] = ..        loop 1: .. = a[i + 1]     i - j = 1: prevents
//!   loop 0: a[i] = ..        loop 1: .. = a[i - 1]     i - j = -1: allows
//! ```
//!
//! # Example
//! The 100 passes of `px[j] = px[j] + vx[j]` (bench/particle). The nest has no
//! MIR form here: the caller hands `Access`es, each a `Subscript { constant,
//! outer, inner }` per dimension, width 2:
//!
//! ```text
//! nest[0]  read   px[j]   subscripts [ (0, 0, 1) ]
//! nest[1]  write  px[j]   subscripts [ (0, 0, 1) ]
//! nest[2]  read   vx[j]   subscripts [ (0, 0, 1) ]    (same object as the others)
//! ```
//!
//! | Query | Result |
//! | --- | --- |
//! | `distances(nest[0], nest[1], 4, Some(101))` | `Vectors([(0, 0), (1, 0)])` |
//! | `factor(nest, 4, 1, Some(101))` | `unroll = 2`, `profit = 2` |
//!
//! The write of `px[j]` is read again by the next outer pass at `(1, 0)`, so
//! two copies make a reference go away.
//!
//! From unit test
//! `an_array_walked_again_by_each_outer_pass_allows_a_jam_by_two` in
//! `dependence_tests.rs`.
//!
//! `interchangeable(accesses, outer_trips, inner_trips)` asks whether the loops
//! may swap: a vector `(x, y)` with `x > 0` and `y < 0` becomes `(y, x)`, the
//! later access first, so none may exist for a pair with a write. `walk_cost`
//! prices a walk by the bytes an access steps in the loop that runs innermost,
//! at most a cache line, none where it does not move.
//!
//! # Provides
//! `distances`, `factor`, `fusable`, `meetings`, `interchangeable`,
//! `walk_cost`. Consumers: `loopdistribute`
//! (`crates/opt/llrm-transforms/src/loopdistribute. rs`) and `loopinterchange`.
//!
//! # See also
//! gcc `tree-data-ref.cc` distance vectors read by `adjust_unroll_factor`
//! ([`gimple-loop-jam.cc:403`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L403)),
//! `tree_loop_unroll_and_jam`
//! ([`gimple-loop-jam.cc:504`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc#L504));
//! LLVM `DependenceAnalysis` and `LoopUnrollAndJam`; `fusable` is LLVM
//! `LoopFuse`'s `dependencesAllowFusion`
//! ([`LoopFuse.cpp:1178`](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopFuse.cpp#L1178)).

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

/// Whether the loops of a nest may swap places: no pair of accesses with a
/// write (a write with itself too) has a dependence vector `(x, y)` with `x >
/// 0` and `y < 0`, which the swap turns into `(y, x)`, the later access first.
/// A pair of unknown distance with a write does not. `outer_trips` bounds the
/// outer distances; `inner_trips` the inner ones where known (gcc's
/// `valid_data_dependences`, LLVM's `LoopInterchangeLegality`'s direction
/// vectors).
pub fn interchangeable(
    accesses: &[Access],
    outer_trips: i64,
    inner_trips: Option<i64>,
) -> bool {
    accesses
        .iter()
        .enumerate()
        .all(
            |(at, one)| accesses[at..]
                .iter()
                .filter(|other| one.write || other.write)
                .all(
                    |other| match distances(one, other, outer_trips - 1, inner_trips) {
                        Distances::Independent => true,
                        Distances::Unknown => false,
                        Distances::Vectors(vectors) => vectors.iter().all(|&(x, y)| !(x > 0 && y < 0)),
                    },
                ),
        )
}

/// Which loop of a nest runs innermost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Innermost {
    Outer,
    Inner,
}

/// What one trip of the loop that runs `innermost` costs the data cache, over
/// all `accesses` (each a flattened byte address, `subscripts[0]`): the bytes
/// it steps, at most a cache `line`; an address the loop does not move is
/// kept in a register and costs none (gcc's `compute_access_stride`, LLVM's
/// `CacheCost`).
pub fn walk_cost(
    accesses: &[Access],
    line: i64,
    innermost: Innermost,
) -> i64 {
    accesses
        .iter()
        .map(|one| {
            let step = one
                .subscripts
                .first()
                .map_or(
                    0,
                    |sub| match innermost {
                        Innermost::Outer => sub.outer,
                        Innermost::Inner => sub.inner,
                    },
                );
            step.abs().min(line)
        })
        .sum()
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

/// Whether the loop of `second`, run after the loop of `first` and of the
/// same `trips`, may run in the same iterations as it: no pair with a write
/// touches bytes at an earlier iteration of `second` than of `first`.
pub fn fusable(
    first: &[Access],
    second: &[Access],
    trips: Option<i64>,
) -> bool {
    first
        .iter()
        .flat_map(|one| second.iter().map(move |other| (one, other)))
        .filter(|(one, other)| one.write || other.write)
        .all(|(one, other)| !meetings(one, other, trips).backward)
}

/// The ways two accesses of one loop's body meet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Meetings {
    /// Some trip of `first` is at or before a trip of `second` that touches
    /// the same bytes: `first` must come before `second` where they are told
    /// apart.
    pub forward: bool,
    /// Some trip of `first` is after a trip of `second` that touches the same
    /// bytes: `second` must come before `first`.
    pub backward: bool,
}

/// How `first`, earlier in the body, and `second`, later in it (or the same
/// access of a loop run before the other's, as `fusable` asks), meet over the
/// loop's `trips`. Both ways where they are on one object and not known
/// apart, neither where they are on two. Told apart, a loop that runs them as
/// two loops in this order is right only if no pair meets backward; one that
/// runs `second` first, only if none meets forward.
pub fn meetings(
    first: &Access,
    second: &Access,
    trips: Option<i64>,
) -> Meetings {
    let (some, none) = (Meetings { forward: true, backward: true }, Meetings { forward: false, backward: false });
    if first.object != second.object {
        return none;
    }
    let ([one], [other]) = (&first.subscripts[..], &second.subscripts[..]) else { return some };
    if first.symbols != second.symbols || one.inner != other.inner || one.outer != 0 || other.outer != 0 {
        return some;
    }
    // `first` at trip `i` and `second` at `j` are `step * (i - j)` apart plus
    // the constants' difference; the trips are `0..trips`.
    let step = i128::from(one.inner);
    let latest = trips.map_or(1 << 40, |trips| i128::from(trips) - 1);
    let below = i128::from(other.constant) - i128::from(one.constant) - i128::from(first.width);
    let above = i128::from(other.constant) - i128::from(one.constant) + i128::from(second.width);
    let meet = |from: i128, to: i128| {
        if from > to {
            return false;
        }
        if step == 0 {
            return below < 0 && 0 < above;
        }
        // `|step| * e` in `(below, above)` for the least `e` in range, where
        // `e` is `i - j` (rising step) or `j - i` (falling).
        let (size, low, high) = if step > 0 { (step, from, to) } else { (-step, -to, -from) };
        let least = low.max(below.div_euclid(size) + 1);
        least <= high && size * least < above
    };
    Meetings { forward: meet(-latest, 0), backward: meet(1, latest) }
}

#[cfg(test)]
#[path = "dependence_tests.rs"]
mod tests;
