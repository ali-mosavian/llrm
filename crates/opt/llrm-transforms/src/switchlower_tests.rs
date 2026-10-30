//! Each body is MIR text run by llrm-mir's interpreter before and after over
//! every selector worth trying, so a wrong target shows as a wrong answer;
//! what is asserted of the text is the shape gcc 13.4 gives that switch.

use llrm_analysis::testing::DOS;

use super::SwitchLower;
use crate::testing::{Tuned, managed_on, parsed, results};

/// A 486's prices, in clocks.
fn i486() -> crate::profit::OperationCosts {
    crate::profit::OperationCosts { add: 1, branch: 3, load: 1, r#move: 1, extend: 3, ..Default::default() }
}

fn lowered(
    text: &str,
    inputs: impl IntoIterator<Item = i128>,
    tables: bool,
) -> String {
    lowered_by(text, inputs, tables, 0)
}

/// `lowered` for a target with a table jump from `jump_table` cases.
fn lowered_by(
    text: &str,
    inputs: impl IntoIterator<Item = i128>,
    tables: bool,
    jump_table: i64,
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let machine =
        Tuned { costs: crate::profit::OperationCosts { jump_table, ..Default::default() }, ..Default::default() };
    let machine = || Tuned { costs: machine.costs.clone(), ..Default::default() };
    if tables {
        managed_on(&mut module, SwitchLower { convert: true, jump_tables: true, size: false }, machine());
    }
    let after = managed_on(&mut module, SwitchLower { convert: false, jump_tables: true, size: false }, machine());
    let inputs: Vec<[i128; 1]> = inputs.into_iter().map(|one| [one]).collect();
    let inputs: Vec<&[i128]> = inputs.iter().map(|one| &one[..]).collect();
    assert_eq!(results(&module, &inputs), results(&before, &inputs), "{after}");
    after
}

/// `lowered` with a lookup table priced by `costs`, and in bytes by `sizes`
/// where `size`.
fn priced(
    text: &str,
    inputs: impl IntoIterator<Item = i128>,
    costs: crate::profit::OperationCosts,
    sizes: crate::profit::OperationCosts,
    size: bool,
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let machine = || Tuned { costs: costs.clone(), sizes: sizes.clone(), ..Default::default() };
    managed_on(&mut module, SwitchLower { convert: true, jump_tables: false, size }, machine());
    let after = managed_on(&mut module, SwitchLower { convert: false, jump_tables: false, size }, machine());
    let inputs: Vec<[i128; 1]> = inputs.into_iter().map(|one| [one]).collect();
    let inputs: Vec<&[i128]> = inputs.iter().map(|one| &one[..]).collect();
    assert_eq!(results(&module, &inputs), results(&before, &inputs), "{after}");
    after
}

/// A switch on `x` of `ty` whose targets each return their number; `holes` to
/// the default, 99.
fn returns(
    ty: &str,
    cases: &[(i128, u32)],
) -> String {
    let labels: String = cases.iter().map(|(value, target)| format!("    {ty} {value}, label %t{target}\n")).collect();
    let mut targets: Vec<u32> = cases.iter().map(|&(_, target)| target).collect();
    targets.sort();
    targets.dedup();
    let blocks: String = targets.iter().map(|target| format!("t{target}:\n  ret i32 {target}\n\n")).collect();
    format!(
        "define i32 @f({ty} %x) {{\nb0:\n  switch {ty} %x, label %d [\n{labels}  ]\n\n{blocks}d:\n  ret i32 99\n}}\n"
    )
}

/// The same, joined: each target jumps to one phi, as the cases of a C switch
/// that only pick a value do.
fn joined(
    ty: &str,
    cases: &[(i128, i128)],
    hole: i128,
) -> String {
    let labels: String =
        cases.iter().enumerate().map(|(at, (value, _))| format!("    {ty} {value}, label %t{at}\n")).collect();
    let blocks: String = (0..cases.len()).map(|at| format!("t{at}:\n  br label %j\n\n")).collect();
    let phi: String = cases.iter().enumerate().map(|(at, (_, got))| format!("[ {got}, %t{at} ], ")).collect();
    format!(
        "define i32 @f({ty} %x) {{\nb0:\n  switch {ty} %x, label %d [\n{labels}  ]\n\n{blocks}d:\n  br label %j\n\nj:\n  %r = phi i32 {phi}[ {hole}, %d ]\n  ret i32 %r\n}}\n"
    )
}

/// `x_switch_ranges`: 58 labels over 6 targets were 58 compares in a row, 3.2x
/// gcc's 486 clocks; neighbours to one target are one range.
#[test]
fn test_neighbours_with_one_target_are_one_range_test() {
    let after = lowered(&returns("i32", &[(0, 1), (1, 1), (2, 1), (40, 2)]), -3..45, false);
    assert!(!after.contains("switch"), "{after}");
    assert!(after.contains("icmp ule i32 %x, 2") && !after.contains("icmp eq i32 %x, 1"), "{after}");
}

/// Selectors compared signed: -1, 0 and 1 are three neighbours, not 0, 1 and
/// 4294967295 (a table of four billion).
#[test]
fn test_negative_neighbours_are_one_range_by_the_signed_order() {
    let after = lowered(&returns("i32", &[(-1, 1), (0, 1), (1, 1), (50, 2)]), -3..60, false);
    assert!(!after.contains("switch"), "{after}");
    assert!(after.contains("sub i32 %x, -1") && after.contains("icmp ule i32 %0, 2"), "{after}");
}

/// gcc's `bit_test_cluster`: 5 cases of 2 targets in a word are `1 << x`
/// against two masks.
#[test]
fn test_few_targets_in_a_word_are_bit_tests() {
    let after = lowered(&returns("i32", &[(1, 1), (3, 1), (5, 1), (2, 2), (8, 2)]), -2..40, false);
    assert!(!after.contains("switch"), "{after}");
    assert!(after.contains("shl i32 1,"), "{after}");
}

/// Four cases of two targets are too few for bit tests (gcc wants 5): a tree.
#[test]
fn test_too_few_cases_are_not_bit_tests() {
    let after = lowered(&returns("i32", &[(1, 1), (3, 1), (2, 2), (8, 2)]), -2..40, false);
    assert!(!after.contains("shl"), "{after}");
}

/// A selector wider than the word is cut to it for the shift, after the range
/// test.
#[test]
fn test_bit_tests_of_a_wide_selector() {
    let after = lowered(&returns("i64", &[(1, 1), (3, 1), (5, 1), (2, 2), (8, 2)]), -2..40, false);
    assert!(after.contains("trunc i64"), "{after}");
}

/// x_switch_dense: 12 cases that each pick a constant were 11 compares in a
/// row (445 B, 1.23x gcc's clocks); they are a load from an array.
#[test]
fn test_cases_that_pick_constants_are_a_table() {
    let cases: Vec<(i128, i128)> =
        (0..12).map(|at| (at, [3, 14, 15, 92, 65, 35, 89, 79, 32, 38, 46, 26][at as usize])).collect();
    let after = lowered(&joined("i32", &cases, 1), -3..20, true);
    assert!(!after.contains("switch i32") && after.contains("load i8"), "{after}");
    assert!(after.contains("@switch.table.0 = private constant [12 x i8]"), "{after}");
}

/// Without switch conversion (-O1) the same switch is a tree.
#[test]
fn test_no_table_without_switch_conversion() {
    let cases: Vec<(i128, i128)> = (0..12).map(|at| (at, at * 7 + 1)).collect();
    let after = lowered(&joined("i32", &cases, 1), -3..20, false);
    assert!(!after.contains("load") && !after.contains("switch"), "{after}");
}

/// A span over 8 per label is no table
/// (`param_switch_conversion_branch_ratio`).
#[test]
fn test_a_sparse_switch_is_no_table() {
    let cases = [(0, 5), (100, 6), (200, 7), (300, 8)];
    let after = lowered(&joined("i32", &cases, 1), [-1, 0, 50, 100, 200, 250, 300, 301], true);
    assert!(!after.contains("load") && !after.contains("switch"), "{after}");
}

/// Two distinct values are branches, not a table (`m_uniq <= 2`).
#[test]
fn test_two_values_are_no_table() {
    let cases: Vec<(i128, i128)> = (0..12).map(|at| (at, at % 2)).collect();
    let after = lowered(&joined("i32", &cases, 1), -3..20, true);
    assert!(!after.contains("load"), "{after}");
}

/// Wide values keep their width in the array; a negative one is sign extended.
#[test]
fn test_a_table_of_negative_values_is_sign_extended() {
    let cases: Vec<(i128, i128)> = (0..8).map(|at| (at, [-1, -2, -3, 4, 5, 6, 7, 8][at as usize])).collect();
    let after = lowered(&joined("i32", &cases, 0), -3..12, true);
    assert!(after.contains("sext i8"), "{after}");
}

/// A target other than the join keeps what it gave: the switch's phis name
/// the new blocks that reach them.
#[test]
fn test_a_target_with_a_phi_names_the_blocks_that_reach_it() {
    let text = "define i32 @f(i32 %x) {
b0:
  switch i32 %x, label %d [
    i32 10, label %a
    i32 20, label %a
    i32 30, label %b
    i32 40, label %a
    i32 50, label %b
    i32 60, label %c
  ]

a:
  %pa = phi i32 [ 1, %b0 ], [ 1, %b0 ], [ 1, %b0 ]
  ret i32 %pa

b:
  %pb = phi i32 [ 2, %b0 ], [ 2, %b0 ]
  ret i32 %pb

c:
  %pc = phi i32 [ 3, %b0 ]
  ret i32 %pc

d:
  ret i32 0
}
";
    let after = lowered(text, (0..70).step_by(5).chain([-1, 11, 59, 61]), true);
    assert!(!after.contains("switch"), "{after}");
}

