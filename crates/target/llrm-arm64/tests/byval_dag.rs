//! `byval` parameters and arguments on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn selects(body: &str) -> String {
    driver::assembly(&format!("{LAYOUT}{body}"), Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"))
}

/// `byval` was refused ("a parameter attribute that moves it"), so
/// `tests/data/tail_calls` fell back to the hand selector. The pin makes the
/// parameter the address of a fixed object in the caller's argument area
/// (`LowerFormalArguments`, `CCPassByVal<8, 8>`), not a pointer passed in `x0`.
#[test]
fn a_byval_parameter_is_read_from_the_argument_area_not_through_x0() {
    let asm = selects(
        "%t = type { i64, i64, i64 }\ndefine i64 @f(ptr byval(%t) %r, i64 %n) {\n  %p = getelementptr %t, ptr %r, i32 0, i32 1\n  %v = load i64, ptr %p\n  %s = add i64 %v, %n\n  ret i64 %s\n}\n",
    );
    // `%n` stays in `x0`: the aggregate takes no register, and its second field
    // is at `sp + 8`.
    assert!(asm.contains("[sp, #8]"), "{asm}");
    assert!(!asm.contains("[x0"), "the aggregate is not read through x0:\n{asm}");
}

/// The caller copies the bytes to the top of its outgoing area (`getMemcpy` at
/// the stack pointer).
#[test]
fn a_byval_argument_is_copied_to_the_stack_before_the_call() {
    let asm = selects(
        "%t = type { i64, i64, i64 }\ndeclare void @g(ptr byval(%t), i64)\ndefine void @f(ptr %r) {\n  call void @g(ptr byval(%t) %r, i64 5)\n  ret void\n}\n",
    );
    let call = asm.find("bl g").unwrap_or_else(|| panic!("{asm}"));
    assert!(asm[..call].contains("[sp"), "the bytes are stored to the stack before the call:\n{asm}");
    assert!(asm[..call].contains("#5"), "{asm}");
    assert!(!asm.lines().any(|line| line.trim().starts_with("mov x") && line.trim().ends_with(", sp")), "{asm}");
}

/// `isEligibleForTailCallOptimization`: a caller's `byval` parameter is a
/// pointer into the area a tail call would reuse, so the call stays a call.
#[test]
fn a_caller_with_a_byval_parameter_makes_no_tail_call() {
    let asm = selects(
        "%t = type { i64, i64 }\ndeclare i64 @g(i64)\ndefine i64 @f(ptr byval(%t) %p) {\n  %r = tail call i64 @g(i64 1)\n  ret i64 %r\n}\n",
    );
    assert!(asm.contains("bl g"), "{asm}");
}
