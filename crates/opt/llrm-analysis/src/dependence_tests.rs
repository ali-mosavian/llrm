//! Cases from gcc's own comments (`gimple-loop-jam.cc` 62-79, 436-466) and the
//! bench programs the pass is for.

use super::*;

fn sub(
    constant: i64,
    outer: i64,
    inner: i64,
) -> Subscript {
    Subscript { constant, outer, inner }
}

fn access(
    subscripts: &[Subscript],
    write: bool,
) -> Access {
    Access { subscripts: subscripts.to_vec(), width: 2, write }
}

/// `px[j] = px[j] + vx[j]`, the 100 passes of bench/particle: the repeat of
/// `px[j]` in the next outer iteration is a distance (1, 0), so two copies
/// make one of the references go away.
#[test]
fn an_array_walked_again_by_each_outer_pass_allows_a_jam_by_two() {
    let nest = [access(&[sub(0, 0, 1)], false), access(&[sub(0, 0, 1)], true), access(&[sub(0, 0, 1)], false)];
    assert_eq!(distances(&nest[0], &nest[1], 4, Some(101)), Distances::Vectors(vec![(0, 0), (1, 0)]));
    let plan = factor(&nest, 4, 1, Some(101));
    assert_eq!((plan.unroll, plan.profit), (2, 2), "{plan:?}");
}

/// gcc's example: `x[j-1] = x[j]` jammed by two moves the content by one
/// element, not two: the write of one pass is read by the next as (1, -1),
/// which fusing reverses.
#[test]
fn a_shift_by_one_each_pass_is_not_jammed() {
    let nest = [access(&[sub(0, 0, 1)], false), access(&[sub(-1, 0, 1)], true)];
    assert_eq!(distances(&nest[0], &nest[1], 4, None), Distances::Vectors(vec![(0, 1), (1, -1), (1, 1)]));
    assert!(factor(&nest, 4, 1, None).unroll < 2, "{:?}", factor(&nest, 4, 1, None));
}

/// gcc's `a[i][0] = a[i+1][0] + 2` with `b[i][j] = b[i+1][j] + 2`: the first
/// is invariant in the inner loop, so fusing reorders its accesses and the
/// factor falls to the distance; the second moves with the inner loop and
/// keeps its.
#[test]
fn an_access_invariant_in_the_inner_loop_limits_the_factor_to_its_distance() {
    let a = [access(&[sub(1, 1, 0), sub(0, 0, 0)], false), access(&[sub(0, 1, 0), sub(0, 0, 0)], true)];
    assert_eq!(factor(&a, 4, 1, Some(8)).unroll, 1, "{:?}", factor(&a, 4, 1, Some(8)));
    let b = [access(&[sub(1, 1, 0), sub(0, 0, 1)], false), access(&[sub(0, 1, 0), sub(0, 0, 1)], true)];
    assert_eq!(distances(&b[0], &b[1], 4, Some(8)), Distances::Vectors(vec![(1, 0)]));
    assert_eq!(factor(&b, 4, 1, Some(8)).unroll, 2, "{:?}", factor(&b, 4, 1, Some(8)));
}

#[test]
fn a_pair_of_unlike_strides_that_writes_is_not_jammed_and_two_reads_are() {
    let (one, other) = (access(&[sub(0, 0, 1)], true), access(&[sub(0, 0, 2)], false));
    assert_eq!(distances(&one, &other, 4, None), Distances::Unknown);
    assert_eq!(factor(&[one, other], 4, 1, None).unroll, 0);
    let reads = [access(&[sub(0, 0, 1)], false), access(&[sub(0, 0, 2)], false), access(&[sub(0, 0, 1)], false)];
    assert!(factor(&reads, 4, 0, None).unroll >= 2, "{:?}", factor(&reads, 4, 0, None));
}

/// Two arrays the outer loop walks a row at a time: no access meets another
/// in a later pass, so no reference goes away and nothing is jammed.
#[test]
fn nothing_goes_away_so_nothing_is_jammed() {
    let nest = [access(&[sub(0, 1, 0), sub(0, 0, 1)], true), access(&[sub(100, 1, 0), sub(0, 0, 1)], false)];
    assert_eq!(distances(&nest[0], &nest[1], 4, Some(8)), Distances::Independent);
    let plan = factor(&nest, 4, 1, Some(8));
    assert_eq!((plan.removed, plan.profit), (0, 1), "{plan:?}");
    assert!(plan.unroll < 2, "{plan:?}");
}
