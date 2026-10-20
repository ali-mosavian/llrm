//! Cases from gcc's own comments (`gimple-loop-jam.cc` 62-79, 436-466) and the
//! bench programs the analysis is for.

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
/// `px[j]` in the next outer iteration is a distance (1, 0).
#[test]
fn an_array_walked_again_by_each_outer_pass_repeats_at_one_outer_distance() {
    let nest = [access(&[sub(0, 0, 1)], false), access(&[sub(0, 0, 1)], true), access(&[sub(0, 0, 1)], false)];
    assert_eq!(distances(&nest[0], &nest[1], 4, Some(101)), Distances::Vectors(vec![(0, 0), (1, 0)]));
}

/// gcc's example: `x[j-1] = x[j]`: the write of one pass is read by the next
/// as (1, -1).
#[test]
fn a_shift_by_one_each_pass_reads_back_at_a_negative_inner_distance() {
    let nest = [access(&[sub(0, 0, 1)], false), access(&[sub(-1, 0, 1)], true)];
    assert_eq!(distances(&nest[0], &nest[1], 4, None), Distances::Vectors(vec![(0, 1), (1, -1), (1, 1)]));
}

/// gcc's `b[i][j] = b[i+1][j] + 2`: the access moves with the inner loop, so
/// the repeat is at (1, 0).
#[test]
fn an_access_that_moves_with_the_inner_loop_repeats_at_the_outer_distance() {
    let b = [access(&[sub(1, 1, 0), sub(0, 0, 1)], false), access(&[sub(0, 1, 0), sub(0, 0, 1)], true)];
    assert_eq!(distances(&b[0], &b[1], 4, Some(8)), Distances::Vectors(vec![(1, 0)]));
}

#[test]
fn a_pair_of_unlike_strides_has_unknown_distances() {
    let (one, other) = (access(&[sub(0, 0, 1)], true), access(&[sub(0, 0, 2)], false));
    assert_eq!(distances(&one, &other, 4, None), Distances::Unknown);
}

/// Two arrays the outer loop walks a row at a time: no access meets another
/// in a later pass.
#[test]
fn rows_walked_a_row_at_a_time_are_independent() {
    let nest = [access(&[sub(0, 1, 0), sub(0, 0, 1)], true), access(&[sub(100, 1, 0), sub(0, 0, 1)], false)];
    assert_eq!(distances(&nest[0], &nest[1], 4, Some(8)), Distances::Independent);
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

/// `a[j][i] = ..` over a 64 by 64 table of dwords, `i` the outer loop: the
/// swap makes the inner walk four bytes, not 256 (a 16 byte line), and no cell
/// is written twice, so it is legal.
#[test]
fn a_column_walk_of_a_table_is_swapped_when_each_cell_is_written_once() {
    let write = [access(&[sub(0, 4, 256)], true)];
    assert!(interchangeable(&write, 64, None));
    assert_eq!(walk_cost(&write, 16, Innermost::Inner), 16);
    assert_eq!(walk_cost(&write, 16, Innermost::Outer), 4);
}

/// The same table walked 128 columns of 64: column 64 of row 0 is column 0 of
/// row 1, a cell written at `(64, -1)` and `(0, 0)`, which the swap reverses.
#[test]
fn rows_that_run_into_the_next_are_not_swapped() {
    let write = [access(&[sub(0, 4, 256)], true)];
    assert!(!interchangeable(&write, 128, Some(64)));
}

/// gcc's `x[j-1] = x[j]` has the vector `(1, -1)`: the swap reads a cell before
/// its write.
#[test]
fn a_shift_that_a_later_pass_reads_back_is_not_swapped() {
    let nest = [access(&[sub(0, 0, 1)], false), access(&[sub(-1, 0, 1)], true)];
    assert!(!interchangeable(&nest, 8, None));
}

/// Two reads never order anything, and an address no loop moves costs none.
#[test]
fn reads_may_swap_and_an_unmoved_address_is_free() {
    let reads = [access(&[sub(0, 4, 256)], false), access(&[sub(8, 0, 0)], false)];
    assert!(interchangeable(&reads, 64, None));
    assert_eq!(walk_cost(&reads, 16, Innermost::Inner), 16);
    assert_eq!(walk_cost(&reads, 16, Innermost::Outer), 4);
}
