//! A call with stack arguments in a function whose stack pointer moves, on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// "a call with stack arguments in a function that moves the stack pointer" refused it, so `tests/darwin/vla` (a
/// `vla` inlined into `main`, which calls `many` with eleven arguments) fell back to the hand selector at O1-Os. The
/// pin (`hasReservedCallFrame` false) lowers the call frame pseudos to `sub sp, sp, #16` and `add sp, sp, #16`
/// around the call, so the arguments are not stored over the dynamic `alloca`.
#[test]
fn a_stack_argument_call_after_a_dynamic_alloca_makes_room_around_the_call() {
    let ir = format!(
        "{LAYOUT}declare i64 @many(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)\ndeclare void @use(ptr)\n\
define i64 @f(i64 %n) {{\n  %v = alloca i32, i64 %n, align 4\n  call void @use(ptr %v)\n  \
%r = call i64 @many(i64 1, i64 2, i64 3, i64 4, i64 5, i64 6, i64 7, i64 8, i64 9, i64 10)\n  call void @use(ptr %v)\n  \
ret i64 %r\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    // The room opens as `sub sp, sp, #16` or, folded into the first argument store, `[sp, #-16]!`.
    let down = asm
        .find("sub sp, sp, #16")
        .or_else(|| asm.find("[sp, #-16]!"))
        .unwrap_or_else(|| panic!("no room made for the arguments:\n{asm}"));
    let call = asm.find("bl many").expect("the call");
    let up = asm.rfind("add sp, sp, #16").expect("the room given back");
    assert!(down < call && call < up, "{asm}");
}
