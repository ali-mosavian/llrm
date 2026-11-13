//! `va_start`, `va_arg` and a variadic call: the unnamed arguments go on the
//! soft stack after the named ones, and `va_start` is the address of the first.
//! Before, any variadic function was `CannotSelect("VAStart")` (2026-11-11).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `ir` compiled, run on sim65 from `main` (raw ca65 that calls `probe`), and
/// the exit code.
fn exit_code(
    name: &str,
    ir: &str,
    target: &str,
) -> i32 {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let compiled = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let functions: String =
        compiled.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    common::run_on(target, name, "  jsr probe\n", &functions).0
}

const PRELUDE: &str = "declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
";

#[test]
fn the_sum_of_unnamed_ints_is_read_back_in_order() {
    let ir = format!(
        "{PRELUDE}define i16 @sum(i16 %n, ...) {{
entry:
  %ap = alloca ptr, align 1
  call void @llvm.va_start.p0(ptr %ap)
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %loop ]
  %acc = phi i16 [ 0, %entry ], [ %acc2, %loop ]
  %v = va_arg ptr %ap, i16
  %acc2 = add i16 %acc, %v
  %i2 = add i16 %i, 1
  %done = icmp eq i16 %i2, %n
  br i1 %done, label %out, label %loop
out:
  call void @llvm.va_end.p0(ptr %ap)
  ret i16 %acc2
}}
define i16 @probe() {{
  %r = call i16 (i16, ...) @sum(i16 3, i16 10, i16 20, i16 31)
  ret i16 %r
}}"
    );
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(exit_code(&format!("va-sum-{target}"), &ir, target), 61, "{target}");
    }
}

/// A long after an int: four bytes read from the next offset, low byte first.
#[test]
fn an_unnamed_long_follows_an_unnamed_int_byte_for_byte() {
    let ir = format!(
        "{PRELUDE}define i16 @mix(i16 %n, ...) {{
  %ap = alloca ptr, align 1
  call void @llvm.va_start.p0(ptr %ap)
  %a = va_arg ptr %ap, i16
  %b = va_arg ptr %ap, i32
  call void @llvm.va_end.p0(ptr %ap)
  %s = lshr i32 %b, 8
  %t = trunc i32 %s to i16
  %r = add i16 %t, %a
  ret i16 %r
}}
define i16 @probe() {{
  %r = call i16 (i16, ...) @mix(i16 1, i16 5, i32 74496)
  ret i16 %r
}}"
    );
    // 74496 = 0x12300: (0x12300 >> 8) + 5 = 0x128
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(exit_code(&format!("va-mix-{target}"), &ir, target), 0x28, "{target}");
    }
}

/// `va_copy` copies the list pointer: reading the copy does not move the
/// original.
#[test]
fn a_copied_list_reads_the_same_arguments_again() {
    let ir = format!(
        "{PRELUDE}declare void @llvm.va_copy.p0.p0(ptr, ptr)
define i16 @twice(i16 %n, ...) {{
  %ap = alloca ptr, align 1
  %ap2 = alloca ptr, align 1
  call void @llvm.va_start.p0(ptr %ap)
  call void @llvm.va_copy.p0.p0(ptr %ap2, ptr %ap)
  %x = va_arg ptr %ap, i16
  %y = va_arg ptr %ap2, i16
  %z = va_arg ptr %ap2, i16
  call void @llvm.va_end.p0(ptr %ap2)
  call void @llvm.va_end.p0(ptr %ap)
  %s = add i16 %x, %y
  %r = add i16 %s, %z
  ret i16 %r
}}
define i16 @probe() {{
  %r = call i16 (i16, ...) @twice(i16 2, i16 10, i16 20)
  ret i16 %r
}}"
    );
    // x = y = 10 (first unnamed), z = 20: 40
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(exit_code(&format!("va-copy-{target}"), &ir, target), 40, "{target}");
    }
}