/// Many singles are a balanced tree, not a chain: no path tests more than
/// three or four values.
#[test]
fn test_twenty_sparse_cases_are_a_balanced_tree() {
    let cases: Vec<(i128, u32)> = (0..20).map(|at| (at * 37 - 100, at as u32 % 7 + 1)).collect();
    let inputs = (0..20).flat_map(|at| [at * 37 - 101, at * 37 - 100, at * 37 - 99]);
    let after = lowered(&returns("i32", &cases), inputs, true);
    assert!(!after.contains("switch"), "{after}");
}

/// A selector of a byte: ranges of neighbours, the way a character class is.
#[test]
fn test_a_character_class_is_a_few_ranges() {
    let mut cases: Vec<(i128, u32)> = Vec::new();
    cases.extend((48..58).map(|c| (c, 1)));
    cases.extend((97..103).map(|c| (c, 2)));
    cases.extend((103..123).map(|c| (c, 3)));
    cases.extend((65..91).map(|c| (c, 4)));
    cases.extend([(9, 5), (10, 5), (32, 5), (95, 6)]);
    let after = lowered(&returns("i8", &cases), -128..128, true);
    assert!(!after.contains("switch"), "{after}");
}

/// Only the default: a jump.
#[test]
fn test_a_switch_of_the_default_alone_is_a_jump() {
    let text = "define i32 @f(i32 %x) {
b0:
  switch i32 %x, label %d [ i32 1, label %d ]

d:
  ret i32 7
}
";
    let after = lowered(text, [0, 1, 2], true);
    assert!(!after.contains("switch"), "{after}");
}

