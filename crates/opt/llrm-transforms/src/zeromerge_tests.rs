//! Tests of `zeromerge`: zeroed locals that follow each other in the entry
//! block.

use llrm_analysis::testing::DOS;

use super::ZeroMerge;
use crate::testing::{managed, parsed};

fn run(body: &str) -> String {
    managed(&mut parsed(&format!("{DOS}declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n{body}")), ZeroMerge)
}

fn fills(text: &str) -> usize {
    text.matches("call void @llvm.memset").count()
}

const THREE: &str = "define i16 @f(i16 %i) {
b0:
  %a = alloca [128 x i8]
  %b = alloca [128 x i8]
  %c = alloca [256 x i8]
  call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 128, i1 false)
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 128, i1 false)
  call void @llvm.memset.p0.i16(ptr %c, i8 0, i16 256, i1 false)
  %p = getelementptr i8, ptr %a, i16 %i
  %v = load i8, ptr %p
  %q = getelementptr i8, ptr %c, i16 %i
  %w = load i8, ptr %q
  %x = zext i8 %v to i16
  %y = zext i8 %w to i16
  %r = add i16 %x, %y
  ret i16 %r
}
";

/// matmul.nib keeps `a`, `b` and `c` zeroed: three `lea di; mov cx; rep
/// stosw` of 7 bytes each beyond the first (314 B against bcc's 298). One
/// fill of the sum is 300.
#[test]
fn test_zeroed_locals_in_a_row_share_one_fill() {
    let after = run(THREE);
    assert_eq!(fills(&after), 1, "{after}");
    assert!(after.contains("i8 0, i16 512"), "{after}");
    assert_eq!(after.matches("alloca").count(), 1, "{after}");
}

/// A member keeps its place in the merged block: `%a` is the second, at 128.
#[test]
fn test_a_member_is_read_at_its_offset_in_the_merged_block() {
    let after = run(THREE);
    assert!(after.contains("getelementptr inbounds i8, ptr %"), "{after}");
    assert!(after.contains(", i16 128"), "{after}");
    assert!(after.contains(", i16 256"), "{after}");
}

/// A fill under 64 bytes may be a run of stores; joined it would become a
/// `rep stos` the stores were smaller than.
#[test]
fn test_small_fills_are_left_alone() {
    let after = run("define void @f(ptr %o) {
b0:
  %a = alloca [16 x i8]
  %b = alloca [16 x i8]
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 16, i1 false)
  store ptr %a, ptr %o
  ret void
}
");
    assert_eq!(fills(&after), 2, "{after}");
}

/// A fill of a value but zero, or of part of the local, is not a zero fill of
/// the whole.
#[test]
fn test_only_whole_zero_fills_join() {
    let after = run("define void @f(ptr %o) {
b0:
  %a = alloca [128 x i8]
  %b = alloca [128 x i8]
  call void @llvm.memset.p0.i16(ptr %a, i8 1, i16 128, i1 false)
  call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 128, i1 false)
  store ptr %a, ptr %o
  ret void
}
");
    assert_eq!(fills(&after), 2, "{after}");
}

/// A fill with code between it and the next is not in the run: the code may
/// read the first local.
#[test]
fn test_fills_with_code_between_do_not_join() {
    let after = run("define void @f(ptr %o) {
b0:
  %a = alloca [128 x i8]
  %b = alloca [128 x i8]
  call void @llvm.memset.p0.i16(ptr %a, i8 0, i16 128, i1 false)
  store ptr %a, ptr %o
  call void @llvm.memset.p0.i16(ptr %b, i8 0, i16 128, i1 false)
  ret void
}
");
    assert_eq!(fills(&after), 2, "{after}");
}
