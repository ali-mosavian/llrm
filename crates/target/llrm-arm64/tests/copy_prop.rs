//! A copy that puts a value back where it already is is dropped.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

/// The argument was copied into the callee-saved `x19` and straight back into
/// `x0` for the call: one `mov` too many in every recursion of recpow's
/// `power` (the pin's MachineCopyPropagation drops it).
#[test]
fn a_value_copied_out_and_straight_back_is_copied_once() {
    let asm = driver::assembly(include_str!("data/shrink_wrap_loop.ll"), Platform::Linux, Level::O2).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let out = lines.iter().position(|line| *line == "orr x19, xzr, x0, lsl #0").expect("the copy out");
    assert_ne!(lines[out + 1], "orr x0, xzr, x19, lsl #0", "{asm}");
}

/// Fourteen accumulators start at 0 across calls. Their phi copies read one
/// shared zero, so `orr x9, xzr, x10` chains were kept (and spilled) where the
/// pin makes the zero again at each copy (`reMaterializeTrivialDef`): +9
/// instructions in jpeg_single -Os.
#[test]
fn a_copy_of_the_zero_register_is_made_again_not_copied_from_one_shared_zero() {
    let asm = driver::assembly(include_str!("data/zero_spill.ll"), Platform::Linux, Level::O1).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let first =
        lines.iter().find_map(|line| line.strip_prefix("orr ")?.strip_suffix(", xzr, xzr, lsl #0")).expect("a zero");
    let shared = format!(", xzr, {first}, lsl #0");
    assert!(!lines.iter().any(|line| line.starts_with("orr ") && line.ends_with(&shared)), "{asm}");
}

/// A zero accumulator that spills before it is first used was made in a
/// register and stored (`orr x9, xzr, xzr; str x9, [sp, N]`): the pin folds the
/// spill of a copy from `xzr` into `str xzr` (`foldMemoryOperandImpl`); two of
/// them pair into `stp`.
#[test]
fn a_spilled_zero_is_stored_from_the_zero_register() {
    let asm = driver::assembly(include_str!("data/zero_spill.ll"), Platform::Linux, Level::O1).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    assert!(
        lines.iter().any(|line| line.starts_with("str xzr, [sp") || line.starts_with("stp xzr, xzr, [sp")),
        "{asm}"
    );
    let zeroed = lines
        .windows(2)
        .any(
            |pair| pair[0]
                .strip_prefix("orr ")
                .and_then(|rest| rest.strip_suffix(", xzr, xzr, lsl #0"))
                .is_some_and(|reg| pair[1].starts_with(&format!("str {reg}, [sp"))),
        );
    assert!(!zeroed, "{asm}");
}

/// The spilled-zero store kept the remat flag of the definition it was cloned
/// from, greedy refused the function ("no definition of the function") and the
/// linear scan left `add x9, x10, #1; mov x10, x9` in jpeg_single's inner
/// loops: +19% instructions at -Os.
#[test]
fn a_function_with_a_spilled_zero_stays_on_greedy() {
    let before = driver::greedy_fallbacks();
    driver::assembly(include_str!("data/zero_spill_os.ll"), Platform::Linux, Level::O2).expect("lowers");
    assert_eq!(driver::greedy_fallbacks(), before, "fell back to the linear scan");
}

/// The loop of `gcd` copied the divisor (`mov x0, x1`) and divided by the copy
/// (`sdiv w10, w9, w0`): the pin's MachineCopyPropagation reads the copy's
/// source (`sdiv w10, w9, w1`), so the divide no longer waits on the `mov`.
/// recgcd -Os ran 13% more modelled clocks than the pin on the same IR.
#[test]
fn a_register_read_after_its_copy_is_read_from_the_copy_source() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
              target triple = \"aarch64-unknown-linux-gnu\"\n\
              define i32 @g(i32 %a0, i32 %b0) {\nentry:\n  br label %loop\nloop:\n  %b = phi i32 [ %b0, %entry ], [ %r, %loop ]\n  \
              %a = phi i32 [ %a0, %entry ], [ %b, %loop ]\n  %r = srem i32 %a, %b\n  %done = icmp eq i32 %r, 0\n  \
              br i1 %done, label %exit, label %loop\nexit:\n  ret i32 %b\n}\n";
    let asm = driver::assembly(ir, Platform::Linux, Level::O2).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let divide = lines.iter().position(|line| line.starts_with("sdiv ")).expect("a divide");
    let divisor = lines[divide].rsplit(", ").next().unwrap().to_owned();
    let copied = format!("orr x{}, xzr, ", &divisor[1..]);
    assert!(!lines[..divide].iter().any(|line| line.starts_with(&copied)), "the divisor {divisor} is a copy: {asm}");
}

/// Two calls and a third that reads what the first returned: the copy of the
/// first result into a register the second callee leaves alone (IPRA) was
/// dropped as dead, because the call between forgot the copy and the read
/// after it found none. computed_goto printed a stale `x1`.
#[test]
fn a_copy_read_after_a_call_is_kept() {
    let ir = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
              target triple = \"aarch64-unknown-linux-gnu\"\n\
              define internal i64 @leaf(i64 %x) noinline nounwind {\n  %r = add i64 %x, 1\n  ret i64 %r\n}\n\
              declare void @use(i64, i64)\n\
              define i32 @f() {\n  %a = call i64 @leaf(i64 1)\n  %b = call i64 @leaf(i64 2)\n  \
              call void @use(i64 %a, i64 %b)\n  ret i32 0\n}\n";
    let asm = driver::assembly(ir, Platform::Linux, Level::O2).expect("lowers");
    let lines: Vec<&str> = asm.lines().map(str::trim).collect();
    let calls: Vec<usize> =
        lines.iter().enumerate().filter(|(_, line)| line.starts_with("bl ")).map(|(at, _)| at).collect();
    assert_eq!(calls.len(), 3, "{asm}");
    assert!(
        lines[calls[0]..calls[1]].iter().any(|line| line.starts_with("orr x") && line.ends_with(", xzr, x0, lsl #0")),
        "{asm}"
    );
}
