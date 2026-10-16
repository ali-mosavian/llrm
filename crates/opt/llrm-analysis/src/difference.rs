//! What a set of compares proves of the unknowns they test, by difference
//! bounds: LLVM's ConstraintElimination.
//!
//! # How it works
//! Each side of a compare is a [`Scev`] that is one unknown plus a constant
//! offset, or a constant. A compare of two sides is a constraint
//! `x - y <= c`. `proves` builds a graph with one node per unknown plus node 0
//! for the constant zero (`Z`), puts the constraints in as edges, and closes it
//! with Floyd-Warshall so `dist[a][b]` is the most `a - b` can be. The asked
//! compare holds when the closure bounds its `left - right` tightly enough.
//!
//! ```text
//!   facts:  x < y        y - x <= -1      node y --(-1)--> node x
//!           y <= 10      y - Z <= 10      node y --(10)--> Z
//!   asked:  x < 11       x - 11 <= -1     closure: x - Z <= (x - y) + (y - Z)
//!                                                    <= -1 + 10 = 9  -> holds
//! ```
//!
//! Other facts the system takes in:
//! - A stated range `[low, high]` of an unknown bounds it against `Z`.
//! - Unsigned compares. An unsigned compare is a signed one where both sides
//!   are known non-negative. `small[n]` marks unknowns that are: `Z` itself, an
//!   unknown whose stated low is at least 0, a `x <u y` with `y` non-negative,
//!   and any unknown the closure bounds below by zero (the closure is then run
//!   again, at most four rounds). Unsigned facts that are not so are ignored,
//!   as are `ne` facts.
//! - A negative cycle means the facts contradict each other: everything is
//!   proved.
//!
//! An offset side is trusted only if the closure keeps `unknown + offset`
//! inside the signed range, so the sum cannot wrap; an unsigned goal also needs
//! both sides' least value to be at least zero. Facts with an offset on an
//! unknown are dropped (only offset-free sides state a fact), and a system
//! with more than 24 nodes (`MOST_NODES`; the closure is cubic) proves
//! nothing. The answers: `slt`/`ult` is `left - right <= -1`, `sle`/`ule` is
//! `<= 0`, `sgt` and `sge` swap the sides, `eq` is both ways, `ne` is either
//! way strictly.
//!
//! # Example
//! The compares of quicksort's slice loop, over 16-bit unknowns `lo`, `hi` and
//! `len`:
//!
//! | Facts | Range | Asked | Result |
//! | --- | --- | --- | --- |
//! | `hi <u len`, `lo <s hi`, `lo <u len` | `len` in `[0, 32767]` | `hi - 1 <u len` | proved |
//! | same | same | `lo <=u hi - 1` | proved |
//! | same | none | `hi - 1 <u len` | not proved |
//! | same | none | `lo <=u hi - 1` | not proved |
//!
//! Without a range on `len`, `lo <u len` leaves `lo` possibly negative (`len`
//! 0xFFF0, `lo` 0x8008 passes), so the last index `hi - 1` is not below `len`.
//!
//! From unit tests `test_a_slice_loops_last_index_is_below_the_length` and
//! `test_an_unbounded_length_proves_nothing_of_the_last_index` in
//! `difference_tests.rs`.
//!
//! # Provides
//! `proves(width, facts, ranges, predicate, left, right)`. Its one consumer is
//! [`crate::guards`] (`Given::holds`), which supplies the guards and the
//! declared ranges.
//!
//! # Needs
//! [`crate::guards::Guard`]s and [`crate::induction`]'s `Scev` and `Monomial`.
//!
//! # See also
//! LLVM `ConstraintElimination`.

use std::collections::BTreeMap;

use llrm_mir::opcode::IntPredicate;

use crate::guards::Guard;
use crate::induction::{Monomial, Scev};
use crate::num::Num as BigInt;

/// The most unknowns a system keeps: the closure is cubic in them.
const MOST_NODES: usize = 24;

/// A side: an unknown, or a constant, with its offset.
struct Side {
    node: usize,
    offset: BigInt,
}