/// A phi names its predecessor once for each edge into it: `switch_empty_cases`
/// had three labels to one target, three entries, and one edge once they were a
/// range (the verifier: "a phi has 5 inputs for 1 edges").
#[test]
fn test_a_phi_names_the_switch_once_for_each_edge_that_remains() {
    let text = "define i32 @f(i32 %x) {
b0:
  switch i32 %x, label %d [
    i32 1, label %a
    i32 2, label %a
    i32 3, label %a
    i32 40, label %b
  ]

a:
  %pa = phi i32 [ 7, %b0 ], [ 7, %b0 ], [ 7, %b0 ]
  ret i32 %pa

b:
  ret i32 5

d:
  ret i32 0
}
";
    let after = lowered(text, [0, 1, 2, 3, 4, 40], true);
    assert!(!after.contains("switch"), "{after}");
}

/// Cases that each go their own way, dense, are a jump table where the target
/// has one (gcc's `jump_table_cluster`, from 5 cases): a range test, then a
/// `switch` the backend lowers.
#[test]
fn test_dense_cases_are_a_jump_table_where_the_target_has_one() {
    let cases: Vec<(i128, u32)> = (0..8).map(|at| (at + 10, at as u32 + 1)).collect();
    let after = lowered_by(&returns("i32", &cases), -3..30, true, 5);
    assert!(after.contains("switch table i16"), "{after}");
    assert!(after.contains("icmp ugt i32"), "{after}");
}

