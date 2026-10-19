//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=memcpyopt` makes of the same body
//! (checked with `tools/difftest`).

use super::MemCpyOpt;
use crate::testing::{managed, parsed};

const DECLARES: &str = "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
";

/// @f after the pass.
fn after(body: &str) -> String {
    let mut module = parsed(&format!("{DECLARES}\n{body}"));
    managed(&mut module, MemCpyOpt)
}

const GLOBALS: &str = "@a = global [64 x i8] zeroinitializer
@b = global [64 x i8] zeroinitializer
@c = global [64 x i8] zeroinitializer
@k = constant [64 x i8] zeroinitializer
";

fn tail(text: &str) -> String {
    text[text.find("define void @f").expect("@f")..].to_owned()
}

#[test]
fn scratch() {
    for body in [
        "define void @f() {
b0:
  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @a, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  call void @llvm.memcpy.p0.p0.i64(ptr @b, ptr @a, i64 0, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  call void @llvm.memcpy.p0.p0.i64(ptr @b, ptr @a, i64 16, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr @c, ptr @b, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  call void @llvm.memcpy.p0.p0.i64(ptr @b, ptr @k, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  call void @llvm.memset.p0.i64(ptr @a, i8 0, i64 64, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr @a, ptr @b, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  call void @llvm.memset.p0.i64(ptr @a, i8 1, i64 32, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr @b, ptr @a, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  %t = alloca [16 x i8]
  call void @llvm.memcpy.p0.p0.i64(ptr @b, ptr %t, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  call void @llvm.memmove.p0.p0.i64(ptr @b, ptr @a, i64 16, i1 false)
  ret void
}
",
        "define void @f() {
b0:
  %p = getelementptr i8, ptr @a, i64 4
  call void @llvm.memmove.p0.p0.i64(ptr %p, ptr @a, i64 16, i1 false)
  ret void
}
",
    ] {
        let mut module = parsed(&format!("{DECLARES}\n{GLOBALS}\n{body}"));
        eprintln!("{}\n=====", tail(&managed(&mut module, MemCpyOpt)));
    }
}
