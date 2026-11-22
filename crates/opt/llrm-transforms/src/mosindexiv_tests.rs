//! A pointer walked by a byte step, whose offset stays in a byte, is a base
//! plus a byte counter (llvm-mos `MOSIndexIV`).
//!
//! Each test runs @f through `mosindexiv` on a 16-bit-pointer datalayout and
//! compares what the interpreter computes before and after.

use crate::mosindexiv::MosIndexIv;
use crate::testing::{Tuned, managed_on, parsed, printed, results};

const DL: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-n8:16\"\n\n";

/// `sum += buf[i]` for `i` in `0..trips` over a global of 300 bytes.
fn walk(trips: u32) -> String {
    format!(
        "{DL}@buf = global [300 x i8] c\"{}\"

define i32 @f(i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %s = phi i32 [ %n, %entry ], [ %acc, %loop ]
  %p = getelementptr inbounds i8, ptr @buf, i16 %i
  %b = load i8, ptr %p
  %w = zext i8 %b to i32
  %acc = add i32 %s, %w
  %next = add nuw nsw i16 %i, 1
  %c = icmp ult i16 %next, {trips}
  br i1 %c, label %loop, label %exit
exit:
  %r = phi i32 [ %acc, %loop ]
  ret i32 %r
}}
",
        "\\01".repeat(300)
    )
}

fn run(
    text: &str,
    index_iv_width: Option<u32>,
) -> String {
    let mut module = parsed(text);
    let inputs: &[&[i128]] = &[&[0], &[7]];
    let before = results(&module, inputs);
    managed_on(&mut module, MosIndexIv, Tuned { index_iv_width, ..Default::default() });
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    after
}

/// A 200-trip walk keeps its base and a byte index: the address is
/// `gep i8 @buf, zext i8`, not a 16-bit counter.
#[test]
fn test_a_byte_range_walk_is_a_base_and_a_byte_index() {
    let after = run(&walk(200), Some(8));
    assert!(after.contains("phi i8"), "{after}");
    assert!(after.contains("getelementptr i8, ptr @buf, i16 %"), "{after}");
    assert!(after.contains("zext i8"), "{after}");
}

/// A 300-trip walk reaches offset 299: no byte holds it, the loop is as it was.
#[test]
fn test_a_walk_past_a_byte_is_left_alone() {
    let after = run(&walk(300), Some(8));
    assert!(!after.contains("phi i8"), "{after}");
}

/// A target without an index width keeps every address.
#[test]
fn test_no_index_width_changes_nothing() {
    let after = run(&walk(200), None);
    assert!(!after.contains("phi i8"), "{after}");
}