/// A target with no table jump keeps the tree.
#[test]
fn test_no_jump_table_where_the_target_has_none() {
    let cases: Vec<(i128, u32)> = (0..8).map(|at| (at + 10, at as u32 + 1)).collect();
    let after = lowered_by(&returns("i32", &cases), -3..30, true, 0);
    assert!(!after.contains("switch"), "{after}");
}

/// LLVM's `getMinimumJumpTableEntries` on x86 is 4: four dense cases are a
/// table, three stay a tree.
#[test]
fn test_four_cases_are_a_jump_table_and_three_are_not() {
    let four: Vec<(i128, u32)> = (0..4).map(|at| (at + 10, at as u32 + 1)).collect();
    let after = lowered_by(&returns("i32", &four), -3..30, true, 5);
    assert!(after.contains("switch table"), "{after}");
    let three: Vec<(i128, u32)> = (0..3).map(|at| (at + 10, at as u32 + 1)).collect();
    let after = lowered_by(&returns("i32", &three), -3..30, true, 5);
    assert!(!after.contains("switch"), "{after}");
}

/// Holes in the table go to the default; a span over 8 per comparison is no
/// table.
#[test]
fn test_a_table_with_holes_sends_them_to_the_default() {
    let cases: Vec<(i128, u32)> =
        [0, 1, 3, 4, 6, 9].iter().enumerate().map(|(at, &value)| (value, at as u32 + 1)).collect();
    let after = lowered_by(&returns("i32", &cases), -2..14, true, 5);
    assert!(after.contains("switch table"), "{after}");
    let sparse: Vec<(i128, u32)> =
        [0, 20, 40, 60, 80, 100].iter().enumerate().map(|(at, &value)| (value, at as u32 + 1)).collect();
    let after = lowered_by(&returns("i32", &sparse), (-2..110).step_by(1), true, 5);
    assert!(!after.contains("switch"), "{after}");
}

/// Each index lands in a block of its own that jumps on, so a phi of the
/// target names it, not the switch's block.
#[test]
fn test_a_table_target_with_a_phi_names_its_landing() {
    let text = "define i32 @f(i32 %x) {
b0:
  switch i32 %x, label %d [
    i32 0, label %a
    i32 1, label %b
    i32 2, label %a
    i32 3, label %c
    i32 4, label %b
    i32 5, label %a
    i32 6, label %e
    i32 7, label %e
  ]

a:
  %pa = phi i32 [ 1, %b0 ], [ 1, %b0 ], [ 1, %b0 ]
  ret i32 %pa

b:
  %pb = phi i32 [ 2, %b0 ], [ 2, %b0 ]
  ret i32 %pb

c:
  %pc = phi i32 [ 3, %b0 ]
  ret i32 %pc

e:
  %pe = phi i32 [ 4, %b0 ], [ 4, %b0 ]
  ret i32 %pe

d:
  ret i32 0
}
";
    let after = lowered_by(text, -2..10, true, 5);
    assert!(after.contains("switch table"), "{after}");
}

