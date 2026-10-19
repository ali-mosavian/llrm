//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=memcpyopt` makes of the same body
//! (checked with `tools/difftest`).

use super::MemCpyOpt;
use crate::testing::{managed, parsed};

const DECLARES: &str = "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
";

const GLOBALS: &str = "@a = global [64 x i8] zeroinitializer
@b = global [64 x i8] zeroinitializer
@c = global [64 x i8] zeroinitializer
@k = constant [64 x i8] zeroinitializer
";

/// @f after the pass, from `define void @f`.
fn after_f(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}\n{GLOBALS}\n{body}"));
    let text = managed(&mut module, MemCpyOpt);
    text[text.find("define void @f").expect("@f")..].replace("b0:\n", "")
}

fn memcpy(
    to: &str,
    from: &str,
    bytes: u32,
) -> String {
    format!("  call void @llvm.memcpy.p0.p0.i64(ptr {to}, ptr {from}, i64 {bytes}, i1 false)\n")
}

fn body(lines: &[String]) -> String {
    format!("define void @f() {{\nb0:\n{}  ret void\n}}\n", lines.concat())
}

/// A memcpy of a pointer to itself is a no-op: gone.
#[test]
fn test_a_memcpy_to_itself_is_removed() {
    assert_eq!(after_f(&body(&[memcpy("@a", "@a", 16)])), "define void @f() {\n  ret void\n}\n");
}

/// A memcpy of no bytes is gone.
#[test]
fn test_a_memcpy_of_no_bytes_is_removed() {
    assert_eq!(after_f(&body(&[memcpy("@b", "@a", 0)])), "define void @f() {\n  ret void\n}\n");
}

/// A memcpy of what a memcpy copied copies from the first one's source.
#[test]
fn test_a_memcpy_of_a_memcpy_copies_from_its_source() {
    let before = body(&[memcpy("@b", "@a", 16), memcpy("@c", "@b", 16)]);
    let expected =
        "define void @f() {\n".to_owned() + &memcpy("@b", "@a", 16) + &memcpy("@c", "@a", 16) + "  ret void\n}\n";
    assert_eq!(after_f(&before), expected);
}

/// A write to the first copy's source between the two copies keeps the second
/// reading the first copy's destination.
#[test]
fn test_a_write_to_the_source_between_stops_the_forwarding() {
    let before = body(&[memcpy("@b", "@a", 16), "  store i8 1, ptr @a\n".to_owned(), memcpy("@c", "@b", 16)]);
    assert_eq!(after_f(&before), before.replace("b0:\n", ""));
}

/// A copy out of a constant that repeats one byte is a memset of that byte.
#[test]
fn test_a_memcpy_from_a_zero_constant_is_a_memset() {
    assert_eq!(
        after_f(&body(&[memcpy("@b", "@k", 16)])),
        "define void @f() {\n  call void @llvm.memset.p0.i64(ptr @b, i8 0, i64 16, i1 false)\n  ret void\n}\n"
    );
}

/// A memset followed by a memcpy over its start only needs to set the rest.
#[test]
fn test_a_memset_only_sets_what_the_memcpy_after_it_leaves() {
    let before =
        body(&["  call void @llvm.memset.p0.i64(ptr @a, i8 0, i64 64, i1 false)\n".to_owned(), memcpy("@a", "@b", 16)]);
    assert_eq!(
        after_f(&before),
        "define void @f() {
  %0 = getelementptr i8, ptr @a, i64 16
  call void @llvm.memset.p0.i64(ptr align 1 %0, i8 0, i64 48, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @b, i64 16, i1 false)
  ret void
}
"
    );
}

/// A memset as long as the memcpy after it is overwritten whole: gone.
#[test]
fn test_a_memset_the_memcpy_overwrites_whole_is_removed() {
    let before =
        body(&["  call void @llvm.memset.p0.i64(ptr @a, i8 0, i64 16, i1 false)\n".to_owned(), memcpy("@a", "@b", 16)]);
    assert_eq!(after_f(&before), "define void @f() {\n".to_owned() + &memcpy("@a", "@b", 16) + "  ret void\n}\n");
}

/// A memcpy of bytes a memset set is a memset of them.
#[test]
fn test_a_memcpy_of_a_memset_is_a_memset() {
    let before =
        body(&["  call void @llvm.memset.p0.i64(ptr @a, i8 1, i64 32, i1 false)\n".to_owned(), memcpy("@b", "@a", 16)]);
    assert_eq!(
        after_f(&before),
        "define void @f() {
  call void @llvm.memset.p0.i64(ptr @a, i8 1, i64 32, i1 false)
  call void @llvm.memset.p0.i64(ptr @b, i8 1, i64 16, i1 false)
  ret void
}
"
    );
}

/// A copy out of an alloca nothing wrote reads undefined bytes: gone.
#[test]
fn test_a_memcpy_of_an_unwritten_alloca_is_removed() {
    let before =
        "define void @f() {\nb0:\n  %t = alloca [16 x i8]\n".to_owned() + &memcpy("@b", "%t", 16) + "  ret void\n}\n";
    assert_eq!(after_f(&before), "define void @f() {\n  %t = alloca [16 x i8]\n  ret void\n}\n");
}

/// A memmove whose source cannot meet its destination is a memcpy.
#[test]
fn test_a_memmove_between_distinct_objects_is_a_memcpy() {
    let before = body(&["  call void @llvm.memmove.p0.p0.i64(ptr @b, ptr @a, i64 16, i1 false)\n".to_owned()]);
    assert_eq!(after_f(&before), "define void @f() {\n".to_owned() + &memcpy("@b", "@a", 16) + "  ret void\n}\n");
}

/// A memmove over its own source stays a memmove.
#[test]
fn test_a_memmove_over_its_own_source_stays() {
    let text = "define void @f() {
b0:
  %p = getelementptr i8, ptr @a, i64 4
  call void @llvm.memmove.p0.p0.i64(ptr %p, ptr @a, i64 16, i1 false)
  ret void
}
";
    assert_eq!(after_f(text), text.replace("b0:\n", ""));
}

/// A volatile memcpy is left alone.
#[test]
fn test_a_volatile_memcpy_is_left_alone() {
    let text =
        "define void @f() {\nb0:\n  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @a, i64 16, i1 true)\n  ret void\n}\n";
    assert_eq!(after_f(text), text.replace("b0:\n", ""));
}
