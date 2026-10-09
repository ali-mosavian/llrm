//! Adapted from llrm-core's `optimize/cfg.rs` tests, the port of
//! `tests/test_cfg_merge.py`; which tests stay behind is in `cfg.rs`.

use super::merged;
use crate::testing::{f, parsed, printed, results};

/// `text` merged, printed, with `inputs` computing what they did before.
fn merged_text(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert!(merged(f(&mut module)), "merges");
    assert_eq!(results(&module, inputs), before);
    assert!(!merged(f(&mut module)), "a fixed point");
    printed(&module)
}

/// `text` is left alone.
fn unchanged(text: &str) {
    let mut module = parsed(text);
    let before = printed(&module);
    assert!(!merged(f(&mut module)));
    assert_eq!(printed(&module), before);
}

const CHAIN: &str = "define i16 @f(i16 %x) {
b0:
  %v = add i16 %x, 7
  br label %b1

b1:
  %joined = phi i16 [ %v, %b0 ]
  ret i16 %joined
}
";

/// A constant passed through a statement join must remain the same PRINT
/// argument.
#[test]
fn test_single_entry_phi_is_replaced() {
    assert_eq!(
        merged_text(CHAIN, &[&[0], &[5], &[-7]]),
        "define i16 @f(i16 %x) {
b0:
  %v = add i16 %x, 7
  ret i16 %v
}
"
    );
}

/// Another predecessor keeps the join, from the old
/// test_merge_preserves_alternate_entries_and_layout.
#[test]
fn test_merge_preserves_a_join_with_another_predecessor() {
    unchanged(
        "define i16 @f(i16 %x) {
b0:
  %v = add i16 %x, 7
  br label %b1

b2:
  br label %b1

b1:
  %joined = phi i16 [ %v, %b0 ], [ 0, %b2 ]
  ret i16 %joined
}
",
    );
}

/// A later join must still receive the value from the merged path.
#[test]
fn test_successor_phi_edge_is_renamed_to_the_surviving_block() {
    assert_eq!(
        merged_text(
            "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b3

b1:
  %v = add i16 %x, 7
  br label %b2

b2:
  %w = mul i16 %v, 3
  br label %b4

b3:
  br label %b4

b4:
  %r = phi i16 [ %w, %b2 ], [ %x, %b3 ]
  ret i16 %r
}
",
            &[&[0, 4], &[1, 4], &[1, -9]],
        ),
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b3

b1:
  %v = add i16 %x, 7
  %w = mul i16 %v, 3
  br label %b4

b3:
  br label %b4

b4:
  %r = phi i16 [ %w, %b1 ], [ %x, %b3 ]
  ret i16 %r
}
"
    );
}

/// A chain merges whole, whatever its blocks' layout order.
#[test]
fn test_chain_merges_into_one_block_across_layout_order() {
    assert_eq!(
        merged_text(
            "define i16 @f(i16 %x) {
b0:
  br label %b2

b1:
  %t = phi i16 [ %v, %b2 ]
  %r = shl i16 %t, 1
  ret i16 %r

b2:
  %v = sub i16 %x, 1
  br label %b1
}
",
            &[&[0], &[3], &[-2]],
        ),
        "define i16 @f(i16 %x) {
b0:
  %v = sub i16 %x, 1
  %r = shl i16 %v, 1
  ret i16 %r
}
"
    );
}

/// A loop body's chain merges; the header, with two predecessors, stays.
#[test]
fn test_loop_body_chain_merges_and_header_stays() {
    assert_eq!(
        merged_text(
            "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %s = phi i16 [ 0, %b0 ], [ %sum, %b3 ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %b2, label %b4

b2:
  %sum = add i16 %s, %i
  br label %b3

b3:
  %next = add i16 %i, 1
  br label %b1

b4:
  ret i16 %s
}
",
            &[&[0], &[1], &[5]],
        ),
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %more = icmp ult i16 %i, %n
  br i1 %more, label %b2, label %b4

b2:
  %sum = add i16 %s, %i
  %next = add i16 %i, 1
  br label %b1

b4:
  ret i16 %s
}
"
    );
}

/// Only a jump merges: a two-way branch or a switch to one block stays.
#[test]
fn test_only_an_unconditional_jump_merges() {
    unchanged(
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b1

b1:
  ret i16 %x
}
",
    );
    unchanged(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b1 []

b1:
  ret i16 %x
}
",
    );
}

/// A block jumping to itself is its own only predecessor, not a chain.
#[test]
fn test_block_jumping_to_itself_stays() {
    unchanged(
        "define i16 @f(i16 %x) {
b0:
  ret i16 %x

b1:
  br label %b1
}
",
    );
}
