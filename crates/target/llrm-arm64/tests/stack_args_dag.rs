//! Stack arguments of a call, on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// The stores of the stack arguments went through a copy of the stack pointer (`mov x9, sp; stp x11, x10, [x9]`),
/// an instruction more than the pin's `stp x11, x10, [sp]`: `InstrEmitter` reads a reserved register where it is
/// used, with no copy.
#[test]
fn stack_arguments_are_stored_from_the_stack_pointer_itself() {
    let ir = format!(
        "{LAYOUT}declare i64 @many(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)\n\
define i64 @f(i64 %a) {{\n  %r = call i64 @many(i64 %a, i64 2, i64 3, i64 4, i64 5, i64 6, i64 7, i64 8, i64 9, i64 10)\n  ret i64 %r\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("[sp]"), "{asm}");
    let copies = |line: &&str| {
        let line = line.trim();
        (line.starts_with("mov ") && line.ends_with(", sp")) || (line.starts_with("add x") && line.contains(", sp, #0"))
    };
    assert!(!asm.lines().any(|line| copies(&line)), "no copy of the stack pointer:\n{asm}");
}
