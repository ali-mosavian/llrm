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
    Access { object: 0, subscripts: subscripts.to_vec(), symbols: Vec::new(), width: 2, write }
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

/// Two arrays are two objects: a write to one never orders a read of the
/// other, whatever their subscripts, and the same subscripts of one array and
/// of a second symbolic base are not compared.
#[test]
fn accesses_to_different_objects_do_not_depend() {
    let (mut one, mut other) = (access(&[sub(0, 0, 1)], true), access(&[sub(0, 0, 1)], false));
    other.object = 1;
    assert_eq!(distances(&one, &other, 4, None), Distances::Independent);
    other.object = 0;
    other.symbols = vec![(7, 1)];
    one.symbols = vec![(8, 1)];
    assert_eq!(distances(&one, &other, 4, None), Distances::Unknown);
}

fn walker(
    constant: i64,
    step: i64,
    width: u32,
    write: bool,
) -> Access {
    Access { object: 0, subscripts: vec![sub(constant, 0, step)], symbols: Vec::new(), width, write }
}

/// Loop 0 writes `a[i]`: loop 1 reading `a[i]` or an earlier element is run
/// after the write in the fused loop as it was before; `a[i + 1]` is written
/// only in the next iteration of loop 0, which fused runs after the read.
#[test]
fn a_later_loop_may_read_what_the_earlier_wrote_not_what_it_will() {
    let write = [walker(0, 4, 4, true)];
    assert!(fusable(&write, &[walker(0, 4, 4, false)], None));
    assert!(fusable(&write, &[walker(-4, 4, 4, false)], None));
    assert!(!fusable(&write, &[walker(4, 4, 4, false)], None));
}

/// The same for what the later loop writes over what the earlier read.
#[test]
fn a_later_loop_may_not_write_what_the_earlier_will_read() {
    let read = [walker(0, 4, 4, false)];
    assert!(fusable(&read, &[walker(-4, 4, 4, true)], None));
    assert!(!fusable(&read, &[walker(4, 4, 4, true)], None));
}

/// `a[i]` written four bytes wide, read at two bytes in: the bytes overlap
/// the next element's write, which fused runs after the read.
#[test]
fn a_partial_overlap_is_a_dependence() {
    assert!(!fusable(&[walker(0, 4, 4, true)], &[walker(2, 4, 4, false)], None));
    assert!(fusable(&[walker(0, 8, 4, true)], &[walker(4, 8, 4, false)], None), "bytes 4..8 of each 8");
}

/// A distance of eight is out of reach of eight trips.
#[test]
fn a_distance_past_the_trips_is_not_a_dependence() {
    let (write, read) = ([walker(0, 4, 4, true)], [walker(32, 4, 4, false)]);
    assert!(!fusable(&write, &read, None));
    assert!(!fusable(&write, &read, Some(9)));
    assert!(fusable(&write, &read, Some(8)));
}

/// A cell no iteration moves is written by every iteration of loop 0: the
/// read of loop 1 would see only some of them, unless there is one trip.
#[test]
fn an_invariant_cell_written_then_read_is_fused_only_for_one_trip() {
    let (write, read) = ([walker(0, 0, 4, true)], [walker(0, 0, 4, false)]);
    assert!(!fusable(&write, &read, None));
    assert!(!fusable(&write, &read, Some(2)));
    assert!(fusable(&write, &read, Some(1)));
}

/// A descending walk is the same question with the distance reversed.
#[test]
fn a_descending_walk_is_judged_by_its_own_direction() {
    let write = [walker(400, -4, 4, true)];
    assert!(fusable(&write, &[walker(400, -4, 4, false)], None));
    assert!(fusable(&write, &[walker(404, -4, 4, false)], None));
    assert!(!fusable(&write, &[walker(396, -4, 4, false)], None));
}

/// Unlike steps, symbols or objects: two reads never conflict, a write does
/// where the bytes are not known apart, and distinct objects never do.
#[test]
fn unlike_accesses_conflict_only_where_unknown() {
    let (write, slow) = (walker(0, 4, 4, true), walker(0, 8, 4, false));
    assert!(!fusable(&[write.clone()], &[slow.clone()], None));
    assert!(fusable(&[walker(0, 4, 4, false)], &[slow], None));
    let other = Access { object: 1, ..walker(0, 4, 4, false) };
    assert!(fusable(&[write.clone()], &[other], None));
    let symbolic = Access { symbols: vec![(7, 1)], ..walker(0, 4, 4, false) };
    assert!(!fusable(&[write], &[symbolic], None));
}

/// `a[i] = ..` then `.. = a[i - 1]`: the read meets the write of the trip
/// before, so the write comes first: forward only, and a loop of the write and
/// a loop of the read after it is right. `a[i + 1]` is read before it is
/// written: backward only.
#[test]
fn a_body_s_accesses_meet_forward_or_backward_by_their_offset() {
    let write = walker(0, 4, 4, true);
    assert_eq!(meetings(&write, &walker(0, 4, 4, false), None), Meetings { forward: true, backward: false });
    assert_eq!(meetings(&write, &walker(-4, 4, 4, false), None), Meetings { forward: true, backward: false });
    assert_eq!(meetings(&write, &walker(4, 4, 4, false), None), Meetings { forward: false, backward: true });
    assert_eq!(meetings(&write, &walker(400, 4, 4, false), Some(10)), Meetings { forward: false, backward: false });
}

/// An invariant cell is met by every pair of trips, both ways, from two trips
/// on.
#[test]
fn an_invariant_cell_is_met_both_ways_by_two_trips_and_one_way_by_one() {
    let (write, read) = (walker(0, 0, 4, true), walker(0, 0, 4, false));
    assert_eq!(meetings(&write, &read, None), Meetings { forward: true, backward: true });
    assert_eq!(meetings(&write, &read, Some(1)), Meetings { forward: true, backward: false });
}

/// What is not known apart on one object meets both ways; two objects never.
#[test]
fn unlike_accesses_of_one_body_meet_both_ways_and_two_objects_none() {
    let both = Meetings { forward: true, backward: true };
    assert_eq!(meetings(&walker(0, 4, 4, true), &walker(0, 8, 4, false), None), both);
    let other = Access { object: 1, ..walker(0, 4, 4, false) };
    assert_eq!(meetings(&walker(0, 4, 4, true), &other, None), Meetings { forward: false, backward: false });
}