/// x_switch_ranges: 58 case values over a span of 114 in 7 clusters were a tree
/// of compares (2.09x clang's instructions): LLVM's lookup table at 51%
/// density, priced under that tree.
#[test]
fn test_a_character_class_is_a_lookup_table() {
    let mut cases: Vec<(i128, i128)> = Vec::new();
    cases.extend((48..58).map(|c| (c, 1)));
    cases.extend((97..103).map(|c| (c, 2)));
    cases.extend((103..123).map(|c| (c, 3)));
    cases.extend((65..91).map(|c| (c, 4)));
    cases.extend([(9, 5), (10, 5), (32, 5), (95, 6)]);
    let after = lowered(&joined("i32", &cases, 0), -3..130, true);
    assert!(after.contains("load i8") && !after.contains("switch i32"), "{after}");
}

/// The price decides: where the load costs more than the compares of the tree
/// the table is not made.
#[test]
fn test_a_table_the_target_prices_over_the_tree_is_not_made() {
    let cases: Vec<(i128, i128)> = (0..12).map(|at| (at, at * 7 + 1)).collect();
    let text = joined("i32", &cases, 1);
    let cheap = priced(&text, -3..20, i486(), i486(), false);
    assert!(cheap.contains("load"), "{cheap}");
    let dear = priced(&text, -3..20, crate::profit::OperationCosts { load: 40, ..i486() }, i486(), false);
    assert!(!dear.contains("load"), "{dear}");
}

/// LLVM's density: 10% of the span (40% tuned for size): 3 cases over 20 is a
/// table never, 4 over 30 is one only tuned for speed's 10%... and 40% in
/// bytes.
#[test]
fn test_the_least_density_is_a_tenth_and_two_fifths_tuned_for_size() {
    // 6 values over a span of 30: 20%.
    let cases: Vec<(i128, i128)> = [0, 6, 12, 18, 24, 29].iter().map(|&at| (at, at + 1)).collect();
    let text = joined("i32", &cases, 1);
    let bytes =
        crate::profit::OperationCosts { add: 3, branch: 2, load: 6, r#move: 5, extend: 3, ..Default::default() };
    let speed = priced(&text, -2..40, i486(), bytes.clone(), false);
    assert!(speed.contains("load"), "{speed}");
    let size = priced(&text, -2..40, i486(), bytes, true);
    assert!(!size.contains("load"), "{size}");
}

/// `lowerWorkItem`: two values to one target one bit apart are one compare,
/// `(x | 2) == 6`, not two.
#[test]
fn test_two_values_one_bit_apart_are_one_compare() {
    let after = lowered(&returns("i32", &[(4, 1), (6, 1)]), -2..10, false);
    assert!(after.contains("or i32 %x, 2") && after.contains("icmp eq i32 %0, 6"), "{after}");
}

/// `peelDominantCaseCluster`: a range holding two cases in three is tested
/// first, ahead of the tree of the rest.
#[test]
fn test_a_dominant_range_is_tested_first() {
    let mut cases: Vec<(i128, u32)> = (0..10).map(|at| (at, 1)).collect();
    cases.extend([(100, 2), (200, 3), (300, 4)]);
    let after = lowered(&returns("i32", &cases), (-2..12).chain([100, 150, 200, 300, 301]), false);
    let first = after.lines().find(|line| line.contains("icmp")).unwrap();
    assert!(first.contains("icmp ule i32 %x, 9"), "{after}");
}
