//! Data dependence distances over a two-deep loop nest, and whether its two
//! loops may swap.
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
//! `interchangeable(accesses, outer_trips, inner_trips)` asks whether the loops
//! may swap: a vector `(x, y)` with `x > 0` and `y < 0` becomes `(y, x)`, the
//! later access first, so none may exist for a pair with a write. `walk_cost`
//! prices a walk by the bytes an access steps in the loop that runs innermost,
//! at most a cache line, none where it does not move.
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
//!
//! The write of `px[j]` is read again by the next outer pass at `(1, 0)`.
//!
//! # Provides
//! `distances`, `interchangeable`, `walk_cost`. Consumer: `loopinterchange`.
//!
//! # See also
//! gcc `tree-data-ref.cc` distance vectors; LLVM `DependenceAnalysis`.

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

#[cfg(test)]
#[path = "dependence_tests.rs"]
mod tests;
