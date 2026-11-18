//! A constant held in a register that has several definitions: the loop's
//! `i32.const 1` is read where the result register's first value is.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

/// `for (i = -204; ; i++)`: a mismatch leaves with 1, the end of the count with
/// 0. The coalescer gives the `1` of the exit `phi` and the loop's `+ 1` one
/// register, which also takes the `0` after the loop.
const SAME: &str = "@a = external global [300 x i8]
@b = external global [300 x i8]
declare void @report(i32)
declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32)
define i32 @main() optsize {
  br label %3
1:
  %2 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %4, i32 1)
  %math = extractvalue { i32, i1 } %2, 0
  %ov = extractvalue { i32, i1 } %2, 1
  br i1 %ov, label %12, label %3
3:
  %4 = phi i32 [ %math, %1 ], [ -204, %0 ]
  %5 = getelementptr i8, ptr @a, i32 %4
  %6 = getelementptr i8, ptr %5, i32 204
  %7 = load i8, ptr %6, align 1
  %8 = getelementptr i8, ptr @b, i32 %4
  %9 = getelementptr i8, ptr %8, i32 204
  %10 = load i8, ptr %9, align 1
  %11 = icmp eq i8 %7, %10
  br i1 %11, label %1, label %12
12:
  %13 = phi i32 [ 1, %3 ], [ 0, %1 ]
  tail call void @report(i32 %13)
  ret i32 0
}
";

fn body(text: &str) -> &str {
    let start = text.find("main:").expect("the function");
    &text[start..start + text[start..].find("end_function").expect("its end")]
}

/// The loop added the local the `1` of the exit value lives in (`local.get 1`)
/// where the pin reads the constant again: memcpy_libcall -Os, `i32.const 1`
/// kept a `local.set` and a `local.get` alive across the loop.
#[test]
fn a_loop_constant_in_a_register_with_several_definitions_is_a_constant_again() {
    let text = assembly(SAME, Width::W32, Options { level: Level::Os, ..Options::default() }).unwrap();
    let main = body(&text);
    assert!(main.contains("local.get\t0\n\ti32.const\t1\n\ti32.add"), "{main}");
    assert!(!main.contains("local.get\t1\n\ti32.add"), "{main}");
}

/// The exit from the mismatch went through a block with nothing in it, which
/// cost a `block`, a `br` and an `end` around the loop: memcpy_libcall -Os,
/// +10 B over the pin.
#[test]
fn the_loop_needs_one_block_for_its_two_exits() {
    let text = assembly(SAME, Width::W32, Options { level: Level::Os, ..Options::default() }).unwrap();
    assert_eq!(body(&text).matches("\tblock").count(), 1, "{text}");
}
