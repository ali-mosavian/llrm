//! `llvm.trap` and `llvm.debugtrap` on the DAG path alone.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// The legalizer marked `TRAP` and `DEBUGTRAP` Expand and refused them ("convert node to libcall"), so
/// `tests/check/c/builtin_trap.c` fell back to the hand selector at every level. The pin selects `brk #1` and
/// `brk #0xf000` (`AArch64InstrInfo.td`: `trap`, `debugtrap`).
#[test]
fn trap_and_debugtrap_are_brk_on_the_dag_path() {
    let ir = format!(
        "{LAYOUT}declare void @llvm.trap()\ndeclare void @llvm.debugtrap()\n\
define void @t() {{\n  call void @llvm.trap()\n  unreachable\n}}\n\
define void @d() {{\n  call void @llvm.debugtrap()\n  ret void\n}}\n"
    );
    let asm = driver::assembly(&ir, Platform::Linux, true).expect("the DAG path selects trap and debugtrap");
    assert!(asm.contains("brk #1\n"), "{asm}");
    assert!(asm.contains("brk #61440\n"), "{asm}");
}