/// The value of `scev` as `unknown + offset`, or a constant, as a number
/// read signed at its width.
fn side(
    scev: &Scev,
    nodes: &mut Vec<Monomial>,
    create: bool,
) -> Option<Side> {
    if let Some(known) = scev.known() {
        return Some(Side { node: 0, offset: known });
    }
    let [(monomial, factor)] = scev.terms.iter().collect::<Vec<_>>()[..] else { return None };
    if *factor != BigInt::from(1) {
        return None;
    }
    let node = match nodes.iter().position(|one| one == monomial) {
        Some(at) => at + 1,
        None if create => {
            nodes.push(monomial.clone());
            nodes.len()
        }
        None => return None,
    };
    let constant = &scev.constant;
    let half = BigInt::from(1) << (scev.width - 1);
    let offset = if *constant >= half { constant - (BigInt::from(1) << scev.width) } else { constant.clone() };
    Some(Side { node, offset })
}

/// Whether `left predicate right` follows from `facts` and `ranges`, each
/// unknown's signed lowest and highest value. Every one is of `width` bits.
pub fn proves(
    width: u32,
    facts: &[Guard],
    ranges: &BTreeMap<Monomial, (BigInt, BigInt)>,
    predicate: IntPredicate,
    left: &Scev,
    right: &Scev,
) -> bool {
    if left.width != width || right.width != width {
        return false;
    }
    let mut nodes: Vec<Monomial> = Vec::new();
    // Only offset-free sides state a fact.
    let usable = facts
        .iter()
        .filter(|one| one.left.width == width && one.right.width == width)
        .filter_map(|one| {
            let (l, r) = (side(&one.left, &mut nodes, true)?, side(&one.right, &mut nodes, true)?);
            (l.offset == BigInt::from(0) || l.node == 0).then_some(())?;
            (r.offset == BigInt::from(0) || r.node == 0).then_some(())?;
            Some((one.predicate, l, r))
        })
        .collect::<Vec<_>>();
    // The goal's unknowns join the system.
    let (goal_left, goal_right) = match (side(left, &mut nodes, true), side(right, &mut nodes, true)) {
        (Some(l), Some(r)) => (l, r),
        _ => return false,
    };
    if nodes.len() + 1 > MOST_NODES {
        return false;
    }
    let size = nodes.len() + 1;
    let half = BigInt::from(1) << (width - 1);

    // Non-negative: Z itself, and the unknowns found so, to a fixpoint.
    let mut small = vec![false; size];
    small[0] = true;
    for (at, monomial) in nodes.iter().enumerate() {
        if ranges.get(monomial).is_some_and(|(low, _)| *low >= BigInt::from(0)) {
            small[at + 1] = true;
        }
    }
    let nonnegative = |small: &[bool], one: &Side| {
        if one.node == 0 { one.offset >= BigInt::from(0) } else { small[one.node] && one.offset == BigInt::from(0) }
    };
    loop {
        let mut grew = false;
        for (predicate, l, r) in &usable {
            // `x <=u y` with `y` below the sign bit: so is `x`.
            let (below, above) = match predicate {
                IntPredicate::Ult | IntPredicate::Ule => (l, r),
                IntPredicate::Ugt | IntPredicate::Uge => (r, l),
                _ => continue,
            };
            if below.node != 0 && !small[below.node] && nonnegative(&small, above) {
                small[below.node] = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    // `dist[a][b]`: the most `a - b` can be. What the closure bounds below by
    // zero is not negative either, which can turn an unsigned compare into
    // a signed one: round again.
    let mut dist;
    let mut rounds = 0;
    loop {
        dist = closed(size, &nodes, &small, ranges, &usable, &nonnegative);
        // A negative cycle: the facts contradict each other, and prove
        // anything.
        if (0..size).any(|at| dist[at][at].as_ref().is_some_and(|one| *one < BigInt::from(0))) {
            return true;
        }
        let found = (1..size)
            .filter(|&node| !small[node] && dist[0][node].as_ref().is_some_and(|most| *most <= BigInt::from(0)))
            .collect::<Vec<_>>();
        rounds += 1;
        if found.is_empty() || rounds > 4 {
            break;
        }
        for node in found {
            small[node] = true;
        }
    }

    // The side's least and greatest values, as numbers: None where unbounded.
    let span = |one: &Side| -> (Option<BigInt>, Option<BigInt>) {
        if one.node == 0 {
            return (Some(one.offset.clone()), Some(one.offset.clone()));
        }
        let high = dist[one.node][0].as_ref().map(|most| most + &one.offset);
        let low = dist[0][one.node].as_ref().map(|most| &one.offset - most);
        (low, high)
    };
    // An offset side is trusted only where its sum stays in the signed range.
    let sound = |one: &Side| {
        one.offset == BigInt::from(0)
            || one.node == 0
            || matches!(
                span(one),
                (Some(low), Some(high)) if low >= -half.clone() && high < half
            )
    };
    if !sound(&goal_left) || !sound(&goal_right) {
        return false;
    }
    let unsigned = matches!(
        predicate,
        IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
    );
    if unsigned {
        // Both sides are to be non-negative sums, so the order is the signed
        // one.
        let non_negative = |one: &Side| matches!(span(one).0, Some(low) if low >= BigInt::from(0));
        if !non_negative(&goal_left) || !non_negative(&goal_right) {
            return false;
        }
    }
    // `left - right <= most`, from the closure.
    let at_most = |l: &Side, r: &Side, most: BigInt| {
        dist[l.node][r.node].as_ref().is_some_and(|one| one + &l.offset - &r.offset <= most)
    };
    match predicate {
        IntPredicate::Slt | IntPredicate::Ult => at_most(&goal_left, &goal_right, BigInt::from(-1)),
        IntPredicate::Sle | IntPredicate::Ule => at_most(&goal_left, &goal_right, BigInt::from(0)),
        IntPredicate::Sgt | IntPredicate::Ugt => at_most(&goal_right, &goal_left, BigInt::from(-1)),
        IntPredicate::Sge | IntPredicate::Uge => at_most(&goal_right, &goal_left, BigInt::from(0)),
        IntPredicate::Eq => {
            at_most(&goal_left, &goal_right, BigInt::from(0)) && at_most(&goal_right, &goal_left, BigInt::from(0))
        }
        IntPredicate::Ne => {
            at_most(&goal_left, &goal_right, BigInt::from(-1)) || at_most(&goal_right, &goal_left, BigInt::from(-1))
        }
    }
}

/// The closure of the compares `usable` over `size` nodes, `dist[a][b]` the
/// most `a - b` can be.
fn closed(
    size: usize,
    nodes: &[Monomial],
    small: &[bool],
    ranges: &BTreeMap<Monomial, (BigInt, BigInt)>,
    usable: &[(IntPredicate, Side, Side)],
    nonnegative: &dyn Fn(&[bool], &Side) -> bool,
) -> Vec<Vec<Option<BigInt>>> {
    let mut dist: Vec<Vec<Option<BigInt>>> = vec![vec![None; size]; size];
    for (at, row) in dist.iter_mut().enumerate() {
        row[at] = Some(BigInt::from(0));
    }
    let bound = |dist: &mut Vec<Vec<Option<BigInt>>>, a: usize, b: usize, most: BigInt| {
        if dist[a][b].as_ref().is_none_or(|one| most < *one) {
            dist[a][b] = Some(most);
        }
    };
    for (at, monomial) in nodes.iter().enumerate() {
        let node = at + 1;
        if small[node] {
            bound(&mut dist, 0, node, BigInt::from(0));
        }
        if let Some((low, high)) = ranges.get(monomial) {
            bound(&mut dist, node, 0, high.clone());
            bound(&mut dist, 0, node, -low.clone());
        }
    }
    for (predicate, l, r) in usable {
        let unsigned =
            matches!(
                predicate,
                IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
            );
        if unsigned && !(nonnegative(small, l) && nonnegative(small, r)) {
            continue;
        }
        // `l.node - r.node <= most`, where `l.offset` and `r.offset` are folded
        // in.
        let gap = &r.offset - &l.offset;
        match predicate {
            IntPredicate::Slt | IntPredicate::Ult => bound(&mut dist, l.node, r.node, gap - 1),
            IntPredicate::Sle | IntPredicate::Ule => bound(&mut dist, l.node, r.node, gap),
            IntPredicate::Sgt | IntPredicate::Ugt => bound(&mut dist, r.node, l.node, -gap - 1),
            IntPredicate::Sge | IntPredicate::Uge => bound(&mut dist, r.node, l.node, -gap),
            IntPredicate::Eq => {
                bound(&mut dist, l.node, r.node, gap.clone());
                bound(&mut dist, r.node, l.node, -gap);
            }
            IntPredicate::Ne => {}
        }
    }
    for k in 0..size {
        for a in 0..size {
            let Some(through) = dist[a][k].clone() else { continue };
            for b in 0..size {
                if let Some(onward) = &dist[k][b] {
                    let total = &through + onward;
                    if dist[a][b].as_ref().is_none_or(|one| total < *one) {
                        dist[a][b] = Some(total);
                    }
                }
            }
        }
    }
    dist
}

#[cfg(test)]
#[path = "difference_tests.rs"]
mod tests;
